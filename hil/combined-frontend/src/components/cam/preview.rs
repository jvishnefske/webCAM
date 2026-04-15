//! Toolpath preview canvas — renders SimMove data as 2D line segments.
//!
//! Rapid moves are drawn as dashed gray lines; cut moves are solid blue.
//! The view auto-fits to the bounding box with 10 % padding.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::sim_move::{compute_bounds, world_to_canvas, SimMove};

/// Draw the full toolpath onto a `<canvas>` element.
///
/// Re-renders reactively whenever `moves` changes.
#[component]
pub fn CamPreview(moves: ReadSignal<Vec<SimMove>>) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Redraw whenever the signal changes.
    Effect::new(move |_| {
        let data = moves.get();
        if let Some(el) = canvas_ref.get() {
            draw_toolpath(&el, &data);
        }
    });

    view! {
        <div class="card" style="margin-top:0.75rem">
            <div class="card-title">"Toolpath Preview"</div>
            <canvas
                node_ref=canvas_ref
                width="600"
                height="400"
                style="width:100%;border:1px solid #ccc;background:#1e1e2e"
            />
        </div>
    }
}

/// Render all moves onto the canvas.
fn draw_toolpath(canvas: &web_sys::HtmlCanvasElement, moves: &[SimMove]) {
    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    let ctx = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|obj| obj.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
    let ctx = match ctx {
        Some(c) => c,
        None => return,
    };

    // Clear
    ctx.clear_rect(0.0, 0.0, width, height);

    if moves.len() < 2 {
        return;
    }

    let bounds = compute_bounds(moves);
    let padding = width.min(height) * 0.10;

    for i in 1..moves.len() {
        let prev = &moves[i - 1];
        let cur = &moves[i];

        let (x0, y0) = world_to_canvas(prev.x, prev.y, bounds, width, height, padding);
        let (x1, y1) = world_to_canvas(cur.x, cur.y, bounds, width, height, padding);

        ctx.begin_path();
        if cur.rapid {
            ctx.set_stroke_style_str("#888");
            ctx.set_line_dash(&js_sys::Array::of2(
                &wasm_bindgen::JsValue::from(4.0),
                &wasm_bindgen::JsValue::from(4.0),
            ))
            .ok();
            ctx.set_line_width(0.8);
        } else {
            ctx.set_stroke_style_str("#4a9eff");
            ctx.set_line_dash(&js_sys::Array::new()).ok();
            ctx.set_line_width(1.5);
        }
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
        ctx.stroke();
    }

    // Reset dash pattern.
    ctx.set_line_dash(&js_sys::Array::new()).ok();
}
