//! Manual I2C read/write console component.
//!
//! Provides input fields for bus index, device address, register, and length
//! to perform I2C reads, and bus, address, and hex data for I2C writes.
//! Displays a scrollable log of recent transactions with color-coded
//! request, response, and error entries.

use leptos::prelude::*;

use crate::hex::{parse_hex_bytes, parse_hex_u8};
use crate::messages::Request;

/// Manual I2C read/write console component.
///
/// Provides form inputs for constructing I2C read and write commands,
/// and displays a transaction log showing requests, responses, and errors.
#[component]
pub fn I2cConsole(
    /// Reactive signal containing the console transaction log.
    console_log: ReadSignal<Vec<String>>,
    /// Callback to send I2C requests (also logs them).
    send: impl Fn(Request) + Copy + 'static,
) -> impl IntoView {
    // Read form signals
    let (read_bus, set_read_bus) = signal(String::from("0"));
    let (read_addr, set_read_addr) = signal(String::from("48"));
    let (read_reg, set_read_reg) = signal(String::from("00"));
    let (read_len, set_read_len) = signal(String::from("2"));

    // Write form signals
    let (write_bus, set_write_bus) = signal(String::from("0"));
    let (write_addr, set_write_addr) = signal(String::from("48"));
    let (write_data, set_write_data) = signal(String::from("00"));

    // Error display
    let (form_error, set_form_error) = signal(String::new());

    let on_read = move |_| {
        let bus: u8 = match read_bus.get().parse() {
            Ok(v) if v <= 9 => v,
            _ => {
                set_form_error.set("Bus must be 0-9".to_string());
                return;
            }
        };
        let addr = match parse_hex_u8(&read_addr.get()) {
            Some(v) if v <= 0x7F => v,
            _ => {
                set_form_error.set("Address must be valid 7-bit hex (00-7F)".to_string());
                return;
            }
        };
        let reg = match parse_hex_u8(&read_reg.get()) {
            Some(v) => v,
            None => {
                set_form_error.set("Register must be valid hex byte".to_string());
                return;
            }
        };
        let len: u8 = match read_len.get().parse() {
            Ok(v) if (1..=32).contains(&v) => v,
            _ => {
                set_form_error.set("Length must be 1-32".to_string());
                return;
            }
        };
        set_form_error.set(String::new());
        send(Request::I2cRead {
            bus,
            addr,
            reg,
            len,
        });
    };

    let on_write = move |_| {
        let bus: u8 = match write_bus.get().parse() {
            Ok(v) if v <= 9 => v,
            _ => {
                set_form_error.set("Bus must be 0-9".to_string());
                return;
            }
        };
        let addr = match parse_hex_u8(&write_addr.get()) {
            Some(v) if v <= 0x7F => v,
            _ => {
                set_form_error.set("Address must be valid 7-bit hex (00-7F)".to_string());
                return;
            }
        };
        let data = match parse_hex_bytes(&write_data.get()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                set_form_error.set("Data must be hex bytes (e.g. 'AB CD' or '00ABCD')".to_string());
                return;
            }
        };
        set_form_error.set(String::new());
        send(Request::I2cWrite { bus, addr, data });
    };

    view! {
        <div>
            <h2 class="section-title">"I2C Console"</h2>

            <div class="card-grid-wide">
                // Read panel
                <div class="card">
                    <div class="card-title">"I2C Read"</div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>"Bus (0-9)"</label>
                            <input
                                type="number"
                                min="0"
                                max="9"
                                prop:value=move || read_bus.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_read_bus.set(input.value());
                                }
                            />
                        </div>
                        <div class="form-group">
                            <label>"Address (hex)"</label>
                            <input
                                type="text"
                                placeholder="48"
                                prop:value=move || read_addr.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_read_addr.set(input.value());
                                }
                            />
                        </div>
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>"Register (hex)"</label>
                            <input
                                type="text"
                                placeholder="00"
                                prop:value=move || read_reg.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_read_reg.set(input.value());
                                }
                            />
                        </div>
                        <div class="form-group">
                            <label>"Length (1-32)"</label>
                            <input
                                type="number"
                                min="1"
                                max="32"
                                prop:value=move || read_len.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_read_len.set(input.value());
                                }
                            />
                        </div>
                    </div>
                    <button class="btn btn-primary" on:click=on_read>"Read"</button>
                </div>

                // Write panel
                <div class="card">
                    <div class="card-title">"I2C Write"</div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>"Bus (0-9)"</label>
                            <input
                                type="number"
                                min="0"
                                max="9"
                                prop:value=move || write_bus.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_write_bus.set(input.value());
                                }
                            />
                        </div>
                        <div class="form-group">
                            <label>"Address (hex)"</label>
                            <input
                                type="text"
                                placeholder="48"
                                prop:value=move || write_addr.get()
                                on:input=move |ev| {
                                    use wasm_bindgen::JsCast;
                                    let target = ev.target().expect("target");
                                    let input: web_sys::HtmlInputElement = target.unchecked_into();
                                    set_write_addr.set(input.value());
                                }
                            />
                        </div>
                    </div>
                    <div class="form-group">
                        <label>"Data (hex bytes, e.g. 'AB CD 01')"</label>
                        <input
                            type="text"
                            placeholder="00 FF"
                            prop:value=move || write_data.get()
                            on:input=move |ev| {
                                use wasm_bindgen::JsCast;
                                let target = ev.target().expect("target");
                                let input: web_sys::HtmlInputElement = target.unchecked_into();
                                set_write_data.set(input.value());
                            }
                        />
                    </div>
                    <button class="btn btn-primary" on:click=on_write>"Write"</button>
                </div>
            </div>

            // Error display
            {move || {
                let err = form_error.get();
                if err.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! { <div class="warning-box">{err}</div> }.into_any()
                }
            }}

            // Transaction log
            <h3 class="section-title" style="margin-top: 1rem;">"Transaction Log"</h3>
            <div class="console-output">
                {move || {
                    let log = console_log.get();
                    if log.is_empty() {
                        view! {
                            <div class="console-entry">"No transactions yet."</div>
                        }.into_any()
                    } else {
                        view! {
                            <div>
                                {log.iter().rev().map(|entry| {
                                    let css_class = if entry.starts_with("[REQ]") {
                                        "console-entry console-entry-req"
                                    } else if entry.starts_with("[RESP]") {
                                        "console-entry console-entry-resp"
                                    } else {
                                        "console-entry console-entry-err"
                                    };
                                    let text = entry.clone();
                                    view! {
                                        <div class={css_class}>{text}</div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
