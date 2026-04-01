//! Tab navigation bar.

use crate::app::{AppContext, Tab};
use leptos::prelude::*;

#[component]
pub fn TabBar() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let tabs = [
        (Tab::Buses, "Buses"),
        (Tab::Telemetry, "Telemetry"),
        (Tab::Console, "I2C Console"),
        (Tab::Firmware, "Firmware"),
        (Tab::DagEditor, "DAG Editor"),
        (Tab::Deploy, "Deploy"),
    ];

    view! {
        <nav class="nav-tabs">
            {tabs.into_iter().map(|(tab, label)| {
                let set_tab = ctx.set_active_tab;
                let active = ctx.active_tab;
                view! {
                    <button
                        class=move || if active.get() == tab { "nav-tab active" } else { "nav-tab" }
                        on:click=move |_| set_tab.set(tab)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </nav>
    }
}
