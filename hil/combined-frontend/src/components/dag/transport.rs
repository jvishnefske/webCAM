//! Transport controls component: Play/Pause, Step, Reset, speed multiplier,
//! dt input, batch run, tick/time display.
//!
//! Pure logic (interval conversion, time formatting) lives in
//! [`crate::sim_util`] so it can be tested on the host target.

use leptos::prelude::*;

use crate::sim_util::{format_sim_time, SPEED_PRESETS};

/// Transport bar with simulation controls.
#[component]
pub fn TransportBar(
    /// Whether the simulation is currently running.
    sim_running: ReadSignal<bool>,
    /// Writer for the running state (used by speed change to restart interval).
    set_sim_running: WriteSignal<bool>,
    /// Current tick count.
    tick_count: ReadSignal<u64>,
    /// Status text to display.
    status: ReadSignal<String>,
    /// Called when Play/Pause is toggled.
    on_play_pause: Callback<()>,
    /// Called when Step is clicked.
    on_step: Callback<()>,
    /// Called when Reset is clicked.
    on_reset: Callback<()>,
    /// Called when Batch run is requested (with step count).
    on_batch_run: Callback<u32>,
) -> impl IntoView {
    // Reserve set_sim_running for future speed-change interval restart.
    let _ = set_sim_running;

    // Local state for the speed multiplier, dt, and batch count
    let (speed, set_speed) = signal(1.0_f64);
    let (dt, set_dt) = signal(0.01_f64);
    let (batch_input, set_batch_input) = signal("100".to_string());

    // Provide speed and dt as context so the editor can read the current interval
    provide_context(speed);
    provide_context(dt);

    view! {
        <div class="dag-toolbar">
            // Play/Pause
            <button
                class=move || if sim_running.get() { "btn btn-danger" } else { "btn btn-primary" }
                on:click=move |_| on_play_pause.run(())
            >
                {move || if sim_running.get() { "Pause" } else { "Play" }}
            </button>
            // Step
            <button class="btn btn-secondary" on:click=move |_| on_step.run(())>
                "Step"
            </button>
            // Reset
            <button class="btn btn-secondary" on:click=move |_| on_reset.run(())>
                "Reset"
            </button>

            // Speed dropdown
            <label class="transport-label">"Speed:"</label>
            <select
                class="transport-select"
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    if let Ok(s) = val.parse::<f64>() {
                        set_speed.set(s);
                    }
                }
            >
                {SPEED_PRESETS.iter().map(|(val, label)| {
                    let selected = *val == 1.0;
                    let val_str = val.to_string();
                    let label_str = label.to_string();
                    view! {
                        <option value=val_str selected=selected>{label_str}</option>
                    }
                }).collect_view()}
            </select>

            // dt input
            <label class="transport-label">"dt:"</label>
            <input
                type="number"
                class="transport-input"
                prop:value=move || format!("{}", dt.get())
                step="0.001"
                min="0.0001"
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    if let Ok(d) = val.parse::<f64>() {
                        if d > 0.0 {
                            set_dt.set(d);
                        }
                    }
                }
            />

            // Batch run
            <input
                type="number"
                class="transport-input transport-batch-input"
                prop:value=move || batch_input.get()
                min="1"
                on:input=move |ev| {
                    set_batch_input.set(event_target_value(&ev));
                }
            />
            <button
                class="btn btn-secondary"
                on:click=move |_| {
                    if let Ok(n) = batch_input.get_untracked().parse::<u32>() {
                        if n > 0 {
                            on_batch_run.run(n);
                        }
                    }
                }
            >
                "Batch"
            </button>

            // Tick count display
            <span class="transport-tick-count">
                {move || format!("Tick: {}", tick_count.get())}
            </span>

            // Simulation time display
            <span class="transport-sim-time">
                {move || {
                    let t = format_sim_time(tick_count.get(), dt.get());
                    format!("t = {}", t)
                }}
            </span>

            // Status text
            <span class="dag-status">{move || status.get()}</span>
        </div>
    }
}
