//! Telemetry panel — temperature, power, and fan readings.

use leptos::prelude::*;
use crate::app::AppContext;
use crate::messages::Request;

fn raw_to_milli_celsius(raw: i32) -> f64 {
    raw as f64 * 62.5 / 1000.0
}

fn format_temp(raw: i32) -> String {
    format!("{:.1}", raw_to_milli_celsius(raw))
}

fn raw_bus_voltage_to_mv(raw: i32) -> i32 {
    raw * 125 / 100
}

fn format_mv(raw: i32) -> String {
    let mv = raw_bus_voltage_to_mv(raw);
    format!("{}.{:03}", mv / 1000, mv % 1000)
}

#[component]
pub fn TelemetryPanel() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    // Poll telemetry every 2 seconds
    let send = ctx.send.clone();
    gloo_timers::callback::Interval::new(2_000, move || {
        send.call(Request::ReadAllTelemetry);
    })
    .forget();

    view! {
        <h2 class="section-title">"Telemetry"</h2>
        <div class="card-grid">
            // Temperature cards
            {move || {
                ctx.temps.get().into_iter().enumerate().map(|(i, raw)| {
                    view! {
                        <div class="card">
                            <div class="card-title">{format!("Temp {i}")}</div>
                            <span class="card-value">{format_temp(raw)}</span>
                            <span class="card-unit">"C"</span>
                        </div>
                    }
                }).collect_view()
            }}
            // Power cards
            {move || {
                ctx.power.get().into_iter().enumerate().map(|(i, raw)| {
                    view! {
                        <div class="card">
                            <div class="card-title">{format!("Power {i}")}</div>
                            <span class="card-value">{format_mv(raw)}</span>
                            <span class="card-unit">"V"</span>
                        </div>
                    }
                }).collect_view()
            }}
            // Fan cards
            {move || {
                ctx.fans.get().into_iter().enumerate().map(|(i, raw)| {
                    view! {
                        <div class="card">
                            <div class="card-title">{format!("Fan {i}")}</div>
                            <span class="card-value">{format!("{raw}")}</span>
                            <span class="card-unit">"RPM"</span>
                        </div>
                    }
                }).collect_view()
            }}
        </div>
    }
}
