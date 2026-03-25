//! Power monitoring panel for INA230 devices.
//!
//! Displays bus voltage and shunt voltage readings from INA230 power monitors
//! on I2C buses 1, 4, and 8. Auto-polls every 2 seconds using `ReadAllTelemetry`
//! requests. Raw register values are displayed in hex alongside converted units.

use leptos::prelude::*;

use crate::messages::Request;

/// INA230 sensor locations: (bus, address, label).
const POWER_SENSORS: &[(u8, u8, &str)] = &[
    (1, 0x40, "Bus 1 / 0x40"),
    (1, 0x41, "Bus 1 / 0x41"),
    (4, 0x40, "Bus 4 / 0x40"),
    (4, 0x41, "Bus 4 / 0x41"),
    (8, 0x40, "Bus 8 / 0x40"),
    (8, 0x41, "Bus 8 / 0x41"),
];

/// Convert a raw INA230 bus voltage register value to millivolts.
///
/// INA230 bus voltage register has 1.25 mV/LSB resolution.
fn raw_bus_voltage_to_mv(raw: i32) -> i32 {
    // 1.25 mV per LSB = 125 / 100
    raw * 125 / 100
}

/// Format millivolts as a string with appropriate units.
fn format_mv(mv: i32) -> String {
    if mv.unsigned_abs() >= 1000 {
        let whole = mv / 1000;
        let frac = (mv % 1000).unsigned_abs() / 100;
        format!("{whole}.{frac} V")
    } else {
        format!("{mv} mV")
    }
}

/// Power monitoring panel component.
///
/// Displays cards for each INA230 sensor showing bus voltage and power data.
/// Sends periodic `ReadAllTelemetry` requests to poll sensor data.
///
/// The polling interval is leaked (`.forget()`) because `gloo_timers::Interval`
/// is `!Send` and cannot be stored in Leptos cleanup handlers.
#[component]
pub fn PowerPanel(
    /// Reactive signal with raw power values from the telemetry response.
    power: ReadSignal<Vec<i32>>,
    /// Callback to send I2C requests.
    send: impl Fn(Request) + Copy + 'static,
) -> impl IntoView {
    // Auto-poll every 2 seconds
    gloo_timers::callback::Interval::new(2_000, move || {
        send(Request::ReadAllTelemetry);
    })
    .forget();

    // Initial request
    send(Request::ReadAllTelemetry);

    view! {
        <div>
            <h2 class="section-title">"Power Monitors (INA230)"</h2>
            {move || {
                let power_data = power.get();
                if power_data.is_empty() {
                    view! {
                        <div class="loading">"Waiting for telemetry data..."</div>
                    }.into_any()
                } else {
                    view! {
                        <div class="card-grid">
                            {POWER_SENSORS.iter().enumerate().map(|(i, (_bus, _addr, label))| {
                                let raw = power_data.get(i).copied().unwrap_or(0);
                                let mv = raw_bus_voltage_to_mv(raw);
                                let display = format_mv(mv);
                                let raw_hex = format!("0x{:04X}", raw as u16);
                                view! {
                                    <div class="card">
                                        <div class="card-title">{*label}</div>
                                        <div class="card-value">
                                            {display}
                                        </div>
                                        <div class="card-subtitle">{format!("Raw: {raw_hex}")}</div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
