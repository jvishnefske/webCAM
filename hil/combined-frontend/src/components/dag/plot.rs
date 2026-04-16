//! Simple plot panel — records topic values over ticks and renders as SVG sparklines.

use std::collections::BTreeMap;

use leptos::prelude::*;

/// Maximum number of data points retained per topic.
const MAX_POINTS: usize = 200;

/// Panel that shows a time-series plot of all pubsub topics.
///
/// On each render it reads the current `topics` signal and appends values to
/// internal history buffers.  Renders one SVG polyline per topic.
#[component]
pub fn PlotPanel(
    /// Reactive signal with current pubsub topics (updated on each tick).
    topics: ReadSignal<BTreeMap<String, f64>>,
    /// Current tick count.
    tick_count: ReadSignal<u64>,
) -> impl IntoView {
    // History: topic name → ring of (tick, value) pairs.
    let (history, set_history) = signal(BTreeMap::<String, Vec<(u64, f64)>>::new());

    // Track the last tick we recorded to avoid duplicate entries.
    let (last_recorded_tick, set_last_recorded_tick) = signal(0_u64);

    // Colors for up to 8 topics.
    let palette = [
        "#3b82f6", // blue
        "#ef4444", // red
        "#10b981", // green
        "#f59e0b", // amber
        "#8b5cf6", // violet
        "#ec4899", // pink
        "#06b6d4", // cyan
        "#f97316", // orange
    ];

    view! {
        <div class="plot-panel">
            <div class="plot-header">"Plot"</div>
            <div class="plot-body">
                {move || {
                    let t = topics.get();
                    let tick = tick_count.get();

                    // Append new data point if tick advanced.
                    if tick > 0 && tick != last_recorded_tick.get_untracked() {
                        set_last_recorded_tick.set(tick);
                        set_history.update(|h| {
                            for (name, val) in &t {
                                let series = h.entry(name.clone()).or_default();
                                series.push((tick, *val));
                                if series.len() > MAX_POINTS {
                                    let excess = series.len() - MAX_POINTS;
                                    series.drain(..excess);
                                }
                            }
                        });
                    }

                    let hist = history.get();

                    if hist.is_empty() {
                        return view! {
                            <div class="plot-empty">"No data yet. Run the simulation to see plots."</div>
                        }.into_any();
                    }

                    // Compute global bounds.
                    let (mut y_min, mut y_max) = (f64::INFINITY, f64::NEG_INFINITY);
                    let (mut x_min, mut x_max) = (u64::MAX, 0_u64);
                    for series in hist.values() {
                        for &(tick_n, val) in series {
                            if val < y_min { y_min = val; }
                            if val > y_max { y_max = val; }
                            if tick_n < x_min { x_min = tick_n; }
                            if tick_n > x_max { x_max = tick_n; }
                        }
                    }
                    // Avoid zero-range.
                    if (y_max - y_min).abs() < 1e-12 { y_min -= 1.0; y_max += 1.0; }
                    if x_max == x_min { x_max = x_min + 1; }

                    let svg_w = 400.0_f64;
                    let svg_h = 180.0_f64;
                    let margin = 4.0_f64;
                    let plot_w = svg_w - 2.0 * margin;
                    let plot_h = svg_h - 2.0 * margin;

                    let x_range = (x_max - x_min) as f64;
                    let y_range = y_max - y_min;

                    let map_x = move |tick_n: u64| -> f64 {
                        margin + (tick_n - x_min) as f64 / x_range * plot_w
                    };
                    let map_y = move |val: f64| -> f64 {
                        margin + plot_h - (val - y_min) / y_range * plot_h
                    };

                    let lines: Vec<_> = hist.iter().enumerate().map(|(i, (name, series))| {
                        let color = palette[i % palette.len()];
                        let points: String = series.iter()
                            .map(|&(tick_n, val)| format!("{:.1},{:.1}", map_x(tick_n), map_y(val)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let legend_label = name.clone();
                        let legend_y = 14 + i * 14;
                        view! {
                            <polyline
                                points=points
                                fill="none"
                                stroke=color
                                stroke-width="1.5"
                            />
                            <text x="8" y=legend_y class="plot-legend" fill=color>
                                {legend_label}
                            </text>
                        }
                    }).collect();

                    let vb = format!("0 0 {} {}", svg_w, svg_h);
                    view! {
                        <svg class="plot-svg" viewBox=vb>
                            // Axes
                            <line x1=margin y1=margin x2=margin y2=svg_h - margin
                                stroke="#555" stroke-width="0.5" />
                            <line x1=margin y1=svg_h - margin x2=svg_w - margin y2=svg_h - margin
                                stroke="#555" stroke-width="0.5" />
                            {lines.into_iter().collect_view()}
                        </svg>
                    }.into_any()
                }}
            </div>
        </div>
    }
}
