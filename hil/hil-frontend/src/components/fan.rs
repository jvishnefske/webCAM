//! Fan controller panel for EMC2305 on I2C bus 7.
//!
//! Provides PWM duty cycle sliders for 5 fan channels and displays RPM
//! readback values. Slider changes send I2C write commands to set PWM
//! duty registers. RPM values are read from tachometer registers.

use leptos::prelude::*;

use crate::messages::Request;

/// Number of fan channels on the EMC2305.
const FAN_CHANNELS: usize = 5;

/// I2C bus index for the EMC2305 fan controller.
const FAN_BUS: u8 = 7;

/// I2C address of the EMC2305 fan controller.
const FAN_ADDR: u8 = 0x2E;

/// EMC2305 PWM duty cycle register base address.
///
/// Fan 1 PWM is at 0x30, Fan 2 at 0x40, Fan 3 at 0x50, Fan 4 at 0x60, Fan 5 at 0x70.
const PWM_REG_BASE: [u8; FAN_CHANNELS] = [0x30, 0x40, 0x50, 0x60, 0x70];

/// Fan controller panel component.
///
/// Displays 5 fan channels with PWM duty sliders (0-255) and RPM readback.
/// Slider changes send I2C write commands. RPM is polled from the telemetry
/// stream.
///
/// The polling interval is leaked (`.forget()`) because `gloo_timers::Interval`
/// is `!Send` and cannot be stored in Leptos cleanup handlers.
#[component]
pub fn FanPanel(
    /// Reactive signal with fan RPM values from the telemetry response.
    fans: ReadSignal<Vec<i32>>,
    /// Callback to send I2C requests.
    send: impl Fn(Request) + Copy + 'static,
) -> impl IntoView {
    // Per-channel PWM duty signals
    let pwm_signals: Vec<(ReadSignal<u8>, WriteSignal<u8>)> =
        (0..FAN_CHANNELS).map(|_| signal(128u8)).collect();

    // Auto-poll every 2 seconds for RPM readback
    gloo_timers::callback::Interval::new(2_000, move || {
        send(Request::ReadAllTelemetry);
    })
    .forget();

    send(Request::ReadAllTelemetry);

    view! {
        <div>
            <h2 class="section-title">"Fan Controller (EMC2305 - Bus 7)"</h2>
            <div class="card-grid-wide">
                {pwm_signals.into_iter().enumerate().map(|(ch, (pwm_read, pwm_write))| {
                    let ch_label = format!("Fan {}", ch + 1);
                    let pwm_reg = PWM_REG_BASE[ch];

                    let on_slider = move |ev: leptos::ev::Event| {
                        use wasm_bindgen::JsCast;
                        let target = ev.target().expect("event target");
                        let input: web_sys::HtmlInputElement = target.unchecked_into();
                        let val: u8 = input.value().parse().unwrap_or(128);
                        pwm_write.set(val);
                        // Send I2C write: [register, value]
                        send(Request::I2cWrite {
                            bus: FAN_BUS,
                            addr: FAN_ADDR,
                            data: vec![pwm_reg, val],
                        });
                    };

                    view! {
                        <div class="card">
                            <div class="card-title">{ch_label}</div>
                            <div class="fan-channel">
                                <div class="slider-container">
                                    <label>"PWM:"</label>
                                    <input
                                        type="range"
                                        min="0"
                                        max="255"
                                        prop:value=move || pwm_read.get().to_string()
                                        on:input=on_slider
                                    />
                                    <span class="slider-value">{move || format!("{}", pwm_read.get())}</span>
                                </div>
                                <div class="fan-rpm">
                                    {move || {
                                        let fan_data = fans.get();
                                        let rpm = fan_data.get(ch).copied().unwrap_or(0);
                                        format!("RPM: {rpm}")
                                    }}
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
