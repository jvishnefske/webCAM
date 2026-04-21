//! Panel editor component: widget palette, workspace, property inspector.

use std::collections::BTreeMap;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::types::*;
use crate::app::{ExternalInputs, SimTopics};

// ---------------------------------------------------------------------------
// LocalStorage helpers
// ---------------------------------------------------------------------------

const STORAGE_KEY: &str = "rustcam-panel";

fn save_panel(panel: &PanelModel) {
    let Ok(json) = serde_json::to_string(panel) else {
        return;
    };
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let _ = storage.set_item(STORAGE_KEY, &json);
}

fn load_panel() -> Option<PanelModel> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    let json = storage.get_item(STORAGE_KEY).ok()??;
    serde_json::from_str(&json).ok()
}

// ---------------------------------------------------------------------------
// PanelEditor component
// ---------------------------------------------------------------------------

#[component]
pub fn PanelEditor() -> impl IntoView {
    // -- Core model --
    let initial = load_panel().unwrap_or_else(|| PanelModel::new("Untitled Panel"));
    let (panel, set_panel) = signal(initial);

    // -- Selection --
    let (selected_id, set_selected_id) = signal(None::<u32>);

    // -- Live topic values (DAG tick loop writes; we read) --
    let sim_topics = use_context::<SimTopics>().expect("SimTopics must be provided via context");
    let external_inputs =
        use_context::<ExternalInputs>().expect("ExternalInputs must be provided via context");

    // -- Drag state --
    let (dragging, set_dragging) = signal(None::<DragState>);

    // -- Palette kinds --
    let palette_kinds: Vec<(WidgetKind, &'static str)> = vec![
        (WidgetKind::Toggle, "Toggle"),
        (
            WidgetKind::Slider {
                min: 0.0,
                max: 100.0,
                step: 1.0,
            },
            "Slider",
        ),
        (
            WidgetKind::Gauge {
                min: 0.0,
                max: 100.0,
            },
            "Gauge",
        ),
        (WidgetKind::Label, "Label"),
        (WidgetKind::Button, "Button"),
        (WidgetKind::Indicator, "Indicator"),
    ];

    // -- Add widget handler --
    let add_widget = move |kind: WidgetKind| {
        let label = kind.display_name().to_string();
        set_panel.update(|p| {
            let id = p.add_widget(kind, &label);
            set_selected_id.set(Some(id));
        });
    };

    // -- Save / Load --
    let on_save = move |_| {
        let p = panel.get();
        save_panel(&p);
    };

    let on_load = move |_| {
        if let Some(p) = load_panel() {
            set_panel.set(p);
            set_selected_id.set(None);
        }
    };

    // -- Delete selected --
    let on_delete_selected = move |_| {
        if let Some(id) = selected_id.get() {
            set_panel.update(|p| {
                p.remove_widget(id);
            });
            set_selected_id.set(None);
        }
    };

    // -- Panel name --
    let on_name_change = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        set_panel.update(|p| p.name = val);
    };

    // -- Workspace mouse handlers for drag --
    let on_workspace_mousedown = move |ev: web_sys::MouseEvent| {
        // Find the widget element under the cursor.
        let target = match ev.target() {
            Some(t) => t,
            None => return,
        };
        let el: web_sys::Element = match target.dyn_into() {
            Ok(e) => e,
            Err(_) => return,
        };

        // Walk up to find a [data-widget-id] element.
        let widget_el = find_widget_ancestor(&el);
        let Some(widget_el) = widget_el else {
            // Clicked on empty workspace.
            set_selected_id.set(None);
            return;
        };
        let Some(id_str) = widget_el.get_attribute("data-widget-id") else {
            return;
        };
        let Ok(widget_id) = id_str.parse::<u32>() else {
            return;
        };

        set_selected_id.set(Some(widget_id));

        // Start drag.
        let p = panel.get();
        if let Some(w) = p.get_widget(widget_id) {
            set_dragging.set(Some(DragState {
                widget_id,
                start_mouse_x: ev.client_x() as f64,
                start_mouse_y: ev.client_y() as f64,
                start_widget_x: w.x,
                start_widget_y: w.y,
            }));
        }
        ev.prevent_default();
    };

    let on_workspace_mousemove = move |ev: web_sys::MouseEvent| {
        let Some(drag) = dragging.get_untracked() else {
            return;
        };
        let dx = ev.client_x() as f64 - drag.start_mouse_x;
        let dy = ev.client_y() as f64 - drag.start_mouse_y;
        let new_x = (drag.start_widget_x + dx).max(0.0);
        let new_y = (drag.start_widget_y + dy).max(0.0);

        set_panel.update(|p| {
            if let Some(w) = p.get_widget_mut(drag.widget_id) {
                w.x = new_x;
                w.y = new_y;
            }
        });
    };

    let on_workspace_mouseup = move |_ev: web_sys::MouseEvent| {
        set_dragging.set(None);
    };

    view! {
        <div class="panel-editor" style="display:flex; gap:1rem; height:100%;">
            // -- Sidebar: palette + save/load --
            <div class="panel-sidebar" style="width:180px; flex-shrink:0; display:flex; flex-direction:column; gap:0.5rem;">
                <div class="card" style="padding:0.5rem;">
                    <div class="card-title">"Panel Name"</div>
                    <input
                        type="text"
                        style="width:100%;"
                        prop:value=move || panel.get().name.clone()
                        on:input=on_name_change
                    />
                </div>

                <div class="card" style="padding:0.5rem;">
                    <div class="card-title">"Add Widget"</div>
                    {palette_kinds.into_iter().map(|(kind, label)| {
                        let kind = kind.clone();
                        let add = add_widget;
                        view! {
                            <button
                                class="btn btn-secondary"
                                style="width:100%; margin-bottom:0.25rem;"
                                on:click=move |_| add(kind.clone())
                            >
                                {label}
                            </button>
                        }
                    }).collect_view()}
                </div>

                <div class="card" style="padding:0.5rem;">
                    <div class="card-title">"Storage"</div>
                    <button class="btn btn-primary" style="width:100%; margin-bottom:0.25rem;" on:click=on_save>"Save"</button>
                    <button class="btn btn-secondary" style="width:100%;" on:click=on_load>"Load"</button>
                </div>
            </div>

            // -- Workspace --
            <div
                class="panel-workspace"
                style="flex:1; position:relative; background:#1e1e2e; border-radius:8px; min-height:400px; overflow:hidden;"
                on:mousedown=on_workspace_mousedown
                on:mousemove=on_workspace_mousemove
                on:mouseup=on_workspace_mouseup
            >
                {move || {
                    let p = panel.get();
                    let sel = selected_id.get();
                    let topics = sim_topics.0.get();
                    let ext_snapshot = external_inputs.0.get();
                    p.widgets.iter().map(|w| {
                        let is_selected = sel == Some(w.id);
                        render_widget(w, is_selected, &topics, &ext_snapshot, external_inputs)
                    }).collect_view()
                }}
            </div>

            // -- Property inspector --
            <div class="panel-inspector" style="width:240px; flex-shrink:0;">
                {move || {
                    let sel = selected_id.get();
                    let p = panel.get();
                    match sel.and_then(|id| p.get_widget(id).cloned()) {
                        None => view! {
                            <div class="card" style="padding:0.5rem;">
                                <div class="card-title">"Inspector"</div>
                                <p style="color:#888;">"Select a widget to edit its properties."</p>
                            </div>
                        }.into_any(),
                        Some(widget) => {
                            render_inspector(
                                widget,
                                set_panel,
                                sim_topics,
                                on_delete_selected,
                            )
                        },
                    }
                }}
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// DragState
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct DragState {
    widget_id: u32,
    start_mouse_x: f64,
    start_mouse_y: f64,
    start_widget_x: f64,
    start_widget_y: f64,
}

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

/// Walk up the DOM tree to find the nearest ancestor with `data-widget-id`.
fn find_widget_ancestor(el: &web_sys::Element) -> Option<web_sys::Element> {
    let mut current: Option<web_sys::Element> = Some(el.clone());
    while let Some(node) = current {
        if node.has_attribute("data-widget-id") {
            return Some(node);
        }
        current = node.parent_element();
    }
    None
}

// ---------------------------------------------------------------------------
// Widget rendering
// ---------------------------------------------------------------------------

fn render_widget(
    w: &Widget,
    selected: bool,
    topics: &BTreeMap<String, f64>,
    external_inputs_snapshot: &BTreeMap<String, f64>,
    external_inputs: ExternalInputs,
) -> impl IntoView {
    let border = if selected {
        "2px solid #60a5fa"
    } else {
        "1px solid #444"
    };

    let style = format!(
        "position:absolute; left:{x}px; top:{y}px; width:{w}px; height:{h}px; \
         background:#2a2a3e; border:{border}; border-radius:4px; padding:4px; \
         cursor:grab; user-select:none; display:flex; flex-direction:column; \
         align-items:center; justify-content:center; color:#ddd; font-size:12px;",
        x = w.x,
        y = w.y,
        w = w.width,
        h = w.height,
        border = border,
    );

    // Collect output-binding topics (written on interaction).
    let out_topics: Vec<String> = w
        .bindings
        .iter()
        .filter(|b| b.direction == BindingDirection::Output && !b.topic.is_empty())
        .map(|b| b.topic.clone())
        .collect();

    // Resolve the effective display value:
    //   Output binding → show our last-published value (from external_inputs).
    //   Input binding  → show what the DAG has put into sim_topics.
    let input_val = out_topics
        .first()
        .and_then(|t| external_inputs_snapshot.get(t))
        .copied()
        .or_else(|| {
            w.bindings
                .iter()
                .find(|b| b.direction == BindingDirection::Input)
                .and_then(|b| topics.get(&b.topic))
                .copied()
        })
        .unwrap_or(0.0);

    let id = w.id;
    let id_str = id.to_string();
    let label = w.label.clone();

    let inner = match &w.kind {
        WidgetKind::Toggle => {
            let checked = input_val > 0.5;
            let out_for_click = out_topics.clone();
            let on_toggle = move |ev: web_sys::MouseEvent| {
                ev.stop_propagation();
                let new_val = if checked { 0.0 } else { 1.0 };
                if !out_for_click.is_empty() {
                    external_inputs.0.update(|m| {
                        for t in &out_for_click {
                            m.insert(t.clone(), new_val);
                        }
                    });
                }
            };
            view! {
                <div on:mousedown=on_toggle>
                    <div style="font-weight:600; margin-bottom:2px;">{label}</div>
                    <div style=move || {
                        if checked {
                            "width:32px;height:18px;background:#22c55e;border-radius:9px;position:relative;cursor:pointer;"
                        } else {
                            "width:32px;height:18px;background:#555;border-radius:9px;position:relative;cursor:pointer;"
                        }
                    }>
                        <div style=move || {
                            if checked {
                                "width:14px;height:14px;background:#fff;border-radius:50%;position:absolute;top:2px;left:16px;transition:left 0.15s;"
                            } else {
                                "width:14px;height:14px;background:#fff;border-radius:50%;position:absolute;top:2px;left:2px;transition:left 0.15s;"
                            }
                        }></div>
                    </div>
                </div>
            }
            .into_any()
        }
        WidgetKind::Slider { min, max, step } => {
            let min = *min;
            let max = *max;
            let step = *step;
            let val = input_val.clamp(min, max);
            let out_for_input = out_topics.clone();
            let on_slide = move |ev: web_sys::Event| {
                let v: f64 = event_target_value(&ev).parse().unwrap_or(0.0);
                if !out_for_input.is_empty() {
                    external_inputs.0.update(|m| {
                        for t in &out_for_input {
                            m.insert(t.clone(), v);
                        }
                    });
                }
            };
            let on_mousedown = |ev: web_sys::MouseEvent| {
                // Prevent the workspace drag handler from stealing focus.
                ev.stop_propagation();
            };
            view! {
                <div style="width:100%; text-align:center;">
                    <div style="font-weight:600; margin-bottom:2px;">{label}</div>
                    <input
                        type="range"
                        style="width:90%;"
                        prop:min=min.to_string()
                        prop:max=max.to_string()
                        prop:step=step.to_string()
                        prop:value=val.to_string()
                        on:input=on_slide
                        on:mousedown=on_mousedown
                    />
                    <div style="font-size:10px; color:#aaa;">{format!("{val:.1}")}</div>
                </div>
            }
            .into_any()
        }
        WidgetKind::Gauge { min, max } => {
            let min = *min;
            let max = *max;
            let range = (max - min).max(1e-9);
            let pct = ((input_val - min) / range * 100.0).clamp(0.0, 100.0);
            view! {
                <div style="width:100%; text-align:center;">
                    <div style="font-weight:600; margin-bottom:2px;">{label}</div>
                    <div style="width:90%; height:12px; background:#333; border-radius:6px; margin:0 auto; overflow:hidden;">
                        <div style=format!(
                            "width:{pct}%; height:100%; background:linear-gradient(90deg, #22c55e, #60a5fa); border-radius:6px;"
                        )></div>
                    </div>
                    <div style="font-size:10px; color:#aaa;">{format!("{input_val:.1}")}</div>
                </div>
            }
            .into_any()
        }
        WidgetKind::Label => {
            view! {
                <div style="text-align:center;">
                    <div style="font-weight:600; margin-bottom:2px;">{label}</div>
                    <div style="font-size:14px; font-family:monospace;">{format!("{input_val:.2}")}</div>
                </div>
            }
            .into_any()
        }
        WidgetKind::Button => {
            let out_down = out_topics.clone();
            let out_up = out_topics.clone();
            let on_down = move |ev: web_sys::MouseEvent| {
                ev.stop_propagation();
                if !out_down.is_empty() {
                    external_inputs.0.update(|m| {
                        for t in &out_down {
                            m.insert(t.clone(), 1.0);
                        }
                    });
                }
            };
            let on_up = move |_ev: web_sys::MouseEvent| {
                if !out_up.is_empty() {
                    external_inputs.0.update(|m| {
                        for t in &out_up {
                            m.insert(t.clone(), 0.0);
                        }
                    });
                }
            };
            view! {
                <div style="text-align:center;">
                    <button
                        class="btn btn-primary"
                        style="font-size:11px; padding:2px 8px;"
                        on:mousedown=on_down
                        on:mouseup=on_up
                    >
                        {label}
                    </button>
                </div>
            }
            .into_any()
        }
        WidgetKind::Indicator => {
            let color = if input_val > 0.5 { "#22c55e" } else { "#555" };
            view! {
                <div style="text-align:center;">
                    <div style=format!(
                        "width:24px; height:24px; border-radius:50%; background:{color}; margin:0 auto;"
                    )></div>
                    <div style="font-size:10px; margin-top:2px;">{label}</div>
                </div>
            }
            .into_any()
        }
    };

    view! {
        <div
            data-widget-id=id_str
            style=style
        >
            {inner}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Property inspector
// ---------------------------------------------------------------------------

fn render_inspector(
    widget: Widget,
    set_panel: WriteSignal<PanelModel>,
    sim_topics: SimTopics,
    on_delete: impl Fn(web_sys::MouseEvent) + 'static,
) -> AnyView {
    let wid = widget.id;
    let kind = widget.kind.clone();
    let schema = widget.kind.binding_schema();
    let bindings = widget.bindings.clone();

    // Label change handler.
    let on_label_change = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        set_panel.update(|p| {
            if let Some(w) = p.get_widget_mut(wid) {
                w.label = val;
            }
        });
    };

    // Kind-specific parameter editors.
    let kind_params = match kind {
        WidgetKind::Slider { min, max, step } => {
            let on_min = move |ev: web_sys::Event| {
                let val: f64 = event_target_value(&ev).parse().unwrap_or(min);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        if let WidgetKind::Slider { ref mut min, .. } = w.kind {
                            *min = val;
                        }
                    }
                });
            };
            let on_max = move |ev: web_sys::Event| {
                let val: f64 = event_target_value(&ev).parse().unwrap_or(max);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        if let WidgetKind::Slider { ref mut max, .. } = w.kind {
                            *max = val;
                        }
                    }
                });
            };
            let on_step = move |ev: web_sys::Event| {
                let val: f64 = event_target_value(&ev).parse().unwrap_or(step);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        if let WidgetKind::Slider { ref mut step, .. } = w.kind {
                            *step = val;
                        }
                    }
                });
            };
            view! {
                <div>
                    <label style="font-size:11px;">"Min"</label>
                    <input type="number" style="width:100%;" prop:value=min.to_string()
                        on:input=on_min />
                    <label style="font-size:11px;">"Max"</label>
                    <input type="number" style="width:100%;" prop:value=max.to_string()
                        on:input=on_max />
                    <label style="font-size:11px;">"Step"</label>
                    <input type="number" style="width:100%;" prop:value=step.to_string()
                        on:input=on_step />
                </div>
            }
            .into_any()
        }
        WidgetKind::Gauge { min, max } => {
            let on_min = move |ev: web_sys::Event| {
                let val: f64 = event_target_value(&ev).parse().unwrap_or(min);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        if let WidgetKind::Gauge { ref mut min, .. } = w.kind {
                            *min = val;
                        }
                    }
                });
            };
            let on_max = move |ev: web_sys::Event| {
                let val: f64 = event_target_value(&ev).parse().unwrap_or(max);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        if let WidgetKind::Gauge { ref mut max, .. } = w.kind {
                            *max = val;
                        }
                    }
                });
            };
            view! {
                <div>
                    <label style="font-size:11px;">"Min"</label>
                    <input type="number" style="width:100%;" prop:value=min.to_string()
                        on:input=on_min />
                    <label style="font-size:11px;">"Max"</label>
                    <input type="number" style="width:100%;" prop:value=max.to_string()
                        on:input=on_max />
                </div>
            }
            .into_any()
        }
        _ => view! { <div></div> }.into_any(),
    };

    // Bindings are a fixed schema per widget kind. User cannot add/remove —
    // only the topic of each role is editable. For Input roles we suggest
    // known topics from sim_topics; for Output roles the topic is auto-filled.
    let datalist_id = format!("topics-{wid}");
    let datalist_id_for_view = datalist_id.clone();
    let bindings_view = schema
        .iter()
        .enumerate()
        .map(|(idx, role)| {
            let topic = bindings
                .get(idx)
                .map(|b| b.topic.clone())
                .unwrap_or_default();
            let role_name = role.name;
            let dir_label = match role.direction {
                BindingDirection::Input => "in",
                BindingDirection::Output => "out",
            };
            let is_input = role.direction == BindingDirection::Input;
            let list_id = datalist_id.clone();

            let on_topic_change = move |ev: web_sys::Event| {
                let val = event_target_value(&ev);
                set_panel.update(|p| {
                    if let Some(w) = p.get_widget_mut(wid) {
                        while w.bindings.len() <= idx {
                            w.bindings.push(ChannelBinding {
                                direction: role.direction,
                                topic: String::new(),
                            });
                        }
                        w.bindings[idx].direction = role.direction;
                        w.bindings[idx].topic = val;
                    }
                });
            };

            view! {
                <div style="margin-bottom:6px;">
                    <div style="font-size:11px; color:#888;">
                        {format!("{role_name} ({dir_label})")}
                    </div>
                    <input type="text" style="width:100%; font-size:11px;"
                        prop:value=topic
                        on:input=on_topic_change
                        placeholder=if is_input { "select topic…" } else { "panel/…" }
                        list=if is_input { list_id } else { String::new() }
                    />
                </div>
            }
        })
        .collect_view();

    // Datalist of currently-known topics (for Input binding autocomplete).
    let datalist_view = {
        let id = datalist_id_for_view;
        view! {
            <datalist id=id>
                {move || {
                    sim_topics.0.get()
                        .keys()
                        .cloned()
                        .map(|t| view! { <option value=t.clone()>{t.clone()}</option> })
                        .collect_view()
                }}
            </datalist>
        }
    };

    view! {
        <div class="card" style="padding:0.5rem;">
            <div class="card-title">"Inspector"</div>

            <label style="font-size:11px;">"Label"</label>
            <input type="text" style="width:100%; margin-bottom:0.5rem;"
                prop:value=widget.label.clone()
                on:input=on_label_change
            />

            <div style="font-size:11px; color:#888; margin-bottom:0.25rem;">
                {format!("Kind: {}", widget.kind.display_name())}
            </div>

            {kind_params}

            <hr style="border-color:#444; margin:0.5rem 0;" />
            <div class="card-title" style="font-size:12px;">"Channels"</div>
            {bindings_view}
            {datalist_view}

            <hr style="border-color:#444; margin:0.5rem 0;" />
            <button class="btn btn-danger" style="width:100%;" on:click=on_delete>
                "Delete Widget"
            </button>
        </div>
    }
    .into_any()
}
