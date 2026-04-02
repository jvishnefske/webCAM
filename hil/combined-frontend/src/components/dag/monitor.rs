//! Live pubsub monitor panel — shows all topics and current values.

use std::collections::BTreeMap;

use leptos::prelude::*;

/// Collapsible panel displaying live pubsub topic values from SimState.
#[component]
pub fn MonitorPanel(
    /// Reactive signal with current pubsub topics (updated on each tick).
    topics: ReadSignal<BTreeMap<String, f64>>,
    /// Current tick count.
    tick_count: ReadSignal<u64>,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(true);

    view! {
        <div class="monitor-panel">
            <button
                class="monitor-header"
                on:click=move |_| set_expanded.set(!expanded.get())
            >
                <span class="monitor-chevron">
                    {move || if expanded.get() { "\u{25BE}" } else { "\u{25B8}" }}
                </span>
                {move || {
                    let count = topics.get().len();
                    let tick = tick_count.get();
                    format!("Monitor ({count} topics, tick {tick})")
                }}
            </button>
            <div
                class="monitor-body"
                style=move || if expanded.get() { "display:block" } else { "display:none" }
            >
                {move || {
                    let t = topics.get();
                    if t.is_empty() {
                        view! {
                            <div class="monitor-empty">"No topics yet. Click Evaluate or Step to run."</div>
                        }.into_any()
                    } else {
                        view! {
                            <table class="monitor-table">
                                <thead>
                                    <tr>
                                        <th>"Topic"</th>
                                        <th>"Value"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {t.iter().map(|(name, val)| {
                                        let name = name.clone();
                                        view! {
                                            <tr>
                                                <td class="monitor-topic">{name}</td>
                                                <td class="monitor-value">{format!("{val:.4}")}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
