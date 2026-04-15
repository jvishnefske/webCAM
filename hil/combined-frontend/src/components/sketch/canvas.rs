//! HTML5 `<canvas>` component for drawing sketch shapes with pan/zoom.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::sketch::{snap_to_grid, DrawingTool, Point, SketchShape};

// ── Constants ──────────────────────────────────────────────────────

const SHAPE_COLOR: &str = "#2196F3";
const SELECTED_COLOR: &str = "#FF9800";
const DRAFT_COLOR: &str = "#4CAF50";
const GRID_COLOR: &str = "#444";
const BORDER_COLOR: &str = "#666";
const POLYLINE_VERTEX_COLOR: &str = "#4CAF50";

// ── Component ──────────────────────────────────────────────────────

#[component]
pub fn SketchCanvas(
    shapes: ReadSignal<Vec<SketchShape>>,
    set_shapes: WriteSignal<Vec<SketchShape>>,
    active_tool: ReadSignal<DrawingTool>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    grid_size: ReadSignal<f64>,
    /// Incremented externally to request a repaint (e.g. after constraint solve).
    redraw_trigger: ReadSignal<u32>,
) -> impl IntoView {
    // Internal draft state: the shape being drawn right now.
    let (draft, set_draft) = signal(None::<SketchShape>);
    // Mouse-is-down flag (for drag-based tools).
    let (mouse_down, set_mouse_down) = signal(false);
    // Drag start point.
    let (drag_start, set_drag_start) = signal(None::<Point>);
    // Polyline accumulated points.
    let (poly_pts, set_poly_pts) = signal(Vec::<Point>::new());
    // Cursor world position for polyline rubber-band.
    let (cursor_pos, set_cursor_pos) = signal(None::<Point>);

    // Canvas world size (mm).
    let canvas_world_size = 100.0_f64;

    // ── Canvas node-ref ─────────────────────────────────────────────

    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // ── Coordinate transform helpers ────────────────────────────────

    let screen_to_world = move |client_x: f64, client_y: f64| -> Point {
        let Some(cvs) = canvas_ref.get() else {
            return Point::new(0.0, 0.0);
        };
        let el: &web_sys::Element = cvs.as_ref();
        let rect = el.get_bounding_client_rect();
        let w = rect.width();
        let h = rect.height();
        let pad = 20.0;
        let avail = w.min(h) - pad * 2.0;
        let scale = avail / canvas_world_size;
        let off_x = (w - canvas_world_size * scale) / 2.0;
        let off_y = (h - canvas_world_size * scale) / 2.0;
        let mut wx = (client_x - rect.left() - off_x) / scale;
        let mut wy = (client_y - rect.top() - off_y) / scale;
        let gs = grid_size.get_untracked();
        if gs > 0.0 {
            let (sx, sy) = snap_to_grid(wx, wy, gs);
            wx = sx;
            wy = sy;
        }
        // Round to 2 decimals.
        wx = (wx * 100.0).round() / 100.0;
        wy = (wy * 100.0).round() / 100.0;
        Point::new(wx, wy)
    };

    // ── Redraw logic ────────────────────────────────────────────────

    let do_redraw = move || {
        let Some(cvs) = canvas_ref.get() else {
            return;
        };
        let html_cvs: &web_sys::HtmlCanvasElement = cvs.as_ref();
        let el: &web_sys::Element = cvs.as_ref();
        let rect = el.get_bounding_client_rect();
        let w = rect.width();
        let h = rect.height();
        if w < 1.0 {
            return;
        }

        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        html_cvs.set_width((w * dpr) as u32);
        html_cvs.set_height((h * dpr) as u32);

        let ctx: web_sys::CanvasRenderingContext2d = html_cvs
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|obj| obj.dyn_into().ok())
            .expect("canvas 2d context");

        ctx.save();
        let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        ctx.clear_rect(0.0, 0.0, w, h);

        let size = canvas_world_size;
        let pad = 20.0;
        let avail = w.min(h) - pad * 2.0;
        let scale = avail / size;
        let off_x = (w - size * scale) / 2.0;
        let off_y = (h - size * scale) / 2.0;
        let tx = |v: f64| off_x + v * scale;
        let ty = |v: f64| off_y + v * scale;

        // Grid
        let gs = grid_size.get_untracked();
        let snap = if gs > 0.0 { gs } else { size / 10.0 };
        if size / snap <= 200.0 {
            ctx.set_stroke_style_str(GRID_COLOR);
            ctx.set_line_width(0.5);
            let mut v = 0.0;
            while v <= size + 1e-9 {
                ctx.begin_path();
                ctx.move_to(tx(v), ty(0.0));
                ctx.line_to(tx(v), ty(size));
                ctx.stroke();
                ctx.begin_path();
                ctx.move_to(tx(0.0), ty(v));
                ctx.line_to(tx(size), ty(v));
                ctx.stroke();
                v += snap;
            }
        }

        // Border
        ctx.set_stroke_style_str(BORDER_COLOR);
        ctx.set_line_width(1.0);
        ctx.stroke_rect(tx(0.0), ty(0.0), size * scale, size * scale);

        // Origin labels
        ctx.set_fill_style_str("#999");
        ctx.set_font("10px monospace");
        let _ = ctx.fill_text("0,0", tx(0.0) + 2.0, ty(0.0) - 4.0);
        let label = format!("{size},{size}");
        let _ = ctx.fill_text(&label, tx(size) - 40.0, ty(size) + 12.0);

        // Committed shapes
        let shapes_val = shapes.get_untracked();
        let sel = selected.get_untracked();
        for (i, shape) in shapes_val.iter().enumerate() {
            let is_selected = sel == Some(i);
            let color = if is_selected {
                SELECTED_COLOR
            } else {
                SHAPE_COLOR
            };
            let lw = if is_selected { 2.5 } else { 1.5 };
            draw_shape(&ctx, shape, color, lw, &tx, &ty, scale);
        }

        // Draft shape
        if let Some(ref d) = draft.get_untracked() {
            draw_shape(&ctx, d, DRAFT_COLOR, 1.5, &tx, &ty, scale);
        }

        // Polyline in-progress vertices
        let pts = poly_pts.get_untracked();
        if !pts.is_empty() {
            ctx.set_stroke_style_str(DRAFT_COLOR);
            ctx.set_line_width(1.5);
            ctx.begin_path();
            ctx.move_to(tx(pts[0].x), ty(pts[0].y));
            for p in pts.iter().skip(1) {
                ctx.line_to(tx(p.x), ty(p.y));
            }
            if let Some(ref cur) = cursor_pos.get_untracked() {
                ctx.line_to(tx(cur.x), ty(cur.y));
            }
            ctx.stroke();
            ctx.set_fill_style_str(POLYLINE_VERTEX_COLOR);
            for p in &pts {
                ctx.begin_path();
                let _ = ctx.arc(tx(p.x), ty(p.y), 3.0, 0.0, std::f64::consts::TAU);
                ctx.fill();
            }
        }

        ctx.restore();
    };

    // Reactive repaint whenever shapes, selection, draft, or external trigger change.
    Effect::new(move |_| {
        let _ = shapes.get();
        let _ = selected.get();
        let _ = draft.get();
        let _ = poly_pts.get();
        let _ = cursor_pos.get();
        let _ = grid_size.get();
        let _ = redraw_trigger.get();
        do_redraw();
    });

    // ── Mouse handlers ──────────────────────────────────────────────

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        let tool = active_tool.get_untracked();
        if tool == DrawingTool::Polyline {
            return; // polyline uses click/dblclick
        }
        let p = screen_to_world(ev.client_x() as f64, ev.client_y() as f64);
        set_mouse_down.set(true);
        set_drag_start.set(Some(p.clone()));
        match tool {
            DrawingTool::Line => {
                set_draft.set(Some(SketchShape::Line {
                    p1: p.clone(),
                    p2: p,
                }));
            }
            DrawingTool::Rectangle => {
                set_draft.set(Some(SketchShape::Rectangle {
                    origin: p,
                    width: 0.0,
                    height: 0.0,
                }));
            }
            DrawingTool::Circle => {
                set_draft.set(Some(SketchShape::Circle {
                    center: p,
                    radius: 0.0,
                }));
            }
            DrawingTool::Polyline => {} // handled by click
        }
    };

    let on_mousemove = move |ev: web_sys::MouseEvent| {
        let p = screen_to_world(ev.client_x() as f64, ev.client_y() as f64);
        let tool = active_tool.get_untracked();

        // Rubber-band for polyline
        if tool == DrawingTool::Polyline && !poly_pts.get_untracked().is_empty() {
            set_cursor_pos.set(Some(p));
            return;
        }

        if !mouse_down.get_untracked() {
            return;
        }
        let Some(start) = drag_start.get_untracked() else {
            return;
        };

        match tool {
            DrawingTool::Line => {
                set_draft.set(Some(SketchShape::Line { p1: start, p2: p }));
            }
            DrawingTool::Rectangle => {
                let w = p.x - start.x;
                let h = p.y - start.y;
                let ox = if w < 0.0 { start.x + w } else { start.x };
                let oy = if h < 0.0 { start.y + h } else { start.y };
                set_draft.set(Some(SketchShape::Rectangle {
                    origin: Point::new(ox, oy),
                    width: w.abs(),
                    height: h.abs(),
                }));
            }
            DrawingTool::Circle => {
                let dx = p.x - start.x;
                let dy = p.y - start.y;
                let r = ((dx * dx + dy * dy).sqrt() * 100.0).round() / 100.0;
                set_draft.set(Some(SketchShape::Circle {
                    center: start,
                    radius: r,
                }));
            }
            DrawingTool::Polyline => {}
        }
    };

    let on_mouseup = move |_ev: web_sys::MouseEvent| {
        if !mouse_down.get_untracked() {
            return;
        }
        set_mouse_down.set(false);

        let Some(d) = draft.get_untracked() else {
            return;
        };

        // Reject zero-size shapes.
        let valid = match &d {
            SketchShape::Line { p1, p2 } => p1.x != p2.x || p1.y != p2.y,
            SketchShape::Rectangle { width, height, .. } => *width > 0.0 && *height > 0.0,
            SketchShape::Circle { radius, .. } => *radius > 0.0,
            SketchShape::Polyline { points } => points.len() >= 2,
        };

        if valid {
            set_shapes.update(|v| v.push(d));
        }
        set_draft.set(None);
    };

    // Polyline: click to add, double-click to finish.
    let on_click = move |ev: web_sys::MouseEvent| {
        let tool = active_tool.get_untracked();
        if tool == DrawingTool::Polyline {
            let p = screen_to_world(ev.client_x() as f64, ev.client_y() as f64);
            set_poly_pts.update(|pts| pts.push(p));
            return;
        }

        // Select shape by clicking near it.
        if !mouse_down.get_untracked() && draft.get_untracked().is_none() {
            let p = screen_to_world(ev.client_x() as f64, ev.client_y() as f64);
            let shapes_val = shapes.get_untracked();
            let tolerance = {
                let gs = grid_size.get_untracked();
                if gs > 0.0 {
                    gs
                } else {
                    2.0
                }
            };
            let hit = shapes_val
                .iter()
                .enumerate()
                .rev()
                .find(|(_, s)| crate::sketch::point_in_shape(p.x, p.y, s, tolerance))
                .map(|(i, _)| i);
            set_selected.set(hit);
        }
    };

    let on_dblclick = move |_ev: web_sys::MouseEvent| {
        let tool = active_tool.get_untracked();
        if tool != DrawingTool::Polyline {
            return;
        }
        // Remove the last point (double-click adds one via on_click first).
        set_poly_pts.update(|pts| {
            pts.pop();
        });
        let pts = poly_pts.get_untracked();
        if pts.len() >= 2 {
            set_shapes.update(|v| {
                v.push(SketchShape::Polyline { points: pts });
            });
        }
        set_poly_pts.set(Vec::new());
        set_cursor_pos.set(None);
    };

    // Escape to cancel, Enter to finish polyline.
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        if key == "Escape" {
            set_poly_pts.set(Vec::new());
            set_draft.set(None);
            set_cursor_pos.set(None);
            set_mouse_down.set(false);
        } else if key == "Enter" {
            let tool = active_tool.get_untracked();
            if tool == DrawingTool::Polyline {
                let pts = poly_pts.get_untracked();
                if pts.len() >= 2 {
                    set_shapes.update(|v| {
                        v.push(SketchShape::Polyline { points: pts });
                    });
                }
                set_poly_pts.set(Vec::new());
                set_cursor_pos.set(None);
            }
        }
    };

    view! {
        <canvas
            node_ref=canvas_ref
            style="width:100%;height:100%;display:block;background:#1a1a2e;cursor:crosshair;"
            tabindex="0"
            on:mousedown=on_mousedown
            on:mousemove=on_mousemove
            on:mouseup=on_mouseup
            on:click=on_click
            on:dblclick=on_dblclick
            on:keydown=on_keydown
        />
    }
}

