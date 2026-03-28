//! Deployment manifest panel — stub for Phase 4 implementation.

use leptos::prelude::*;

#[component]
pub fn DeployPanel() -> impl IntoView {
    view! {
        <h2 class="section-title">"Deployment"</h2>
        <div class="card">
            <div class="card-title">"Deployment Manifest"</div>
            <p class="card-subtitle">"System topology, task bindings, channel mapping — Phase 4"</p>
            <div class="info-box">
                "Configure board nodes, physical links, task scheduling, "
                "and channel transport (intra-node memory vs inter-node CAN/RS485)."
            </div>
        </div>
    }
}
