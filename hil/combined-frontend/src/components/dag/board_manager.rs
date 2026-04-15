//! Board management component — add, list, and delete target boards.
//!
//! Each board has a user-chosen name and an MCU family selected from
//! [`module_traits::inventory::supported_families`]. The component also
//! manages per-block target assignment (which board each block runs on).

use std::collections::HashMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use module_traits::inventory;

/// A target board in the multi-board deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// User-chosen board name (e.g., "motor_ctrl", "sensor_hub").
    pub name: String,
    /// MCU family from `target-registry` / `module-traits` (e.g., "Rp2040").
    pub family: String,
}

/// Board management panel.
///
/// Allows users to add boards with a name and MCU family, view the board
/// list with delete buttons, and manage per-block target assignment.
#[component]
pub fn BoardManager(
    boards: ReadSignal<Vec<Board>>,
    set_boards: WriteSignal<Vec<Board>>,
    block_targets: ReadSignal<HashMap<u32, String>>,
    set_block_targets: WriteSignal<HashMap<u32, String>>,
) -> impl IntoView {
    let (new_name, set_new_name) = signal(String::new());
    let (new_family, set_new_family) = signal("Rp2040".to_string());

    let families = inventory::supported_families();

    let on_add = move |_| {
        let name = new_name.get();
        if name.trim().is_empty() {
            return;
        }
        let family = new_family.get();
        set_boards.update(|list| {
            // Prevent duplicate board names.
            if !list.iter().any(|b| b.name == name) {
                list.push(Board {
                    name: name.clone(),
                    family,
                });
            }
        });
        set_new_name.set(String::new());
    };

    let on_delete = move |board_name: String| {
        set_boards.update(|list| {
            list.retain(|b| b.name != board_name);
        });
        // Remove any block assignments pointing to the deleted board.
        set_block_targets.update(|map| {
            map.retain(|_, v| *v != board_name);
        });
    };

    view! {
        <div class="card" style="max-width:600px;margin-bottom:1rem">
            <div class="card-title">"Board Management"</div>

            // Add board form
            <div class="form-row" style="margin-bottom:0.75rem">
                <div class="form-group">
                    <label>"Board Name"</label>
                    <input type="text"
                        placeholder="e.g. motor_ctrl"
                        prop:value=move || new_name.get()
                        on:input=move |ev| set_new_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-group">
                    <label>"MCU Family"</label>
                    <select
                        prop:value=move || new_family.get()
                        on:change=move |ev| set_new_family.set(event_target_value(&ev))
                    >
                        {families.iter().map(|f| {
                            let f = f.to_string();
                            let display = inventory::mcu_for(&f)
                                .map(|m| m.display_name)
                                .unwrap_or_else(|| f.clone());
                            view! { <option value=f.clone()>{display}</option> }
                        }).collect_view()}
                    </select>
                </div>
                <div class="form-group" style="align-self:flex-end">
                    <button class="btn btn-primary" on:click=on_add>"Add Board"</button>
                </div>
            </div>

            // Board list
            {move || {
                let board_list = boards.get();
                if board_list.is_empty() {
                    view! {
                        <div class="info-box" style="color:#6b7280">
                            "No boards configured. Add a board above to begin multi-board deployment."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <table style="width:100%;border-collapse:collapse;font-size:0.9rem">
                            <thead>
                                <tr style="text-align:left;border-bottom:1px solid #e5e7eb">
                                    <th style="padding:0.25rem 0.5rem">"Name"</th>
                                    <th style="padding:0.25rem 0.5rem">"MCU Family"</th>
                                    <th style="padding:0.25rem 0.5rem">"Display"</th>
                                    <th style="padding:0.25rem 0.5rem">"Actions"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {board_list.into_iter().map(|board| {
                                    let delete_name = board.name.clone();
                                    let display = inventory::mcu_for(&board.family)
                                        .map(|m| m.display_name)
                                        .unwrap_or_else(|| board.family.clone());
                                    view! {
                                        <tr style="border-bottom:1px solid #f3f4f6">
                                            <td style="padding:0.25rem 0.5rem;font-family:monospace">{board.name.clone()}</td>
                                            <td style="padding:0.25rem 0.5rem">{board.family.clone()}</td>
                                            <td style="padding:0.25rem 0.5rem">{display}</td>
                                            <td style="padding:0.25rem 0.5rem">
                                                <button
                                                    class="btn"
                                                    style="font-size:0.8rem;padding:0.15rem 0.4rem;color:#dc2626"
                                                    on:click=move |_| on_delete(delete_name.clone())
                                                >"Delete"</button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }.into_any()
                }
            }}

            // Block target assignment summary
            {move || {
                let targets = block_targets.get();
                if targets.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <div style="margin-top:0.75rem">
                            <div style="font-weight:600;margin-bottom:0.25rem">"Block Assignments"</div>
                            <table style="width:100%;border-collapse:collapse;font-size:0.85rem">
                                <thead>
                                    <tr style="text-align:left;border-bottom:1px solid #e5e7eb">
                                        <th style="padding:0.2rem 0.5rem">"Block ID"</th>
                                        <th style="padding:0.2rem 0.5rem">"Target Board"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {targets.into_iter().map(|(block_id, board_name)| {
                                        view! {
                                            <tr style="border-bottom:1px solid #f3f4f6">
                                                <td style="padding:0.2rem 0.5rem;font-family:monospace">{block_id}</td>
                                                <td style="padding:0.2rem 0.5rem;font-family:monospace">{board_name}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
