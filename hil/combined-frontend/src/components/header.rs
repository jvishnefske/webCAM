//! Header component with connection status badge.

use crate::app::AppContext;
use crate::ws_client::ConnState;
use leptos::prelude::*;

#[component]
pub fn Header() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let conn_class = move || match ctx.conn_state.get() {
        ConnState::Open => "status-indicator status-connected",
        ConnState::Closed => "status-indicator status-disconnected",
        ConnState::Connecting => "status-indicator status-connecting",
    };

    let conn_text = move || match ctx.conn_state.get() {
        ConnState::Open => "Connected",
        ConnState::Closed => "Disconnected",
        ConnState::Connecting => "Connecting...",
    };

    view! {
        <div class="header">
            <h1>"MCU Control Panel"</h1>
            <div class=conn_class>
                <span class="status-dot"></span>
                <span>{conn_text}</span>
            </div>
        </div>
    }
}
