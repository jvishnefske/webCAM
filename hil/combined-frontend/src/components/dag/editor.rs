//! DAG editor panel — stub for Phase 2 implementation.

use leptos::prelude::*;

#[component]
pub fn DagEditorPanel() -> impl IntoView {
    view! {
        <h2 class="section-title">"DAG Editor"</h2>
        <div class="card">
            <div class="card-title">"Expression DAG"</div>
            <p class="card-subtitle">"SVG node editor with CBOR deployment — Phase 2"</p>
            <div class="info-box">
                "Build expression DAGs (const, add, mul, subscribe, publish), "
                "deploy to MCU via CBOR, tick and monitor pubsub topics."
            </div>
        </div>
    }
}
