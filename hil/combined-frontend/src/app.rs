//! Root application component with tab navigation and shared state.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;

use crate::backoff;
use crate::components::header::Header;
use crate::components::tab_bar::TabBar;
use crate::messages::{BusEntry, Request, Response};
use crate::ws_client::{self, ConnState};

// ---------------------------------------------------------------------------
// Tab enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Buses,
    Telemetry,
    Console,
    Firmware,
    DagEditor,
    Deploy,
}

// ---------------------------------------------------------------------------
// AppContext — shared state via Leptos context
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppContext {
    // Connection
    pub conn_state: ReadSignal<ConnState>,
    // HIL data
    pub buses: ReadSignal<Vec<BusEntry>>,
    pub temps: ReadSignal<Vec<i32>>,
    pub power: ReadSignal<Vec<i32>>,
    pub fans: ReadSignal<Vec<i32>>,
    pub console_log: ReadSignal<Vec<String>>,
    pub set_console_log: WriteSignal<Vec<String>>,
    pub fw_response: ReadSignal<Option<Response>>,
    // Tab
    pub active_tab: ReadSignal<Tab>,
    pub set_active_tab: WriteSignal<Tab>,
    // Request queue — components push requests here, app drains and sends
    pub request_tx: WriteSignal<Vec<Request>>,
}

impl AppContext {
    /// Queue a request for sending over WebSocket.
    pub fn send(&self, req: Request) {
        self.request_tx.update(|q| q.push(req));
    }

    /// Queue a request and log it to the console.
    pub fn send_logged(&self, req: Request) {
        let label = format!("[REQ] {:?}", req);
        self.set_console_log.update(|log| log.push(label));
        self.send(req);
    }
}

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

#[component]
pub fn App() -> impl IntoView {
    // -- HIL signals --
    let (conn_state, set_conn_state) = signal(ConnState::Closed);
    let (buses, set_buses) = signal(Vec::<BusEntry>::new());
    let (temps, set_temps) = signal(Vec::<i32>::new());
    let (power, set_power) = signal(Vec::<i32>::new());
    let (fans, set_fans) = signal(Vec::<i32>::new());
    let (console_log, set_console_log) = signal(Vec::<String>::new());
    let (fw_response, set_fw_response) = signal(None::<Response>);
    let (active_tab, set_active_tab) = signal(Tab::Buses);

    // -- Request queue (safe: Vec<Request> is Send+Sync) --
    let (request_rx, request_tx) = signal(Vec::<Request>::new());

    // Drain the request queue and send via WebSocket
    Effect::new(move |_| {
        let pending = request_rx.get();
        if !pending.is_empty() {
            for req in &pending {
                let _ = ws_client::send_request(req);
            }
            request_tx.set(Vec::new());
        }
    });

    // -- Response handler --
    let handle_response = move |resp: Response| match &resp {
        Response::BusList { buses: b } => set_buses.set(b.clone()),
        Response::Telemetry {
            temps: t,
            power: p,
            fans: f,
        } => {
            set_temps.set(t.clone());
            set_power.set(p.clone());
            set_fans.set(f.clone());
        }
        Response::I2cData { data } => {
            let hex: Vec<String> = data.iter().map(|b| format!("{b:02X}")).collect();
            set_console_log.update(|log| {
                log.push(format!("[RESP] data: {}", hex.join(" ")));
            });
        }
        Response::WriteOk => {
            set_console_log.update(|log| {
                log.push("[RESP] Write OK".to_string());
            });
        }
        Response::Error { message: msg } => {
            set_console_log.update(|log| {
                log.push(format!("[ERR] {msg}"));
            });
            set_fw_response.set(Some(resp.clone()));
        }
        Response::FwReady { .. }
        | Response::FwChunkAck { .. }
        | Response::FwFinishAck
        | Response::FwMarkBootedAck => {
            set_fw_response.set(Some(resp.clone()));
        }
    };

    // -- WebSocket connect with reconnect --
    type ConnectFn = Rc<RefCell<Option<Rc<dyn Fn()>>>>;
    let do_connect: ConnectFn = Rc::new(RefCell::new(None));
    let backoff_ms = Rc::new(RefCell::new(backoff::initial_backoff()));

    let connect_fn: Rc<dyn Fn()> = {
        let do_connect = Rc::clone(&do_connect);
        let backoff_ms = Rc::clone(&backoff_ms);
        Rc::new(move || {
            let do_connect = Rc::clone(&do_connect);
            let backoff_ms_rc = Rc::clone(&backoff_ms);
            let on_close = move || {
                let delay = *backoff_ms_rc.borrow();
                *backoff_ms_rc.borrow_mut() = backoff::next_backoff(delay);
                let dc = Rc::clone(&do_connect);
                gloo_timers::callback::Timeout::new(delay, move || {
                    if let Some(f) = dc.borrow().as_ref() {
                        f();
                    }
                })
                .forget();
            };
            ws_client::connect(
                "ws://169.254.1.61:8080",
                set_conn_state,
                handle_response,
                on_close,
            );
        })
    };

    *do_connect.borrow_mut() = Some(connect_fn);
    if let Some(f) = do_connect.borrow().as_ref() {
        f();
    }

    // -- Provide context --
    let ctx = AppContext {
        conn_state,
        buses,
        temps,
        power,
        fans,
        console_log,
        set_console_log,
        fw_response,
        active_tab,
        set_active_tab,
        request_tx,
    };
    provide_context(ctx);

    view! {
        <Header />
        <TabBar />
        <div class="main-content">
            {move || {
                match active_tab.get() {
                    Tab::Buses => view! {
                        <crate::components::hil::bus_overview::BusOverview />
                    }.into_any(),
                    Tab::Telemetry => view! {
                        <crate::components::hil::telemetry::TelemetryPanel />
                    }.into_any(),
                    Tab::Console => view! {
                        <crate::components::hil::i2c_console::I2cConsole />
                    }.into_any(),
                    Tab::Firmware => view! {
                        <crate::components::hil::firmware::FirmwarePanel />
                    }.into_any(),
                    Tab::DagEditor => view! {
                        <crate::components::dag::editor::DagEditorPanel />
                    }.into_any(),
                    Tab::Deploy => view! {
                        <crate::components::deploy::panel::DeployPanel />
                    }.into_any(),
                }
            }}
        </div>
    }
}
