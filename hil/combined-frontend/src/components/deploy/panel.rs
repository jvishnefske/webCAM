//! Deployment panel — generate firmware crates from deployment config.

use leptos::prelude::*;

use configurable_blocks::codegen;
use module_traits::deployment::*;
use module_traits::inventory;

#[component]
pub fn DeployPanel() -> impl IntoView {
    let (target_family, set_target_family) = signal("Rp2040".to_string());
    let (node_id, set_node_id) = signal("controller".to_string());
    let (tick_hz, set_tick_hz) = signal("100".to_string());
    let (gen_status, set_gen_status) = signal(String::new());
    let (gen_files, set_gen_files) = signal(Vec::<(String, String)>::new());

    let families = inventory::supported_families();

    let on_generate = move |_| {
        let family = target_family.get();
        let node = node_id.get();
        let hz: f64 = tick_hz.get().parse().unwrap_or(100.0);

        // Build a minimal deployment manifest for one node
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

        // Build a simple test DAG (const → publish)
        // In the full version, this would come from the DAG editor's lowered blocks
        let mut dag = dag_core::op::Dag::new();
        let c = dag.constant(0.0).unwrap_or(0);
        let _ = dag.publish("output", c);

        match codegen::generate_all_crates(&manifest, &dag) {
            Ok(files) => {
                set_gen_status.set(format!(
                    "Generated {} files for {} ({})",
                    files.len(), node, family
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
