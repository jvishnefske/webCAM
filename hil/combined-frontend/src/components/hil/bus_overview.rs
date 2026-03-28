//! I2C bus overview panel — shows detected buses and devices.

use leptos::prelude::*;
use crate::app::AppContext;

#[component]
pub fn BusOverview() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();
    let buses = ctx.buses;

    view! {
        <h2 class="section-title">"I2C Buses"</h2>
        <div class="card-grid">
            {move || {
                let b = buses.get();
                if b.is_empty() {
                    view! { <p class="loading">"No buses detected"</p> }.into_any()
                } else {
                    b.into_iter().map(|bus| {
                        view! {
                            <div class="card">
                                <div class="card-title">
                                    {format!("Bus {}", bus.bus_idx)}
                                    <span class="card-subtitle">
                                        {format!("{} device(s)", bus.devices.len())}
                                    </span>
                                </div>
                                <ul class="device-list">
                                    {bus.devices.into_iter().map(|dev| {
                                        view! {
                                            <li>
                                                <span class="device-addr">
                                                    {format!("0x{:02X}", dev.addr)}
                                                </span>
                                                <span class="device-name">{dev.name}</span>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            </div>
                        }
                    }).collect_view().into_any()
                }
            }}
        </div>
    }
}
