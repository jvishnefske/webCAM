//! I2C console — manual read/write with log output.

use leptos::prelude::*;
use crate::app::AppContext;
use crate::hex;
use crate::messages::Request;

#[component]
pub fn I2cConsole() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let (read_bus, set_read_bus) = signal("0".to_string());
    let (read_addr, set_read_addr) = signal("48".to_string());
    let (read_reg, set_read_reg) = signal("00".to_string());
    let (read_len, set_read_len) = signal("2".to_string());

    let (write_bus, set_write_bus) = signal("0".to_string());
    let (write_addr, set_write_addr) = signal("48".to_string());
    let (write_data, set_write_data) = signal(String::new());

    let (form_error, set_form_error) = signal(String::new());

    let ctx_read = ctx.clone();
    let on_read = move |_| {
        let bus: u8 = read_bus.get().parse().unwrap_or(0);
        let addr = match hex::parse_hex_u8(&read_addr.get()) {
            Some(a) => a,
            None => { set_form_error.set("Invalid address".into()); return; }
        };
        let reg = match hex::parse_hex_u8(&read_reg.get()) {
            Some(r) => r,
            None => { set_form_error.set("Invalid register".into()); return; }
        };
        let len: u8 = read_len.get().parse().unwrap_or(1);
        set_form_error.set(String::new());
        ctx_read.send_logged(Request::I2cRead { bus, addr, reg, len });
    };

    let ctx_write = ctx.clone();
    let on_write = move |_| {
        let bus: u8 = write_bus.get().parse().unwrap_or(0);
        let addr = match hex::parse_hex_u8(&write_addr.get()) {
            Some(a) => a,
            None => { set_form_error.set("Invalid address".into()); return; }
        };
        let data = match hex::parse_hex_bytes(&write_data.get()) {
            Some(d) => d,
            None => { set_form_error.set("Invalid hex data".into()); return; }
        };
        set_form_error.set(String::new());
        ctx_write.send_logged(Request::I2cWrite { bus, addr, data });
    };

    view! {
        <h2 class="section-title">"I2C Console"</h2>
        <div class="card-grid-wide">
            <div class="card">
                <div class="card-title">"Read"</div>
                <div class="form-row">
                    <div class="form-group">
                        <label>"Bus"</label>
                        <input type="number"
                            prop:value=move || read_bus.get()
                            on:input=move |ev| set_read_bus.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Addr (hex)"</label>
                        <input type="text"
                            prop:value=move || read_addr.get()
                            on:input=move |ev| set_read_addr.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Reg (hex)"</label>
                        <input type="text"
                            prop:value=move || read_reg.get()
                            on:input=move |ev| set_read_reg.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Len"</label>
                        <input type="number"
                            prop:value=move || read_len.get()
                            on:input=move |ev| set_read_len.set(event_target_value(&ev))
                        />
                    </div>
                </div>
                <button class="btn btn-primary" on:click=on_read>"Read"</button>
            </div>

            <div class="card">
                <div class="card-title">"Write"</div>
                <div class="form-row">
                    <div class="form-group">
                        <label>"Bus"</label>
                        <input type="number"
                            prop:value=move || write_bus.get()
                            on:input=move |ev| set_write_bus.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Addr (hex)"</label>
                        <input type="text"
                            prop:value=move || write_addr.get()
                            on:input=move |ev| set_write_addr.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Data (hex)"</label>
                        <input type="text" placeholder="AB CD 01"
                            prop:value=move || write_data.get()
                            on:input=move |ev| set_write_data.set(event_target_value(&ev))
                        />
                    </div>
                </div>
                <button class="btn btn-primary" on:click=on_write>"Write"</button>
            </div>
        </div>

        {move || {
            let err = form_error.get();
            if err.is_empty() {
                view! { <span></span> }.into_any()
            } else {
                view! { <div class="warning-box">{err}</div> }.into_any()
            }
        }}

        <h3 class="section-title" style="margin-top:1rem">"Log"</h3>
        <div class="console-output">
            {move || {
                ctx.console_log.get().into_iter().map(|entry| {
                    let class = if entry.starts_with("[REQ]") {
                        "console-entry console-entry-req"
                    } else if entry.starts_with("[RESP]") {
                        "console-entry console-entry-resp"
                    } else {
                        "console-entry console-entry-err"
                    };
                    view! { <div class=class>{entry}</div> }
                }).collect_view()
            }}
        </div>
    }
}
