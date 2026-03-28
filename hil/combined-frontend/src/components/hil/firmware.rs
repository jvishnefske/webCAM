//! Firmware update panel — OTA upload with progress.

use leptos::prelude::*;

#[component]
pub fn FirmwarePanel() -> impl IntoView {
    view! {
        <h2 class="section-title">"Firmware Update"</h2>
        <div class="card">
            <div class="card-title">"OTA Update"</div>
            <p class="card-subtitle">"Firmware update panel — full implementation in Phase 2"</p>
            <div class="info-box">
                "Select a .bin firmware file to upload over WebSocket. "
                "The device will erase, write, verify, and reboot automatically."
            </div>
        </div>
    }
}
