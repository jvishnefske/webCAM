//! Simulation playback component — animated progressive drawing of moves.
//!
//! Features:
//! - Play / Pause / Reset buttons
//! - Speed slider (0.5x to 4x)
//! - Progress display ("Move 45 / 120")
//! - Tool-position circle in the current location

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::sim_move::{compute_bounds, world_to_canvas, SimMove};

/// Animated simulation playback of a sequence of [`SimMove`]s.
#[component]
pub fn CamSimulation(moves: ReadSignal<Vec<SimMove>>) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Playback state.
    let (current_idx, set_current_idx) = signal(0usize);
    let (playing, set_playing) = signal(false);
    let (speed, set_speed) = signal(1.0f64);

    // Shared handle to the gloo interval so we can stop it.
    let interval_handle: Rc<RefCell<Option<gloo_timers::callback::Interval>>> =
        Rc::new(RefCell::new(None));

    // Reset when the move data changes.
    {
        let ih = Rc::clone(&interval_handle);
        Effect::new(move |_| {
            let _data = moves.get(); // subscribe
            set_current_idx.set(0);
            set_playing.set(false);
            ih.borrow_mut().take(); // cancel running interval
        });
    }

    // Redraw whenever current_idx, moves, or speed changes.
    Effect::new(move |_| {
        let data = moves.get();
        let idx = current_idx.get();
        if let Some(el) = canvas_ref.get() {
            draw_sim_frame(&el, &data, idx);
        }
    });

    // --- Play / Pause ---
    let ih_play = Rc::clone(&interval_handle);
    let on_play_pause = move |_| {
        let is_playing = playing.get();
        if is_playing {
            // Pause
            set_playing.set(false);
            ih_play.borrow_mut().take();
        } else {
            // Play
            let total = moves.get().len();
            if total == 0 {
                return;
            }
            // If at the end, restart.
            if current_idx.get() >= total.saturating_sub(1) {
                set_current_idx.set(0);
            }
            set_playing.set(true);

            let ih_inner = Rc::clone(&ih_play);
            let interval = gloo_timers::callback::Interval::new(33, move || {
                let spd = speed.get_untracked();
                let steps = (spd * 2.0).max(1.0) as usize;
                let total = moves.get_untracked().len();
                set_current_idx.update(|idx| {
                    *idx = (*idx + steps).min(total.saturating_sub(1));
                    if *idx >= total.saturating_sub(1) {
                        set_playing.set(false);
                        ih_inner.borrow_mut().take();
                    }
                });
            });
            *ih_play.borrow_mut() = Some(interval);
        }
    };

    // --- Reset ---
    let ih_reset = Rc::clone(&interval_handle);
    let on_reset = move |_| {
        set_playing.set(false);
        set_current_idx.set(0);
        ih_reset.borrow_mut().take();
    };

    view! {
        <div class="card" style="margin-top:0.75rem">
            <div class="card-title">"Simulation"</div>

            <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.5rem;flex-wrap:wrap">
                <button class="btn btn-primary" on:click=on_play_pause>
                    {move || if playing.get() { "Pause" } else { "Play" }}
                </button>
                <button class="btn" on:click=on_reset>"Reset"</button>

                <label style="margin-left:0.5rem">"Speed:"</label>
                <input
                    type="range"
                    min="0.5"
                    max="4"
                    step="0.5"
                    prop:value=move || speed.get().to_string()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                            set_speed.set(v);
                        }
                    }
                    style="width:100px"
                />
                <span style="min-width:2.5rem">{move || format!("{:.1}x", speed.get())}</span>

                <span class="card-subtitle" style="margin-left:auto">
                    {move || {
                        let total = moves.get().len();
                        let idx = current_idx.get();
                        format!("Move {idx} / {total}")
                    }}
                </span>
            </div>

            <canvas
                node_ref=canvas_ref
                width="600"
                height="400"
                style="width:100%;border:1px solid #ccc;background:#1e1e2e"
            />
        </div>
    }
}

/// Draw the simulation frame up to `idx`, including the tool-position circle.
fn draw_sim_frame(canvas: &web_sys::HtmlCanvasElement, moves: &[SimMove], idx: usize) {
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

    ctx.clear_rect(0.0, 0.0, width, height);

    if moves.is_empty() {
        return;
    }

    let bounds = compute_bounds(moves);
    let padding = width.min(height) * 0.10;

    // Draw segments up to `idx`.
    let draw_up_to = idx.min(moves.len() - 1);

    for i in 1..=draw_up_to {
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
    ctx.set_line_dash(&js_sys::Array::new()).ok();

    // Draw tool-position circle.
    let current = &moves[draw_up_to.min(moves.len() - 1)];
    let (cx, cy) = world_to_canvas(current.x, current.y, bounds, width, height, padding);
    let radius = 5.0;

    // Outer glow
    ctx.begin_path();
    ctx.arc(cx, cy, radius + 2.0, 0.0, std::f64::consts::TAU)
        .ok();
    ctx.set_fill_style_str("rgba(255,255,255,0.2)");
    ctx.fill();

    // Tool circle
    ctx.begin_path();
    ctx.arc(cx, cy, radius, 0.0, std::f64::consts::TAU).ok();
    if current.rapid {
        ctx.set_fill_style_str("#fbbf24"); // amber for rapid
    } else {
        ctx.set_fill_style_str("#ef4444"); // red for cutting
    }
    ctx.fill();
    ctx.set_stroke_style_str("#fff");
    ctx.set_line_width(1.0);
    ctx.stroke();
}
