//! Bus overview component displaying a grid of I2C bus cards.
//!
//! Shows all 10 I2C buses with their attached devices. Each card lists
//! the bus index and all discovered devices with their 7-bit addresses
//! and human-readable names.

use leptos::prelude::*;

use crate::messages::BusEntry;

/// Grid overview of all I2C buses and their attached devices.
///
/// Renders a card for each bus in the `buses` signal. If no bus data
/// has been received yet, displays a loading placeholder.
#[component]
pub fn BusOverview(
    /// Reactive signal containing the list of buses and their devices.
    buses: ReadSignal<Vec<BusEntry>>,
) -> impl IntoView {
    view! {
        <div>
            <h2 class="section-title">"I2C Bus Overview"</h2>
            {move || {
                let bus_list = buses.get();
                if bus_list.is_empty() {
                    view! {
                        <div class="loading">"Waiting for bus data... Connect to the Pico to see devices."</div>
                    }.into_any()
                } else {
                    view! {
                        <div class="card-grid">
                            {bus_list.into_iter().map(|bus| {
                                let bus_idx = bus.bus_idx;
                                let devices = bus.devices;
                                view! {
                                    <div class="card">
                                        <div class="card-title">
                                            <span>{format!("Bus {bus_idx}")}</span>
                                            <span class="card-subtitle">{format!("{} device(s)", devices.len())}</span>
                                        </div>
                                        {if devices.is_empty() {
                                            view! {
                                                <div class="card-subtitle">"No devices"</div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <ul class="device-list">
                                                    {devices.into_iter().map(|dev| {
                                                        view! {
                                                            <li>
                                                                <span class="device-addr">{format!("0x{:02X}", dev.addr)}</span>
                                                                <span class="device-name">{dev.name.clone()}</span>
                                                            </li>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </ul>
                                            }.into_any()
                                        }}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
