//! Root application component for the HIL dashboard.
//!
//! Manages the WebSocket connection lifecycle, holds top-level reactive signals
//! for connection state, bus topology, and telemetry data, and renders the
//! main layout with navigation tabs and content panels.
//!
//! The WebSocket client is stored in a thread-local (see [`ws_client`]) because
//! `web_sys::WebSocket` is `!Send` and cannot be placed in Leptos signals.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;

use crate::components::bus_overview::BusOverview;
use crate::components::fan::FanPanel;
use crate::components::firmware_update::FirmwareUpdatePanel;
use crate::components::i2c_console::I2cConsole;
use crate::components::power::PowerPanel;
use crate::components::temperature::TemperaturePanel;
use crate::messages::{BusEntry, Request, Response};
use crate::ws_client::{self, ConnState};

/// The active tab in the dashboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    /// Overview of all I2C buses and their devices.
    Overview,
    /// Temperature sensor readings.
    Temperature,
    /// Power monitoring readings.
    Power,
    /// Fan controller panel.
    Fans,
    /// Manual I2C read/write console.
    Console,
    /// Firmware update utilities.
    Firmware,
}

/// Root application component.
///
/// Establishes the WebSocket connection, manages reconnection, and renders
/// the full dashboard layout with header, navigation tabs, and content panels.
#[component]
pub fn App() -> impl IntoView {
    // Connection state - Send+Sync OK since ConnState is Copy
    let (conn_state, set_conn_state) = signal(ConnState::Closed);

    // Data signals - all contain Send+Sync types
    let (buses, set_buses) = signal(Vec::<BusEntry>::new());
    let (temps, set_temps) = signal(Vec::<i32>::new());
    let (power, set_power) = signal(Vec::<i32>::new());
    let (fans, set_fans) = signal(Vec::<i32>::new());
    let (console_log, set_console_log) = signal(Vec::<String>::new());
    let (fw_response, set_fw_response) = signal(Option::<Response>::None);

    // Active tab
    let (active_tab, set_active_tab) = signal(Tab::Overview);

    let ws_url = "ws://169.254.1.61:8080";

    // Response handler - updates signals when messages arrive from the Pico
    let handle_response = move |resp: Response| match resp {
        Response::BusList { buses: b } => {
            set_buses.set(b);
        }
        Response::Telemetry {
            temps: t,
            power: p,
            fans: f,
        } => {
            set_temps.set(t);
            set_power.set(p);
            set_fans.set(f);
        }
        Response::I2cData { data } => {
            let hex: Vec<String> = data.iter().map(|b| format!("{b:02X}")).collect();
            let entry = format!("[RESP] data: {}", hex.join(" "));
            set_console_log.update(|log| log.push(entry));
        }
        Response::WriteOk => {
            set_console_log.update(|log| log.push("[RESP] Write OK".to_string()));
        }
        Response::Error { message } => {
            let entry = format!("[ERR] {message}");
            set_console_log.update(|log| log.push(entry));
        }
        resp @ (Response::FwReady { .. }
        | Response::FwChunkAck { .. }
        | Response::FwFinishAck
        | Response::FwMarkBootedAck) => {
            set_fw_response.set(Some(resp));
        }
    };

    // Backoff tracking - thread-local Rc because it is used in the closure
    // chain for reconnection.
    let backoff: Rc<RefCell<u32>> = Rc::new(RefCell::new(ws_client::initial_backoff()));

    // Build a self-referential reconnect chain.
    // `do_connect` is an Rc so the on_close callback can re-invoke it.
    type ConnectFn = Rc<RefCell<Option<Rc<dyn Fn()>>>>;
    let do_connect: ConnectFn = Rc::new(RefCell::new(None));

    {
        let do_connect_inner = Rc::clone(&do_connect);
        let backoff = Rc::clone(&backoff);
        let url = ws_url.to_string();

        let connect_fn = Rc::new(move || {
            let do_connect_for_close = Rc::clone(&do_connect_inner);
            let backoff_for_close = Rc::clone(&backoff);

            let on_close = move || {
                let delay = *backoff_for_close.borrow();
                let next = ws_client::next_backoff(delay);
                *backoff_for_close.borrow_mut() = next;
                let do_connect_timer = Rc::clone(&do_connect_for_close);
                gloo_timers::callback::Timeout::new(delay, move || {
                    if let Some(f) = do_connect_timer.borrow().as_ref() {
                        f();
                    }
                })
                .forget();
            };

            ws_client::connect(&url, set_conn_state, handle_response, on_close);

            // Request bus list on new connection
            let _ = ws_client::send_request(&Request::ListBuses);
        }) as Rc<dyn Fn()>;

        *do_connect.borrow_mut() = Some(connect_fn);
    }

    // Initial connection
    if let Some(f) = do_connect.borrow().as_ref() {
        f();
    }

    // Send helper for child components - pure function, no non-Send captures
    let send = move |req: Request| {
        if let Err(e) = ws_client::send_request(&req) {
            set_console_log.update(|log| log.push(format!("[ERR] Send failed: {e}")));
        }
    };

    // Send that also logs the request
    let send_logged = move |req: Request| {
        let desc = format!("[REQ] {req:?}");
        set_console_log.update(|log| log.push(desc));
        send(req);
    };

    let conn_class = move || match conn_state.get() {
        ConnState::Connecting => "status-indicator status-connecting",
        ConnState::Open => "status-indicator status-connected",
        ConnState::Closed => "status-indicator status-disconnected",
    };

    let conn_text = move || match conn_state.get() {
        ConnState::Connecting => "Connecting...",
        ConnState::Open => "Connected",
        ConnState::Closed => "Disconnected",
    };

    let tab_class = move |tab: Tab| {
        if active_tab.get() == tab {
            "nav-tab active"
        } else {
            "nav-tab"
        }
    };

    view! {
        <div class="header">
            <h1>"HIL Dashboard"</h1>
            <div class={conn_class}>
                <span class="status-dot"></span>
                <span>{conn_text}</span>
            </div>
        </div>

        <nav class="nav-tabs">
            <button class={move || tab_class(Tab::Overview)} on:click=move |_| set_active_tab.set(Tab::Overview)>"Buses"</button>
            <button class={move || tab_class(Tab::Temperature)} on:click=move |_| set_active_tab.set(Tab::Temperature)>"Temperature"</button>
            <button class={move || tab_class(Tab::Power)} on:click=move |_| set_active_tab.set(Tab::Power)>"Power"</button>
            <button class={move || tab_class(Tab::Fans)} on:click=move |_| set_active_tab.set(Tab::Fans)>"Fans"</button>
            <button class={move || tab_class(Tab::Console)} on:click=move |_| set_active_tab.set(Tab::Console)>"I2C Console"</button>
            <button class={move || tab_class(Tab::Firmware)} on:click=move |_| set_active_tab.set(Tab::Firmware)>"Firmware"</button>
        </nav>

        <div class="main-content">
            {move || match active_tab.get() {
                Tab::Overview => view! {
                    <BusOverview buses=buses />
                }.into_any(),
                Tab::Temperature => view! {
                    <TemperaturePanel temps=temps send=send />
                }.into_any(),
                Tab::Power => view! {
                    <PowerPanel power=power send=send />
                }.into_any(),
                Tab::Fans => view! {
                    <FanPanel fans=fans send=send />
                }.into_any(),
                Tab::Console => view! {
                    <I2cConsole console_log=console_log send=send_logged />
                }.into_any(),
                Tab::Firmware => view! {
                    <FirmwareUpdatePanel send=send fw_response=fw_response />
                }.into_any(),
            }}
        </div>
    }
}
