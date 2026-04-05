//! Deployment panel — generate firmware crates from deployment config.
//!
//! Reads the shared block set from the DAG editor (via Leptos context) and
//! lowers it with [`lower_block_set`] before feeding the result into
//! [`generate_all_crates`]. If no blocks have been placed in the editor,
//! the panel shows an informative message.

use leptos::prelude::*;

use configurable_blocks::codegen;
use configurable_blocks::deployment_profile::{ChannelMap, DeploymentProfile};
use configurable_blocks::lower::lower_block_set;
use module_traits::deployment::*;
use module_traits::inventory;

use crate::types::BlockSet;
use super::profile_editor::ProfileEditor;

#[component]
pub fn DeployPanel() -> impl IntoView {
    let (target_family, set_target_family) = signal("Rp2040".to_string());
    let (node_id, set_node_id) = signal("controller".to_string());
    let (tick_hz, set_tick_hz) = signal("100".to_string());
    let (gen_status, set_gen_status) = signal(String::new());
    let (gen_files, set_gen_files) = signal(Vec::<(String, String)>::new());
    let (channel_map, set_channel_map) = signal(ChannelMap::new());

    let families = inventory::supported_families();

    // Read the shared block set from the DAG editor tab.
    let shared_blocks = use_context::<ReadSignal<BlockSet>>();

    // Create a local read signal for the block set that the ProfileEditor can use.
    // If no shared context exists, we provide an always-empty signal.
    let (empty_blocks, _set_empty) = signal(BlockSet::new());
    let blocks_for_editor: ReadSignal<BlockSet> = shared_blocks.unwrap_or(empty_blocks);

    let block_count = Signal::derive(move || blocks_for_editor.get().len());

    let on_map_change = Callback::new(move |map: ChannelMap| {
        set_channel_map.set(map);
    });

    let on_generate = move |_| {
        let family = target_family.get();
        let node = node_id.get();
        let hz: f64 = tick_hz.get().parse().unwrap_or(100.0);

        // Retrieve blocks from the shared editor state.
        let editor_blocks: BlockSet = blocks_for_editor.get();

        if editor_blocks.is_empty() {
            set_gen_status.set("No blocks to deploy. Add blocks in the DAG Editor tab first.".into());
            set_gen_files.set(vec![]);
            return;
        }

        // Build a deployment profile from UI inputs with channel remapping from the editor.
        let mut profile = DeploymentProfile::new(&node);
        profile.channel_map = channel_map.get();

        // Lower the editor's block set into a single DAG.
        let dag = match lower_block_set(&editor_blocks, &profile) {
            Ok(d) => d,
            Err(e) => {
                set_gen_status.set(format!("Lowering error: {e}"));
                set_gen_files.set(vec![]);
                return;
            }
        };

        // Build a minimal deployment manifest for one node.
        let manifest = DeploymentManifest {
            topology: SystemTopology {
                nodes: vec![BoardNode {
                    id: node.clone(),
                    mcu_family: family.clone(),
                    board: None,
                    rust_target: None,
                }],
                links: vec![],
            },
            tasks: vec![TaskBinding {
                name: "main_loop".into(),
                node: node.clone(),
                blocks: vec![],
                trigger: TaskTrigger::Periodic { hz },
                priority: 1,
                stack_size: None,
            }],
            channels: vec![],
            peripheral_bindings: vec![],
        };

        match codegen::generate_all_crates(&manifest, &dag) {
            Ok(files) => {
                set_gen_status.set(format!(
                    "Generated {} files for {} ({}) from {} editor blocks",
                    files.len(), node, family, editor_blocks.len()
                ));
                set_gen_files.set(files);
            }
            Err(e) => {
                set_gen_status.set(format!("Error: {e}"));
                set_gen_files.set(vec![]);
            }
        }
    };

    view! {
        <h2 class="section-title">"Generate Firmware Crate"</h2>
        <div class="card" style="max-width:600px">
            <div class="card-title">"Deployment Configuration"</div>

            // Block set status from the editor
            {move || {
                let count = block_count.get();
                if count == 0 {
                    view! {
                        <div class="info-box" style="margin-bottom:0.75rem;color:#b45309">
                            "No blocks configured. Add blocks in the DAG Editor tab first."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="info-box" style="margin-bottom:0.75rem">
                            {format!("{count} block(s) from editor ready for deployment.")}
                        </div>
                    }.into_any()
                }
            }}

            <div class="form-row">
                <div class="form-group">
                    <label>"Node ID"</label>
                    <input type="text"
                        prop:value=move || node_id.get()
                        on:input=move |ev| set_node_id.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-group">
                    <label>"MCU Target"</label>
                    <select
                        prop:value=move || target_family.get()
                        on:change=move |ev| set_target_family.set(event_target_value(&ev))
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
                <div class="form-group">
                    <label>"Tick Rate (Hz)"</label>
                    <input type="number"
                        prop:value=move || tick_hz.get()
                        on:input=move |ev| set_tick_hz.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <button class="btn btn-primary" on:click=on_generate>"Generate Crate"</button>

            {move || {
                let status = gen_status.get();
                if status.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! { <div class="info-box" style="margin-top:0.75rem">{status}</div> }.into_any()
                }
            }}
        </div>

        // Channel mapping editor
        <ProfileEditor blocks=blocks_for_editor on_map_change=on_map_change />

        // Show generated files
        {move || {
            let files = gen_files.get();
            if files.is_empty() {
                view! { <div></div> }.into_any()
            } else {
                view! {
                    <div style="margin-top:1rem">
                        <h3 class="section-title">"Generated Files"</h3>
                        {files.iter().map(|(path, content)| {
                            let path = path.clone();
                            let content = content.clone();
                            let lines = content.lines().count();
                            view! {
                                <details class="card" style="margin-bottom:0.5rem">
                                    <summary class="card-title" style="cursor:pointer">
                                        {path.clone()}
                                        <span class="card-subtitle">{format!("{lines} lines")}</span>
                                    </summary>
                                    <pre class="console-output" style="margin-top:0.5rem;max-height:400px">{content}</pre>
                                </details>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }
        }}
    }
}
