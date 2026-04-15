//! Mode bar (top-level) and dataflow sub-tab bar.

use crate::app::{AppContext, AppMode, DataflowTab};
use leptos::prelude::*;

/// Top-level navigation across the four application modes.
#[component]
pub fn ModeBar() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let modes = [
        (AppMode::Cam, "CAM"),
        (AppMode::Sketch, "Sketch"),
        (AppMode::Dataflow, "Dataflow"),
        (AppMode::Panel, "Panel"),
    ];

    view! {
        <nav class="nav-tabs mode-bar">
            {modes.into_iter().map(|(mode, label)| {
                let set_mode = ctx.set_active_mode;
                let active = ctx.active_mode;
                view! {
                    <button
                        class=move || if active.get() == mode { "nav-tab active" } else { "nav-tab" }
                        on:click=move |_| set_mode.set(mode)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </nav>
    }
}

/// Sub-tab navigation within Dataflow mode.
#[component]
pub fn DataflowTabBar() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let tabs = [
        (DataflowTab::DagEditor, "DAG Editor"),
        (DataflowTab::Buses, "Buses"),
        (DataflowTab::Telemetry, "Telemetry"),
        (DataflowTab::Console, "I2C Console"),
        (DataflowTab::Firmware, "Firmware"),
        (DataflowTab::Deploy, "Deploy"),
    ];

    view! {
        <nav class="nav-tabs sub-tabs">
            {tabs.into_iter().map(|(tab, label)| {
                let set_tab = ctx.set_active_dataflow_tab;
                let active = ctx.active_dataflow_tab;
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
