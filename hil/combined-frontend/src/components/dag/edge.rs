//! Edge component: renders an SVG Bezier path for a DAG editor connection.

use leptos::prelude::*;

// Re-export the pure geometry helpers for convenient access from sibling modules.
pub use crate::edge_math::{edge_path_d, port_y};

/// A single edge rendered as an SVG Bezier path.
///
/// Includes an invisible fat hit-area path for easier clicking, followed by
/// the visible styled path. The hit-area is transparent with a wide stroke so
/// the user does not need pixel-precise aim.
#[component]
pub fn EdgePath(
    channel_id: u32,
    /// Path data string (d attribute).
    path_d: String,
    is_selected: Signal<bool>,
    on_select: Callback<u32>,
) -> impl IntoView {
    let d1 = path_d.clone();
    let d2 = path_d;

    view! {
        // Invisible fat hit-area for easier clicking
        <path
            d=d1
            fill="none"
            stroke="transparent"
            stroke-width="12"
            class="df-edge-hit"
            style="cursor:pointer;pointer-events:stroke"
            data-ch=channel_id.to_string()
            on:click=move |ev| {
                ev.stop_propagation();
                on_select.run(channel_id);
            }
        />
        // Visible edge path
        <path
            d=d2
            fill="none"
            stroke=move || if is_selected.get() { "#4f8cff" } else { "#4f8cff66" }
            stroke-width=move || if is_selected.get() { "3" } else { "2" }
            stroke-dasharray=move || if is_selected.get() { "6 3" } else { "" }
            class="df-edge"
            style="pointer-events:none"
            data-ch=channel_id.to_string()
        />
    }
}
