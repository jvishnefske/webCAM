//! Codegen export panel — generate and download firmware crate files.
//!
//! Provides single-target and multi-target export with collapsible file
//! listings, line counts, and ZIP download via in-browser blob URLs.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::codegen;
use configurable_blocks::deployment_profile::DeploymentProfile;
use configurable_blocks::lower::lower_block_set;
use module_traits::deployment::*;

use crate::types::BlockSet;
use crate::zip::build_zip;

use super::board_manager::Board;

/// Export panel for generating firmware crate files and downloading as ZIP.
///
/// Supports both single-board and multi-board export. When boards are provided,
/// generates a workspace with per-board crates. Otherwise falls back to a
/// single-crate export.
#[component]
pub fn ExportPanel(boards: ReadSignal<Vec<Board>>) -> impl IntoView {
    let (gen_status, set_gen_status) = signal(String::new());
    let (gen_files, set_gen_files) = signal(Vec::<(String, String)>::new());

    let shared_blocks = use_context::<ReadSignal<BlockSet>>();
    let (empty_blocks, _set_empty) = signal(BlockSet::new());
    let blocks_signal: ReadSignal<BlockSet> = shared_blocks.unwrap_or(empty_blocks);

    let block_count = Signal::derive(move || blocks_signal.get().len());

    // Single-board export
    let on_export_single = move |_| {
        let editor_blocks = blocks_signal.get();
        if editor_blocks.is_empty() {
            set_gen_status.set("No blocks to export. Add blocks in the DAG Editor tab.".into());
            set_gen_files.set(vec![]);
            return;
        }

        let profile = DeploymentProfile::new("controller");
        let dag = match lower_block_set(&editor_blocks, &profile) {
            Ok(d) => d,
            Err(e) => {
                set_gen_status.set(format!("Lowering error: {e}"));
                set_gen_files.set(vec![]);
                return;
            }
        };

        let manifest = DeploymentManifest {
            topology: SystemTopology {
                nodes: vec![BoardNode {
                    id: "controller".into(),
                    mcu_family: "Rp2040".into(),
                    board: None,
                    rust_target: None,
                }],
                links: vec![],
            },
            tasks: vec![TaskBinding {
                name: "main_loop".into(),
                node: "controller".into(),
                blocks: vec![],
                trigger: TaskTrigger::Periodic { hz: 100.0 },
                priority: 1,
                stack_size: None,
            }],
            channels: vec![],
            peripheral_bindings: vec![],
        };

        match codegen::generate_all_crates(&manifest, &dag) {
            Ok(files) => {
                set_gen_status.set(format!("Generated {} files (single-board)", files.len()));
                set_gen_files.set(files);
            }
            Err(e) => {
                set_gen_status.set(format!("Codegen error: {e}"));
                set_gen_files.set(vec![]);
            }
        }
    };

    // Multi-board export
    let on_export_multi = move |_| {
        let editor_blocks = blocks_signal.get();
        let board_list = boards.get();

        if editor_blocks.is_empty() {
            set_gen_status.set("No blocks to export. Add blocks in the DAG Editor tab.".into());
            set_gen_files.set(vec![]);
            return;
        }
        if board_list.is_empty() {
            set_gen_status.set("No boards configured. Add boards in Board Management.".into());
            set_gen_files.set(vec![]);
            return;
        }

        let profile = DeploymentProfile::new("workspace");
        let dag = match lower_block_set(&editor_blocks, &profile) {
            Ok(d) => d,
            Err(e) => {
                set_gen_status.set(format!("Lowering error: {e}"));
                set_gen_files.set(vec![]);
                return;
            }
        };

        let nodes: Vec<BoardNode> = board_list
            .iter()
            .map(|b| BoardNode {
                id: b.name.clone(),
                mcu_family: b.family.clone(),
                board: None,
                rust_target: None,
            })
            .collect();

        let tasks: Vec<TaskBinding> = board_list
            .iter()
            .map(|b| TaskBinding {
                name: format!("{}_loop", b.name),
                node: b.name.clone(),
                blocks: vec![],
                trigger: TaskTrigger::Periodic { hz: 100.0 },
                priority: 1,
                stack_size: None,
            })
            .collect();

        let manifest = DeploymentManifest {
            topology: SystemTopology {
                nodes,
                links: vec![],
            },
            tasks,
            channels: vec![],
            peripheral_bindings: vec![],
        };

        match codegen::generate_all_crates(&manifest, &dag) {
            Ok(mut files) => {
                // Add a root Cargo.toml workspace manifest
                let members: Vec<String> = board_list
                    .iter()
                    .map(|b| format!("    \"firmware-{}\"", b.name))
                    .collect();
                let workspace_toml = format!(
                    "[workspace]\nmembers = [\n{}\n]\nresolver = \"2\"\n",
                    members.join(",\n")
                );
                files.insert(0, ("Cargo.toml".into(), workspace_toml));

                set_gen_status.set(format!(
                    "Generated {} files for {} boards (workspace)",
                    files.len(),
                    board_list.len()
                ));
                set_gen_files.set(files);
            }
            Err(e) => {
                set_gen_status.set(format!("Codegen error: {e}"));
                set_gen_files.set(vec![]);
            }
        }
    };

    // ZIP download
    let on_download_zip = move |_| {
        let files = gen_files.get();
        if files.is_empty() {
            return;
        }
        let zip_bytes = build_zip(&files);
        trigger_browser_download("firmware.zip", &zip_bytes, "application/zip");
    };

    view! {
        <div class="card" style="max-width:800px;margin-bottom:1rem">
            <div class="card-title">"Codegen Export"</div>

            {move || {
                let count = block_count.get();
                if count == 0 {
                    view! {
                        <div class="info-box" style="color:#b45309;margin-bottom:0.75rem">
                            "No blocks available. Add blocks in the DAG Editor tab."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="info-box" style="margin-bottom:0.75rem">
                            {format!("{count} block(s) ready for export.")}
                        </div>
                    }.into_any()
                }
            }}

            <div style="display:flex;gap:0.5rem;margin-bottom:0.75rem;flex-wrap:wrap">
                <button class="btn btn-primary" on:click=on_export_single>
                    "Export (Single Board)"
                </button>
                <button class="btn btn-primary" on:click=on_export_multi>
                    "Export (Multi-Board Workspace)"
                </button>
                <button
                    class="btn"
                    style="border:1px solid #d1d5db"
                    on:click=on_download_zip
                >
                    "Download ZIP"
                </button>
            </div>

            // Status message
            {move || {
                let status = gen_status.get();
                if status.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <div class="info-box" style="margin-bottom:0.75rem">{status}</div>
                    }.into_any()
                }
            }}

            // Generated file listing with collapsible details
            {move || {
                let files = gen_files.get();
                if files.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    let total_lines: usize = files.iter().map(|(_, c)| c.lines().count()).sum();
                    view! {
                        <div>
                            <div style="font-weight:600;margin-bottom:0.5rem">
                                {format!("{} files, {} total lines", files.len(), total_lines)}
                            </div>
                            {files.iter().map(|(path, content)| {
                                let path = path.clone();
                                let content = content.clone();
                                let lines = content.lines().count();
                                view! {
                                    <details class="card" style="margin-bottom:0.5rem;padding:0.5rem">
                                        <summary style="cursor:pointer;font-family:monospace;font-size:0.9rem">
                                            {path.clone()}
                                            <span style="color:#6b7280;margin-left:0.5rem">
                                                {format!("({lines} lines)")}
                                            </span>
                                        </summary>
                                        <pre class="console-output" style="margin-top:0.5rem;max-height:400px;overflow:auto;font-size:0.8rem">{content}</pre>
                                    </details>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

/// Trigger a file download in the browser by creating a temporary blob URL.
fn trigger_browser_download(filename: &str, data: &[u8], mime_type: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create a Uint8Array from the data.
    let uint8_array = js_sys::Uint8Array::new_with_length(data.len() as u32);
    uint8_array.copy_from(data);

    // Create a Blob with the specified MIME type.
    let parts = js_sys::Array::new();
    parts.push(&uint8_array.buffer());

    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime_type);

    let blob = match web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &options) {
        Ok(b) => b,
        Err(_) => return,
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    // Create a temporary <a> element, click it, then clean up.
    let anchor = match document.create_element("a") {
        Ok(el) => el,
        Err(_) => return,
    };

    let _ = anchor.set_attribute("href", &url);
    let _ = anchor.set_attribute("download", filename);
    let _ = anchor.set_attribute("style", "display:none");

    let body = match document.body() {
        Some(b) => b,
        None => return,
    };
    let _ = body.append_child(&anchor);

    if let Some(html_el) = anchor.dyn_ref::<web_sys::HtmlElement>() {
        html_el.click();
    }

    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
}
