//! Sketch editor panel: tool selector, shape list, constraints, export.

use leptos::prelude::*;

use crate::sketch::{shape_label, shapes_to_svg, ConstraintKind, DrawingTool, Point, SketchShape};

use super::canvas::SketchCanvas;
use super::constraint_bridge;

// ── Undo history entry ─────────────────────────────────────────────

type ShapeVec = Vec<SketchShape>;

// ── Constraint list entry ──────────────────────────────────────────

#[derive(Clone, Debug)]
struct ConstraintEntry {
    id: u32,
    label: String,
}

// ── Component ──────────────────────────────────────────────────────

#[component]
pub fn SketchEditor() -> impl IntoView {
    // -- Shape state --
    let (shapes, set_shapes) = signal(Vec::<SketchShape>::new());
    let (selected, set_selected) = signal(None::<usize>);
    let (active_tool, set_active_tool) = signal(DrawingTool::Line);
    let (grid_size, set_grid_size) = signal(10.0_f64);
    let (redraw_trigger, set_redraw_trigger) = signal(0_u32);

    // -- Undo stack --
    let (_undo_stack, set_undo_stack) = signal(Vec::<ShapeVec>::new());

    // Push current state onto undo stack whenever shapes change.
    let prev_len = StoredValue::new(0_usize);
    Effect::new(move |_| {
        let current = shapes.get();
        let old_len = prev_len.get_value();
        if current.len() != old_len && old_len > 0 {
            // We don't push if this was the initial empty state.
            // The undo stack is managed via push_undo instead.
        }
        prev_len.set_value(current.len());
    });

    // Helper to record undo before mutating.
    let push_undo = move || {
        let snap = shapes.get_untracked();
        set_undo_stack.update(|stack| stack.push(snap));
    };

    // Wrap set_shapes so every mutation records undo.
    let add_shape = move |shape: SketchShape| {
        push_undo();
        set_shapes.update(|v| v.push(shape));
    };

    // Watch for new shapes added by the canvas (it calls set_shapes directly).
    // We intercept by wrapping in an effect that detects growth.
    let tracked_len = StoredValue::new(0_usize);
    Effect::new(move |_| {
        let current = shapes.get();
        let old = tracked_len.get_value();
        if current.len() > old && old > 0 {
            // A shape was added by the canvas — record the previous state.
            let mut prev = current.clone();
            prev.truncate(old);
            set_undo_stack.update(|stack| stack.push(prev));
        }
        tracked_len.set_value(current.len());
    });

    let on_undo = move |_| {
        set_undo_stack.update(|stack| {
            if let Some(prev) = stack.pop() {
                set_shapes.set(prev);
                set_selected.set(None);
            }
        });
    };

    let on_clear = move |_| {
        push_undo();
        set_shapes.set(Vec::new());
        set_selected.set(None);
    };

    // -- Constraint state --
    let (constraint_kind, set_constraint_kind) = signal(ConstraintKind::Coincident);
    let (constraints, set_constraints) = signal(Vec::<ConstraintEntry>::new());
    let (dof_text, set_dof_text) = signal(String::from("DOF: --"));
    let (cst_status, set_cst_status) = signal(String::new());

    // Sync shapes to solver whenever shapes change and re-solve.
    Effect::new(move |_| {
        let current_shapes = shapes.get();
        constraint_bridge::reset();

        // Re-add all points from shapes.
        // We don't persist point IDs across resets, so constraints are also reset.
        let mut _point_ids = Vec::<Vec<u32>>::new();
        for shape in &current_shapes {
            let ids = add_shape_points(shape);
            _point_ids.push(ids);
        }

        // Re-solve and update DOF display.
        if let Some(snap) = constraint_bridge::solve() {
            let status_label = match snap.dof_status.as_str() {
                "FullyConstrained" => "Fully Constrained",
                "OverConstrained" => "Over Constrained",
                _ => "Under Constrained",
            };
            set_dof_text.set(format!("DOF: {} ({})", snap.dof, status_label));

            // Update constraint list from snapshot.
            let entries: Vec<ConstraintEntry> = snap
                .constraints
                .iter()
                .map(|(id, val)| {
                    let label = constraint_value_label(val);
                    ConstraintEntry { id: *id, label }
                })
                .collect();
            set_constraints.set(entries);
        }

        // Trigger a canvas repaint.
        set_redraw_trigger.update(|n| *n = n.wrapping_add(1));
    });

    let on_add_constraint = move |_| {
        let kind = constraint_kind.get_untracked();
        let needed = kind.pick_count();
        let current_shapes = shapes.get_untracked();

        // For simplicity: if we have a selection, use its points.
        // For 2-point constraints on a line: use both endpoints.
        // For fixed: use the first point of the selected shape.
        let sel = selected.get_untracked();
        if sel.is_none() {
            set_cst_status.set("Select a shape first.".into());
            return;
        }
        let sel_idx = sel.unwrap();
        if sel_idx >= current_shapes.len() {
            set_cst_status.set("Invalid selection.".into());
            return;
        }

        // Compute solver point IDs for the selected shape.
        // Point IDs are 1-based and allocated in order of shape creation.
        let mut offset = 0_u32;
        for (i, s) in current_shapes.iter().enumerate() {
            if i == sel_idx {
                break;
            }
            offset += shape_point_count(s) as u32;
        }
        let shape = &current_shapes[sel_idx];
        let n_pts = shape_point_count(shape) as u32;
        let ids: Vec<u32> = (1..=n_pts).map(|i| offset + i).collect();

        if (ids.len()) < needed {
            set_cst_status.set(format!(
                "Shape has {} points, need {} for {}.",
                ids.len(),
                needed,
                kind.label()
            ));
            return;
        }

        let (value, value2) = if kind == ConstraintKind::Fixed {
            // Fix the first point at its current position.
            let p = first_point(shape);
            (p.x, p.y)
        } else if kind == ConstraintKind::Distance && ids.len() >= 2 {
            // Use the current distance between the first two points.
            let pts = shape_points(shape);
            if pts.len() >= 2 {
                let dx = pts[1].x - pts[0].x;
                let dy = pts[1].y - pts[0].y;
                ((dx * dx + dy * dy).sqrt(), 0.0)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let pick_ids: Vec<u32> = ids.into_iter().take(needed).collect();
        if let Some(cid) =
            constraint_bridge::add_constraint(kind.api_name(), &pick_ids, value, value2)
        {
            set_cst_status.set(format!("Added {} (id={}).", kind.label(), cid));
        } else {
            set_cst_status.set("Failed to add constraint.".into());
        }

        // Re-solve.
        if let Some(snap) = constraint_bridge::solve() {
            let status_label = match snap.dof_status.as_str() {
                "FullyConstrained" => "Fully Constrained",
                "OverConstrained" => "Over Constrained",
                _ => "Under Constrained",
            };
            set_dof_text.set(format!("DOF: {} ({})", snap.dof, status_label));
            let entries: Vec<ConstraintEntry> = snap
                .constraints
                .iter()
                .map(|(id, val)| {
                    let label = constraint_value_label(val);
                    ConstraintEntry { id: *id, label }
                })
                .collect();
            set_constraints.set(entries);
        }
        set_redraw_trigger.update(|n| *n = n.wrapping_add(1));
    };

    let on_remove_constraint = move |cid: u32| {
        constraint_bridge::remove_constraint(cid);
        if let Some(snap) = constraint_bridge::solve() {
            let status_label = match snap.dof_status.as_str() {
                "FullyConstrained" => "Fully Constrained",
                "OverConstrained" => "Over Constrained",
                _ => "Under Constrained",
            };
            set_dof_text.set(format!("DOF: {} ({})", snap.dof, status_label));
            let entries: Vec<ConstraintEntry> = snap
                .constraints
                .iter()
                .map(|(id, val)| {
                    let label = constraint_value_label(val);
                    ConstraintEntry { id: *id, label }
                })
                .collect();
            set_constraints.set(entries);
        }
        set_redraw_trigger.update(|n| *n = n.wrapping_add(1));
    };

    // -- Export to SVG --
    let on_use_in_cam = move |_| {
        let current = shapes.get_untracked();
        let svg = shapes_to_svg(&current);
        // Store SVG in local storage for the CAM panel to pick up.
        if let Some(win) = web_sys::window() {
            if let Ok(Some(storage)) = win.local_storage() {
                let _ = storage.set_item("sketch_svg", &svg);
            }
        }
        web_sys::console::log_1(&"Sketch exported to SVG in localStorage('sketch_svg')".into());
    };

    // -- Tool buttons --
    let tools = [
        (DrawingTool::Line, "Line"),
        (DrawingTool::Rectangle, "Rect"),
        (DrawingTool::Circle, "Circle"),
        (DrawingTool::Polyline, "Polyline"),
    ];

    let constraint_kinds = [
        (ConstraintKind::Coincident, "Coincident"),
        (ConstraintKind::Horizontal, "Horizontal"),
        (ConstraintKind::Vertical, "Vertical"),
        (ConstraintKind::Distance, "Distance"),
        (ConstraintKind::Fixed, "Fixed"),
    ];

    // We suppress the "unused variable" warning: `add_shape` is available but canvas
    // calls set_shapes directly.
    let _ = add_shape;

    view! {
        <div style="display:flex;height:calc(100vh - 100px);gap:8px;padding:8px;">
            // -- Left panel: tools + shapes --
            <div style="width:220px;flex-shrink:0;display:flex;flex-direction:column;gap:8px;overflow-y:auto;">
                // Tool selector
                <div style="background:#16213e;padding:8px;border-radius:4px;">
                    <div style="font-weight:bold;margin-bottom:6px;color:#eee;">"Drawing Tools"</div>
                    <div style="display:flex;flex-wrap:wrap;gap:4px;">
                        {tools.into_iter().map(|(tool, label)| {
                            let tool_clone = tool.clone();
                            view! {
                                <button
                                    style=move || {
                                        let base = "padding:4px 10px;border:1px solid #555;border-radius:3px;cursor:pointer;font-size:12px;";
                                        if active_tool.get() == tool_clone {
                                            format!("{base}background:#2196F3;color:#fff;")
                                        } else {
                                            format!("{base}background:#2a2a4a;color:#ccc;")
                                        }
                                    }
                                    on:click=move |_| set_active_tool.set(tool.clone())
                                >
                                    {label}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </div>

                // Grid size
                <div style="background:#16213e;padding:8px;border-radius:4px;">
                    <label style="color:#ccc;font-size:12px;">"Grid Snap: "
                        <input
                            type="number"
                            prop:value=move || grid_size.get().to_string()
                            on:change=move |ev| {
                                let target = leptos::prelude::event_target::<web_sys::HtmlInputElement>(&ev);
                                if let Ok(v) = target.value().parse::<f64>() {
                                    if v >= 0.0 {
                                        set_grid_size.set(v);
                                    }
                                }
                            }
                            style="width:60px;background:#1a1a2e;color:#eee;border:1px solid #555;border-radius:3px;padding:2px 4px;"
                        />
                    </label>
                </div>

                // Actions
                <div style="background:#16213e;padding:8px;border-radius:4px;display:flex;gap:4px;">
                    <button
                        style="padding:4px 10px;background:#FF9800;color:#fff;border:none;border-radius:3px;cursor:pointer;font-size:12px;"
                        on:click=on_undo
                    >"Undo (Ctrl+Z)"</button>
                    <button
                        style="padding:4px 10px;background:#f44336;color:#fff;border:none;border-radius:3px;cursor:pointer;font-size:12px;"
                        on:click=on_clear
                    >"Clear All"</button>
                </div>

                // Shape list
                <div style="background:#16213e;padding:8px;border-radius:4px;flex:1;overflow-y:auto;">
                    <div style="font-weight:bold;margin-bottom:6px;color:#eee;">
                        "Shapes ("
                        {move || shapes.get().len().to_string()}
                        ")"
                    </div>
                    <div style="font-size:11px;color:#aaa;">
                        {move || {
                            let s = shapes.get();
                            if s.is_empty() {
                                vec![view! { <div>"No shapes yet."</div> }.into_any()]
                            } else {
                                s.iter().enumerate().map(|(i, shape)| {
                                    let label = shape_label(shape);
                                    let is_sel = selected.get() == Some(i);
                                    let bg = if is_sel { "#333366" } else { "transparent" };
                                    view! {
                                        <div
                                            style=format!("padding:2px 4px;cursor:pointer;border-radius:2px;background:{bg};")
                                            on:click=move |_| set_selected.set(Some(i))
                                        >
                                            {format!("{}. {}", i + 1, label)}
                                        </div>
                                    }.into_any()
                                }).collect::<Vec<_>>()
                            }
                        }}
                    </div>
                </div>
            </div>

            // -- Center: canvas --
            <div style="flex:1;min-width:0;">
                <SketchCanvas
                    shapes=shapes
                    set_shapes=set_shapes
                    active_tool=active_tool
                    selected=selected
                    set_selected=set_selected
                    grid_size=grid_size
                    redraw_trigger=redraw_trigger
                />
            </div>

            // -- Right panel: constraints --
            <div style="width:220px;flex-shrink:0;display:flex;flex-direction:column;gap:8px;overflow-y:auto;">
                // Constraint tools
                <div style="background:#16213e;padding:8px;border-radius:4px;">
                    <div style="font-weight:bold;margin-bottom:6px;color:#eee;">"Constraints"</div>

                    // Constraint type selector
                    <div style="margin-bottom:6px;">
                        <select
                            style="width:100%;background:#1a1a2e;color:#eee;border:1px solid #555;border-radius:3px;padding:4px;"
                            on:change=move |ev| {
                                let target = leptos::prelude::event_target::<web_sys::HtmlInputElement>(&ev);
                                let val = target.value();
                                let kind = match val.as_str() {
                                    "Horizontal" => ConstraintKind::Horizontal,
                                    "Vertical" => ConstraintKind::Vertical,
                                    "Distance" => ConstraintKind::Distance,
                                    "Fixed" => ConstraintKind::Fixed,
                                    _ => ConstraintKind::Coincident,
                                };
                                set_constraint_kind.set(kind);
                            }
                        >
                            {constraint_kinds.iter().map(|(_, label)| {
                                view! {
                                    <option value={*label}>{*label}</option>
                                }
                            }).collect_view()}
                        </select>
                    </div>

                    <button
                        style="width:100%;padding:6px;background:#4CAF50;color:#fff;border:none;border-radius:3px;cursor:pointer;font-size:12px;margin-bottom:6px;"
                        on:click=on_add_constraint
                    >"Add Constraint"</button>

                    // Status
                    <div style="font-size:11px;color:#ff9800;min-height:16px;">
                        {move || cst_status.get()}
                    </div>
                </div>

                // DOF status
                <div style="background:#16213e;padding:8px;border-radius:4px;">
                    <div style="font-weight:bold;color:#eee;font-size:13px;">
                        {move || dof_text.get()}
                    </div>
                </div>

                // Constraint list
                <div style="background:#16213e;padding:8px;border-radius:4px;flex:1;overflow-y:auto;">
                    <div style="font-weight:bold;margin-bottom:6px;color:#eee;">"Active Constraints"</div>
                    <div style="font-size:11px;color:#aaa;">
                        {move || {
                            let csts = constraints.get();
                            if csts.is_empty() {
                                vec![view! { <div>"None"</div> }.into_any()]
                            } else {
                                csts.iter().map(|entry| {
                                    let cid = entry.id;
                                    let label = entry.label.clone();
                                    view! {
                                        <div style="display:flex;justify-content:space-between;align-items:center;padding:2px 0;">
                                            <span>{format!("{}. {}", cid, label)}</span>
                                            <button
                                                style="background:none;border:none;color:#f44336;cursor:pointer;font-size:12px;padding:0 4px;"
                                                on:click=move |_| on_remove_constraint(cid)
                                            >"x"</button>
                                        </div>
                                    }.into_any()
                                }).collect::<Vec<_>>()
                            }
                        }}
                    </div>
                </div>

                // Export
                <div style="background:#16213e;padding:8px;border-radius:4px;">
                    <button
                        style="width:100%;padding:8px;background:#9C27B0;color:#fff;border:none;border-radius:3px;cursor:pointer;font-size:12px;"
                        on:click=on_use_in_cam
                    >"Use Sketch in CAM"</button>
                </div>
            </div>
        </div>
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Add all points of a shape to the constraint solver.
fn add_shape_points(shape: &SketchShape) -> Vec<u32> {
    let pts = shape_points(shape);
    pts.iter()
        .filter_map(|p| constraint_bridge::add_point(p.x, p.y))
        .collect()
}

/// Extract raw points from a shape.
fn shape_points(shape: &SketchShape) -> Vec<Point> {
    match shape {
        SketchShape::Line { p1, p2 } => vec![p1.clone(), p2.clone()],
        SketchShape::Rectangle {
            origin,
            width,
            height,
        } => vec![
            origin.clone(),
            Point::new(origin.x + width, origin.y),
            Point::new(origin.x + width, origin.y + height),
            Point::new(origin.x, origin.y + height),
        ],
        SketchShape::Circle { center, radius } => {
            vec![center.clone(), Point::new(center.x + radius, center.y)]
        }
        SketchShape::Polyline { points } => points.clone(),
    }
}

/// Number of solver points for a shape.
fn shape_point_count(shape: &SketchShape) -> usize {
    match shape {
        SketchShape::Line { .. } => 2,
        SketchShape::Rectangle { .. } => 4,
        SketchShape::Circle { .. } => 2,
        SketchShape::Polyline { points } => points.len(),
    }
}

/// First point of a shape (for Fixed constraint default values).
fn first_point(shape: &SketchShape) -> Point {
    match shape {
        SketchShape::Line { p1, .. } => p1.clone(),
        SketchShape::Rectangle { origin, .. } => origin.clone(),
        SketchShape::Circle { center, .. } => center.clone(),
        SketchShape::Polyline { points } => points.first().cloned().unwrap_or(Point::new(0.0, 0.0)),
    }
}

/// Human-readable label for a constraint JSON value from the solver snapshot.
fn constraint_value_label(val: &serde_json::Value) -> String {
    if let Some(obj) = val.as_object() {
        // The solver serializes constraints as {"Horizontal": [id1, id2]} etc.
        if let Some((key, _)) = obj.iter().next() {
            return key.clone();
        }
    }
    if let Some(s) = val.as_str() {
        return s.to_string();
    }
    format!("{val}")
}
