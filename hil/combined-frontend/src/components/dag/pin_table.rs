//! MCU pin inspector component — shows pin definitions for a selected MCU family.
//!
//! Uses [`module_traits::inventory::mcu_for`] to look up the full [`McuDef`]
//! and renders the pin table including alternate functions and ADC channels.

use leptos::prelude::*;

use module_traits::inventory;

/// Pin inspector table for a given MCU family.
///
/// Shows all GPIO pin definitions with their port, number, alternate functions,
/// and ADC channel assignment. The table updates reactively when the `family`
/// signal changes.
#[component]
pub fn PinTable(family: ReadSignal<String>) -> impl IntoView {
    let pins = Signal::derive(move || {
        let fam = family.get();
        inventory::mcu_for(&fam)
            .map(|mcu| mcu.pins)
            .unwrap_or_default()
    });

    let mcu_display = Signal::derive(move || {
        let fam = family.get();
        inventory::mcu_for(&fam)
            .map(|m| m.display_name)
            .unwrap_or_else(|| fam)
    });

    view! {
        <div class="card" style="max-width:800px;margin-bottom:1rem">
            <div class="card-title">
                "Pin Inspector: "
                {move || mcu_display.get()}
            </div>

            {move || {
                let pin_list = pins.get();
                if pin_list.is_empty() {
                    view! {
                        <div class="info-box" style="color:#6b7280">
                            "No pin definitions available for this MCU family."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div style="max-height:400px;overflow-y:auto">
                            <table style="width:100%;border-collapse:collapse;font-size:0.85rem">
                                <thead>
                                    <tr style="text-align:left;border-bottom:2px solid #e5e7eb;position:sticky;top:0;background:#fff">
                                        <th style="padding:0.3rem 0.5rem">"Pin"</th>
                                        <th style="padding:0.3rem 0.5rem">"Port"</th>
                                        <th style="padding:0.3rem 0.5rem">"Number"</th>
                                        <th style="padding:0.3rem 0.5rem">"ADC Ch"</th>
                                        <th style="padding:0.3rem 0.5rem">"5V Tolerant"</th>
                                        <th style="padding:0.3rem 0.5rem">"Alternate Functions"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {pin_list.into_iter().map(|pin| {
                                        let adc_label = pin.adc_channel
                                            .map(|ch| format!("ADC{ch}"))
                                            .unwrap_or_else(|| "-".to_string());
                                        let five_v = if pin.five_v_tolerant { "Yes" } else { "-" };
                                        let alt_fns: String = pin.alt_functions.iter()
                                            .map(|af| format!("AF{}: {}/{}", af.af, af.peripheral, af.signal))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let alt_display = if alt_fns.is_empty() {
                                            "-".to_string()
                                        } else {
                                            alt_fns
                                        };
                                        view! {
                                            <tr style="border-bottom:1px solid #f3f4f6">
                                                <td style="padding:0.2rem 0.5rem;font-family:monospace;font-weight:600">{pin.name}</td>
                                                <td style="padding:0.2rem 0.5rem">{pin.port}</td>
                                                <td style="padding:0.2rem 0.5rem">{pin.number}</td>
                                                <td style="padding:0.2rem 0.5rem">{adc_label}</td>
                                                <td style="padding:0.2rem 0.5rem">{five_v}</td>
                                                <td style="padding:0.2rem 0.5rem;font-size:0.8rem">{alt_display}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                        <div class="info-box" style="margin-top:0.5rem;font-size:0.85rem">
                            {move || format!("{} pins total", pins.get().len())}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