// ── Shape drawing helper ───────────────────────────────────────────

fn draw_shape(
    ctx: &web_sys::CanvasRenderingContext2d,
    shape: &SketchShape,
    color: &str,
    line_width: f64,
    tx: &dyn Fn(f64) -> f64,
    ty: &dyn Fn(f64) -> f64,
    scale: f64,
) {
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(line_width);
    match shape {
        SketchShape::Line { p1, p2 } => {
            ctx.begin_path();
            ctx.move_to(tx(p1.x), ty(p1.y));
            ctx.line_to(tx(p2.x), ty(p2.y));
            ctx.stroke();
        }
        SketchShape::Rectangle {
            origin,
            width,
            height,
        } => {
            ctx.stroke_rect(tx(origin.x), ty(origin.y), *width * scale, *height * scale);
        }
        SketchShape::Circle { center, radius } => {
            ctx.begin_path();
            let _ = ctx.arc(
                tx(center.x),
                ty(center.y),
                *radius * scale,
                0.0,
                std::f64::consts::TAU,
            );
            ctx.stroke();
        }
        SketchShape::Polyline { points } => {
            if points.len() < 2 {
                return;
            }
            ctx.begin_path();
            ctx.move_to(tx(points[0].x), ty(points[0].y));
            for p in points.iter().skip(1) {
                ctx.line_to(tx(p.x), ty(p.y));
            }
            ctx.stroke();
        }
    }
}
