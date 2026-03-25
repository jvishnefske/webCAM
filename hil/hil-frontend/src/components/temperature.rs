//! Temperature monitoring panel for TMP1075 sensors.
//!
//! Displays a grid of temperature cards across I2C buses 0, 2, 3, and 7.
//! Auto-polls every 2 seconds by sending `ReadAllTelemetry` requests.
//! Raw 16-bit big-endian values are converted to degrees Celsius using
//! the TMP1075 formula: `(raw >> 4) * 625 / 10000`.

use leptos::prelude::*;

use crate::messages::Request;

/// Temperature sensor locations: (bus, address, label).
const TEMP_SENSORS: &[(u8, u8, &str)] = &[
    (0, 0x48, "Bus 0 / 0x48"),
    (0, 0x49, "Bus 0 / 0x49"),
    (2, 0x48, "Bus 2 / 0x48"),
    (2, 0x49, "Bus 2 / 0x49"),
    (3, 0x48, "Bus 3 / 0x48"),
    (3, 0x49, "Bus 3 / 0x49"),
    (7, 0x48, "Bus 7 / 0x48"),
    (7, 0x49, "Bus 7 / 0x49"),
];

/// Convert a raw TMP1075 16-bit register value to millidegrees Celsius.
///
/// The TMP1075 returns a 12-bit two's complement value in bits [15:4].
/// Resolution is 0.0625 degrees per LSB, i.e. 625 millidegrees per 10 LSBs.
fn raw_to_milli_celsius(raw: i32) -> i32 {
    // Shift right 4 to get 12-bit value, multiply by 625, divide by 10
    // to get millidegrees. Integer math only.
    (raw >> 4) * 625 / 10
}

/// Format millidegrees Celsius as a string with one decimal place.
fn format_temp(milli_c: i32) -> String {
    let whole = milli_c / 1000;
    let frac = (milli_c % 1000).unsigned_abs() / 100;
    if milli_c < 0 && whole == 0 {
        format!("-0.{frac}")
    } else {
        format!("{whole}.{frac}")
    }
}

/// Temperature monitoring panel component.
///
/// Displays cards for each TMP1075 sensor. Sends periodic `ReadAllTelemetry`
/// requests to poll sensor data. The `temps` signal is updated by the app
/// layer when telemetry responses arrive.
///
/// The polling interval is leaked (`.forget()`) because `gloo_timers::Interval`
/// is `!Send` and cannot be stored in Leptos cleanup handlers that require
/// `Send + Sync`. This is acceptable because the interval is lightweight and
/// the app is a single-page dashboard.
#[component]
pub fn TemperaturePanel(
    /// Reactive signal with raw temperature values from the telemetry response.
    temps: ReadSignal<Vec<i32>>,
    /// Callback to send I2C requests.
    send: impl Fn(Request) + Copy + 'static,
) -> impl IntoView {
    // Auto-poll every 2 seconds. The interval handle is forgotten because
    // gloo Interval is !Send and cannot be used with on_cleanup.
    gloo_timers::callback::Interval::new(2_000, move || {
        send(Request::ReadAllTelemetry);
    })
    .forget();

    // Send an initial telemetry request
    send(Request::ReadAllTelemetry);

    view! {
        <div>
            <h2 class="section-title">"Temperature Sensors (TMP1075)"</h2>
            {move || {
                let temp_data = temps.get();
                if temp_data.is_empty() {
                    view! {
                        <div class="loading">"Waiting for telemetry data..."</div>
                    }.into_any()
                } else {
                    view! {
                        <div class="card-grid">
                            {TEMP_SENSORS.iter().enumerate().map(|(i, (_bus, _addr, label))| {
                                let raw = temp_data.get(i).copied().unwrap_or(0);
                                let milli_c = raw_to_milli_celsius(raw);
                                let display = format_temp(milli_c);
                                let raw_hex = format!("0x{:04X}", raw as u16);
                                view! {
                                    <div class="card">
                                        <div class="card-title">{*label}</div>
                                        <div class="card-value">
                                            {display}
                                            <span class="card-unit">"C"</span>
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
