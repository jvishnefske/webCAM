//! Deployment profile editor — channel mapping UI for deployment profiles.
//!
//! Lists all declared channels from the block set (derived by calling
//! `registry::create_block` + `apply_config` + `declared_channels()`) and
//! lets users remap PubSub channel names for deployment. Hardware channels
//! are shown as read-only with a note that peripheral binding is needed.

use leptos::prelude::*;

use configurable_blocks::deployment_profile::ChannelMap;
use configurable_blocks::registry;
use configurable_blocks::schema::{ChannelDirection, ChannelKind};

use crate::types::BlockSet;

/// A channel row in the editor table, enriched with a unique key for Leptos.
#[derive(Debug, Clone, PartialEq)]
struct ChannelRow {
    /// Unique key for Leptos `<For>` (block_idx + channel name).
    key: String,
    /// Block type that declared this channel.
    block_type: String,
    /// Logical channel name from the block's `declared_channels()`.
    logical_name: String,
    /// Direction: Input or Output.
    direction: ChannelDirection,
    /// Kind: PubSub or Hardware.
    kind: ChannelKind,
}

/// Derive all declared channels from a block set.
///
/// This is a non-reactive helper that creates block instances from the registry,
/// applies config, and collects declared channels.  We do NOT store trait objects
/// in signals (they are not Send+Sync).
fn derive_channels(blocks: &BlockSet) -> Vec<ChannelRow> {
    let mut rows = Vec::new();
    for (idx, (block_type, config)) in blocks.iter().enumerate() {
        let mut block = match registry::create_block(block_type) {
            Some(b) => b,
            None => continue,
        };
        block.apply_config(config);
        for ch in block.declared_channels() {
            rows.push(ChannelRow {
                key: format!("{idx}:{}", ch.name),
                block_type: block_type.clone(),
                logical_name: ch.name,
                direction: ch.direction,
                kind: ch.kind,
            });
        }
    }
    rows
}

