//! Canvas-based scrolling series plot component.
//!
//! Renders multiple time-series as polylines on an HTML5 `<canvas>`,
//! keeping a scrolling history buffer of the last [`MAX_HISTORY`] samples
//! per series.  Pure-logic helpers live in [`crate::plot_math`] so they
//! are testable on native (non-wasm) targets.

use std::collections::VecDeque;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::plot_math::{
    compute_y_bounds, format_axis_label, grid_lines, value_to_canvas_y, MAX_HISTORY, PAD,
    SERIES_COLORS,
};

/// Canvas-based scrolling plot panel.
///
/// Accepts a reactive signal of series data (`Vec<Vec<f64>>`). On every
/// change the component appends the latest sample of each series to an
/// internal history buffer and redraws the canvas.
#[component]
pub fn PlotPanel(
    /// Series data to plot -- updated on each tick.
    series_data: ReadSignal<Vec<Vec<f64>>>,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let (history, set_history) = signal(Vec::<VecDeque<f64>>::new());

    // Track series_data changes and update history + redraw.
    Effect::new(move |_| {
        let data = series_data.get();

        // Update history buffer.
        set_history.update(|hist| {
            // Grow history vec if new series appeared.
            while hist.len() < data.len() {
                hist.push(VecDeque::with_capacity(MAX_HISTORY));
            }

            for (i, series) in data.iter().enumerate() {
                if let Some(last) = series.last() {
                    let dq = &mut hist[i];
                    dq.push_back(*last);
                    if dq.len() > MAX_HISTORY {
                        dq.pop_front();
                    }
                }
            }
        });

        // Redraw.
        let Some(canvas_el) = canvas_ref.get() else {
            return;
        };
        let canvas: &web_sys::HtmlCanvasElement = canvas_el
            .dyn_ref::<web_sys::HtmlCanvasElement>()
            .expect("NodeRef should be an HtmlCanvasElement");

        let rect = canvas.get_bounding_client_rect();
        let w = rect.width();
        let h = rect.height();
        if w < 1.0 || h < 1.0 {
            return;
        }

        // High-DPI support.
        let dpr = web_sys::window()
            .map(|win| win.device_pixel_ratio())
            .unwrap_or(1.0);
        canvas.set_width((w * dpr) as u32);
        canvas.set_height((h * dpr) as u32);

        let ctx = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|obj| obj.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
        let Some(ctx) = ctx else { return };

        ctx.save();
        let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        // Background
        ctx.set_fill_style_str("#1e293b"); // slate-800
        ctx.fill_rect(0.0, 0.0, w, h);

        let hist = history.get();
        let refs: Vec<&VecDeque<f64>> = hist.iter().collect();

        let max_len = refs.iter().map(|dq| dq.len()).max().unwrap_or(0);
        if max_len < 2 {
            ctx.set_fill_style_str("#94a3b8");
            ctx.set_font("12px monospace");
            let _ = ctx.fill_text("Waiting for data...", PAD, h / 2.0);
            ctx.restore();
            return;
        }

        let plot_w = w - PAD * 2.0;
        let plot_h = h - PAD * 2.0;
        let (y_min, y_max) = compute_y_bounds(&refs);

        // Grid lines.
        let grid_count = 5;
        let grid_vals = grid_lines(y_min, y_max, grid_count);
        ctx.set_stroke_style_str("#334155"); // slate-700
        ctx.set_line_width(0.5);
        ctx.set_fill_style_str("#94a3b8"); // slate-400
        ctx.set_font("10px monospace");
        ctx.set_text_align("right");
        for gv in &grid_vals {
            let cy = PAD + value_to_canvas_y(*gv, y_min, y_max, plot_h);
            ctx.begin_path();
            ctx.move_to(PAD, cy);
            ctx.line_to(w - PAD, cy);
            ctx.stroke();
            let _ = ctx.fill_text(&format_axis_label(*gv), PAD - 4.0, cy + 3.0);
        }

        // Axes
        ctx.set_stroke_style_str("#475569"); // slate-600
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(PAD, PAD);
        ctx.line_to(PAD, h - PAD);
        ctx.line_to(w - PAD, h - PAD);
        ctx.stroke();

        // X-axis labels (sample indices).
        ctx.set_text_align("center");
        ctx.set_fill_style_str("#94a3b8");
        let x_label_count = 5usize;
        for i in 0..x_label_count {
            let sample_idx = if max_len > 1 {
                i * (max_len - 1) / (x_label_count - 1)
            } else {
                0
            };
            let x = PAD + (i as f64 / (x_label_count - 1) as f64) * plot_w;
            let _ = ctx.fill_text(&sample_idx.to_string(), x, h - PAD + 14.0);
        }

        // Draw each series polyline.
        for (si, dq) in refs.iter().enumerate() {
            if dq.len() < 2 {
                continue;
            }
            let color = SERIES_COLORS[si % SERIES_COLORS.len()];
            ctx.set_stroke_style_str(color);
            ctx.set_line_width(1.5);
            ctx.begin_path();
            for (j, &val) in dq.iter().enumerate() {
                let x = PAD + (j as f64 / (dq.len() - 1) as f64) * plot_w;
                let y = PAD + value_to_canvas_y(val, y_min, y_max, plot_h);
                if j == 0 {
                    ctx.move_to(x, y);
                } else {
                    ctx.line_to(x, y);
                }
            }
            ctx.stroke();
        }

        ctx.restore();
    });

    view! {
        <canvas
            node_ref=canvas_ref
            style="width:100%; height:200px; display:block;"
        />
    }
}