/// Deployment profile editor component.
///
/// Reads the shared block set, derives declared channels, and presents a
/// channel mapping table.  PubSub channels get a text input for remapping;
/// Hardware channels show an informational note.
///
/// The component exposes the constructed `ChannelMap` via the `on_map_change`
/// callback, which the parent deploy panel uses to populate the
/// `DeploymentProfile` before calling `lower_block_set`.
#[component]
pub fn ProfileEditor(
    /// The editor's block set (read-only signal).
    blocks: ReadSignal<BlockSet>,
    /// Callback fired whenever the user changes a channel mapping.
    /// Receives the full `ChannelMap` built from all current inputs.
    on_map_change: Callback<ChannelMap>,
) -> impl IntoView {
    // Derive channel rows reactively from the block set.
    let channel_rows = Signal::derive(move || derive_channels(&blocks.get()));

    // Store remapping overrides: logical_name -> deployment_name.
    // Only entries where the user has actively typed something different are stored.
    let (overrides, set_overrides) = signal(std::collections::HashMap::<String, String>::new());

    // Build and emit a ChannelMap whenever overrides or channels change.
    let emit_map = move || {
        let rows = channel_rows.get();
        let ovr = overrides.get();
        let mut map = ChannelMap::new();
        for row in &rows {
            if row.kind == ChannelKind::PubSub {
                let deployment_name = ovr
                    .get(&row.logical_name)
                    .cloned()
                    .unwrap_or_else(|| row.logical_name.clone());
                // Only insert if the names differ (identity mappings are implicit).
                if deployment_name != row.logical_name {
                    map.insert(row.logical_name.clone(), deployment_name);
                }
            }
        }
        on_map_change.run(map);
    };

    // Fire once on mount and whenever inputs change.
    Effect::new(move |_| {
        emit_map();
    });

    view! {
        <div class="card" style="margin-top:1rem;max-width:600px">
            <div class="card-title">"Channel Mapping"</div>

            {move || {
                let rows = channel_rows.get();
                if rows.is_empty() {
                    view! {
                        <div class="info-box" style="color:#6b7280">
                            "No channels declared. Add blocks with channels in the DAG Editor."
                        </div>
                    }.into_any()
                } else {
                    let pubsub_rows: Vec<_> = rows.iter()
                        .filter(|r| r.kind == ChannelKind::PubSub)
                        .cloned()
                        .collect();
                    let hw_rows: Vec<_> = rows.iter()
                        .filter(|r| r.kind == ChannelKind::Hardware)
                        .cloned()
                        .collect();

                    view! {
                        <div>
                            // PubSub channels
                            {if !pubsub_rows.is_empty() {
                                view! {
                                    <div style="margin-bottom:0.75rem">
                                        <div style="font-weight:600;margin-bottom:0.25rem">"PubSub Channels"</div>
                                        <table style="width:100%;border-collapse:collapse;font-size:0.9rem">
                                            <thead>
                                                <tr style="text-align:left;border-bottom:1px solid #e5e7eb">
                                                    <th style="padding:0.25rem 0.5rem">"Block"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Direction"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Logical Name"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Deployment Name"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {pubsub_rows.into_iter().map(|row| {
                                                    let logical = row.logical_name.clone();
                                                    let logical_for_value = row.logical_name.clone();
                                                    let logical_for_handler = row.logical_name.clone();
                                                    let dir_label = match row.direction {
                                                        ChannelDirection::Input => "IN",
                                                        ChannelDirection::Output => "OUT",
                                                    };
                                                    view! {
                                                        <tr style="border-bottom:1px solid #f3f4f6">
                                                            <td style="padding:0.25rem 0.5rem">{row.block_type.clone()}</td>
                                                            <td style="padding:0.25rem 0.5rem">{dir_label}</td>
                                                            <td style="padding:0.25rem 0.5rem;font-family:monospace">{logical.clone()}</td>
                                                            <td style="padding:0.25rem 0.5rem">
                                                                <input
                                                                    type="text"
                                                                    style="width:100%;padding:0.15rem 0.3rem;font-family:monospace;font-size:0.85rem"
                                                                    prop:value=move || {
                                                                        overrides.get()
                                                                            .get(&logical_for_value)
                                                                            .cloned()
                                                                            .unwrap_or_else(|| logical_for_value.clone())
                                                                    }
                                                                    on:input=move |ev| {
                                                                        let val = event_target_value(&ev);
                                                                        let key = logical_for_handler.clone();
                                                                        set_overrides.update(|m| {
                                                                            m.insert(key, val);
                                                                        });
                                                                    }
                                                                />
                                                            </td>
                                                        </tr>
                                                    }
                                                }).collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}

                            // Hardware channels
                            {if !hw_rows.is_empty() {
                                view! {
                                    <div style="margin-bottom:0.75rem">
                                        <div style="font-weight:600;margin-bottom:0.25rem">"Hardware Channels"</div>
                                        <table style="width:100%;border-collapse:collapse;font-size:0.9rem">
                                            <thead>
                                                <tr style="text-align:left;border-bottom:1px solid #e5e7eb">
                                                    <th style="padding:0.25rem 0.5rem">"Block"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Direction"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Channel"</th>
                                                    <th style="padding:0.25rem 0.5rem">"Status"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {hw_rows.into_iter().map(|row| {
                                                    let dir_label = match row.direction {
                                                        ChannelDirection::Input => "IN",
                                                        ChannelDirection::Output => "OUT",
                                                    };
                                                    view! {
                                                        <tr style="border-bottom:1px solid #f3f4f6">
                                                            <td style="padding:0.25rem 0.5rem">{row.block_type.clone()}</td>
                                                            <td style="padding:0.25rem 0.5rem">{dir_label}</td>
                                                            <td style="padding:0.25rem 0.5rem;font-family:monospace">{row.logical_name.clone()}</td>
                                                            <td style="padding:0.25rem 0.5rem;color:#b45309;font-style:italic">
                                                                "Peripheral binding required"
                                                            </td>
                                                        </tr>
                                                    }
                                                }).collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
