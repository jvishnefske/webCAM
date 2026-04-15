//! DAG editor panel: palette, canvas, config, deploy.
//!
//! Uses [`GraphEngine`] for block/channel management and simulation,
//! [`storage`] for localStorage persistence, and [`ProjectSidebar`]
//! for project load/save UI.

use std::cell::RefCell;
use std::collections::HashMap;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::lower;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;

use crate::graph_engine::{BlockId, ChannelId, GraphEngine};
use crate::sim_util::{format_sim_time, SPEED_PRESETS};
use crate::types::BlockSet;

use super::config_panel::ConfigPanel;
use super::monitor::MonitorPanel;
use super::palette::BlockPalette;
use super::sidebar::ProjectSidebar;
use super::storage::{self, SavedProject};

// ── Thread-local engine (not Send, same pattern as SimState) ────────────────

thread_local! {
    static ENGINE: RefCell<GraphEngine> = RefCell::new(GraphEngine::new());
}

/// Run a closure with a mutable reference to the engine.
fn with_engine<R>(f: impl FnOnce(&mut GraphEngine) -> R) -> R {
    ENGINE.with(|cell| f(&mut cell.borrow_mut()))
}

/// Run a closure with an immutable reference to the engine.
fn with_engine_ref<R>(f: impl FnOnce(&GraphEngine) -> R) -> R {
    ENGINE.with(|cell| f(&cell.borrow()))
}

// ── Wire drag state ─────────────────────────────────────────────────────────

/// State of an in-progress wire drag from an output port.
#[derive(Clone, Copy)]
struct DraggingWire {
    from_block: BlockId,
    from_port: usize,
    mouse_x: f64,
    mouse_y: f64,
}

/// State of an in-progress node drag.
#[derive(Clone, Copy)]
struct DraggingNode {
    block_id: BlockId,
    start_mouse_x: f64,
    start_mouse_y: f64,
    start_node_x: f64,
    start_node_y: f64,
    moved: bool,
}

/// State of an in-progress canvas pan.
#[derive(Clone, Copy)]
struct Panning {
    start_mouse_x: f64,
    start_mouse_y: f64,
    start_pan_x: f64,
    start_pan_y: f64,
}

#[component]
pub fn DagEditorPanel() -> impl IntoView {
    // ── Layout / interaction constants ──────────────────────────────────────
    const NODE_WIDTH: f64 = 190.0;
    const PORT_RADIUS: f64 = 6.0;
    const PORT_SPACING: f64 = 20.0;
    const PORT_Y_START: f64 = 46.0;
    const ZOOM_FACTOR: f64 = 1.1;
    const ZOOM_MIN: f64 = 0.2;
    const ZOOM_MAX: f64 = 5.0;
    const DRAG_THRESHOLD: f64 = 3.0;

    // Revision counter — bumped after every engine mutation to trigger re-reads.
    let (revision, set_revision) = signal(0_u64);
    let bump = move || set_revision.update(|r| *r += 1);

    // Block positions: GraphEngine does not track positions.
    let (positions, set_positions) = signal(HashMap::<BlockId, (f64, f64)>::new());

    // Pan / zoom state.
    let (pan_x, set_pan_x) = signal(0.0_f64);
    let (pan_y, set_pan_y) = signal(0.0_f64);
    let (zoom, set_zoom) = signal(1.0_f64);

    // Node drag state.
    let (dragging_node, set_dragging_node) = signal(None::<DraggingNode>);

    // Panning state.
    let (panning, set_panning) = signal(None::<Panning>);

    // Shared block-set context: push (block_type, config) pairs to deploy panel.
    let set_shared_blocks = use_context::<WriteSignal<BlockSet>>();

    // Sync engine blocks -> shared context.
    let sync_shared = move || {
        if let Some(setter) = set_shared_blocks {
            let block_set: BlockSet = with_engine_ref(|eng| {
                eng.blocks()
                    .iter()
                    .map(|b| (b.block_type.clone(), b.config.clone()))
                    .collect()
            });
            setter.set(block_set);
        }
    };

    // Selected block / channel.
    let (selected_id, set_selected_id) = signal(None::<BlockId>);
    let (selected_channel, set_selected_channel) = signal(None::<ChannelId>);

    // Wire drag state.
    let (dragging_wire, set_dragging_wire) = signal(None::<DraggingWire>);

    // Project name.
    let (project_name, set_project_name) = signal("untitled".to_string());

    // ── Auto-save debounce ──────────────────────────────────────────────────

    // We use a revision counter to detect changes. When revision changes,
    // schedule a save after 2 seconds (debounced via gloo Timeout).
    //
    // The timeout handle is stored in a thread_local so we can cancel it
    // on subsequent changes (debounce).
    thread_local! {
        static AUTOSAVE_HANDLE: RefCell<Option<gloo_timers::callback::Timeout>> = const { RefCell::new(None) };
    }

    let schedule_autosave = move || {
        let name = project_name.get_untracked();
        if name.is_empty() {
            return;
        }
        // Cancel any pending autosave.
        AUTOSAVE_HANDLE.with(|cell| {
            *cell.borrow_mut() = None;
        });
        let pos = positions.get_untracked();
        let timeout = gloo_timers::callback::Timeout::new(2_000, move || {
            let snapshot = with_engine_ref(|eng| eng.snapshot());
            let project = SavedProject {
                name: name.clone(),
                snapshot,
                positions: pos,
                saved_at: String::new(),
            };
            let _ = storage::save_project(&project);
        });
        AUTOSAVE_HANDLE.with(|cell| {
            *cell.borrow_mut() = Some(timeout);
        });
    };

    // Watch revision and schedule autosave.
    Effect::new(move |_| {
        let _rev = revision.get(); // track
        schedule_autosave();
    });

    // ── Config signals derived from selection ───────────────────────────────

    let selected_block_type = Signal::derive(move || {
        let _rev = revision.get();
        let sel = selected_id.get()?;
        with_engine_ref(|eng| {
            let blk = eng.block(sel)?;
            let block = blk.reconstruct()?;
            Some(block.display_name().to_string())
        })
    });

    let config_fields = Signal::derive(move || {
        let _rev = revision.get();
        let sel = selected_id.get();
        match sel {
            Some(id) => with_engine_ref(|eng| {
                eng.block(id)
                    .and_then(|b| b.reconstruct())
                    .map(|b| b.config_schema())
                    .unwrap_or_default()
            }),
            None => Vec::new(),
        }
    });

    let config_values = Signal::derive(move || {
        let _rev = revision.get();
        let sel = selected_id.get();
        match sel {
            Some(id) => with_engine_ref(|eng| {
                eng.block(id)
                    .map(|b| b.config.clone())
                    .unwrap_or_else(|| serde_json::Value::Object(Default::default()))
            }),
            None => serde_json::Value::Object(Default::default()),
        }
    });

    let channels_text = Signal::derive(move || {
        let _rev = revision.get();
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return String::new(),
        };
        with_engine_ref(|eng| {
            let blk = match eng.block(sel) {
                Some(b) => b,
                None => return String::new(),
            };
            let block = match blk.reconstruct() {
                Some(b) => b,
                None => return String::new(),
            };

            // Declared channels from the block schema.
            let chs = block.declared_channels();
            let mut lines: Vec<String> = chs
                .iter()
                .map(|ch| {
                    let dir = match ch.direction {
                        ChannelDirection::Input => "IN",
                        ChannelDirection::Output => "OUT",
                    };
                    let kind = format!("{:?}", ch.kind).to_lowercase();
                    format!("{} {} [{}]", dir, ch.name, kind)
                })
                .collect();

            // Connected channels from the engine.
            let connected = eng.channels_for_block(sel);
            if !connected.is_empty() {
                lines.push(String::new());
                lines.push("-- Connections --".into());
                for ch in connected {
                    if ch.from_block == sel {
                        lines.push(format!(
                            "  OUT[{}] -> block {} IN[{}] ({})",
                            ch.from_port, ch.to_block, ch.to_port, ch.topic
                        ));
                    } else {
                        lines.push(format!(
                            "  IN[{}] <- block {} OUT[{}] ({})",
                            ch.to_port, ch.from_block, ch.from_port, ch.topic
                        ));
                    }
                }
            }

            lines.join("\n")
        })
    });

    let il_text = Signal::derive(move || {
        let _rev = revision.get();
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return String::new(),
        };
        with_engine_ref(|eng| {
            eng.block(sel)
                .and_then(|b| b.reconstruct())
                .map(|block| {
                    lower::lower_to_il_text(block.as_ref())
                        .unwrap_or_else(|e| format!("Error: {}", e))
                })
                .unwrap_or_default()
        })
    });

    // ── Edges from engine channels ──────────────────────────────────────────

    // Structured edge data derived from the engine's channels.
    let edges = Signal::derive(move || {
        let _rev = revision.get();
        with_engine_ref(|eng| eng.channels().to_vec())
    });

    // ── Deploy status ───────────────────────────────────────────────────────
    let (deploy_status, set_deploy_status) = signal(String::new());

    // ── Sim state signals ───────────────────────────────────────────────────
    let (sim_topics, set_sim_topics) = signal(std::collections::BTreeMap::<String, f64>::new());
    let (sim_tick_count, set_sim_tick_count) = signal(0_u64);
    let (sim_running, set_sim_running) = signal(false);

    // ── Callbacks ───────────────────────────────────────────────────────────

    // Add block from palette.
    let on_add_block = Callback::new(move |block_type: String| {
        if let Some(block) = registry::create_block(&block_type) {
            let config = block.config_json();
            let id = with_engine(|eng| eng.add_block(&block_type, config));
            if let Some(id) = id {
                let count = with_engine_ref(|eng| eng.block_count());
                let x = 30.0 + ((count - 1) % 3) as f64 * 220.0;
                let y = 30.0 + ((count - 1) / 3) as f64 * 120.0;
                set_positions.update(|p| {
                    p.insert(id, (x, y));
                });
                sync_shared();
                bump();
                set_selected_id.set(Some(id));
                set_selected_channel.set(None);
            }
        }
    });

    // Config change handler.
    let on_config_change = Callback::new(move |(key, value): (String, serde_json::Value)| {
        let sel = match selected_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        with_engine(|eng| eng.update_config(sel, key, value));
        sync_shared();
        bump();
    });

    // Delete selected block or channel.
    let on_delete = move |_| {
        // If a channel is selected, delete the channel.
        if let Some(ch_id) = selected_channel.get_untracked() {
            with_engine(|eng| eng.disconnect(ch_id));
            set_selected_channel.set(None);
            sync_shared();
            bump();
            return;
        }
        // Otherwise delete selected block.
        if let Some(sel) = selected_id.get_untracked() {
            with_engine(|eng| eng.remove_block(sel));
            set_positions.update(|p| {
                p.remove(&sel);
            });
            sync_shared();
            bump();
            set_selected_id.set(None);
        }
    };

    // ── Simulation handlers ─────────────────────────────────────────────────

    let on_step = move |_| {
        let result = with_engine(|eng| eng.tick());
        match result {
            Ok(()) => {
                let (topics, count) = with_engine_ref(|eng| (eng.topics(), eng.tick_count()));
                set_sim_topics.set(topics);
                set_sim_tick_count.set(count);
                set_deploy_status.set(format!(
                    "Tick {} ({} topics)",
                    count,
                    sim_topics.get_untracked().len()
                ));
            }
            Err(e) => set_deploy_status.set(e),
        }
    };

    let on_reset = move |_| {
        with_engine(|eng| eng.reset_sim());
        set_sim_topics.set(std::collections::BTreeMap::new());
        set_sim_tick_count.set(0);
        set_sim_running.set(false);
        set_deploy_status.set("Reset".into());
    };

    let on_play_pause = move |_| {
        let running = sim_running.get();
        if running {
            set_sim_running.set(false);
            set_deploy_status.set("Paused".into());
        } else {
            // Verify DAG can build.
            let result = with_engine_ref(|eng| eng.build_dag());
            if let Err(e) = result {
                set_deploy_status.set(e);
                return;
            }
            set_sim_running.set(true);
            set_deploy_status.set("Running...".into());

            gloo_timers::callback::Interval::new(100, move || {
                if !sim_running.get_untracked() {
                    return;
                }
                let result = with_engine(|eng| eng.tick());
                if result.is_ok() {
                    let (topics, count) = with_engine_ref(|eng| (eng.topics(), eng.tick_count()));
                    set_sim_topics.set(topics);
                    set_sim_tick_count.set(count);
                }
            })
            .forget();
        }
    };

    // Deploy: lower all blocks, merge DAGs, CBOR encode, POST to MCU.
    let on_deploy = move |_| {
        let dag_result = with_engine_ref(|eng| eng.build_dag());
        let dag = match dag_result {
            Ok(d) => d,
            Err(e) => {
                set_deploy_status.set(e);
                return;
            }
        };

        let cbor_bytes = dag_core::cbor::encode_dag(&dag);
        let node_count = dag.len();

        set_deploy_status.set(format!(
            "Deploying {} nodes ({} bytes)...",
            node_count,
            cbor_bytes.len()
        ));

        let status_setter = set_deploy_status;
        wasm_bindgen_futures::spawn_local(async move {
            match deploy_to_mcu(&cbor_bytes).await {
                Ok(msg) => status_setter.set(format!("Deployed: {}", msg)),
                Err(e) => status_setter.set(format!("Deploy failed: {}", e)),
            }
        });
    };

    // Tick MCU remotely.
    let _on_tick = move |_: web_sys::MouseEvent| {
        let status_setter = set_deploy_status;
        wasm_bindgen_futures::spawn_local(async move {
            match tick_mcu().await {
                Ok(msg) => status_setter.set(format!("Tick: {}", msg)),
                Err(e) => status_setter.set(format!("Tick failed: {}", e)),
            }
        });
    };

    // ── Sidebar callbacks ───────────────────────────────────────────────────

    let on_save = Callback::new(move |()| {
        let name = project_name.get_untracked();
        if name.is_empty() {
            set_deploy_status.set("Enter a project name first".into());
            return;
        }
        let snapshot = with_engine_ref(|eng| eng.snapshot());
        let pos = positions.get_untracked();
        let project = SavedProject {
            name,
            snapshot,
            positions: pos,
            saved_at: String::new(),
        };
        match storage::save_project(&project) {
            Ok(()) => set_deploy_status.set("Project saved".into()),
            Err(e) => set_deploy_status.set(format!("Save error: {e}")),
        }
    });

    let on_load = Callback::new(move |name: String| match storage::load_project(&name) {
        Ok(project) => {
            with_engine(|eng| eng.restore(&project.snapshot));
            set_positions.set(project.positions);
            set_project_name.set(project.name);
            set_selected_id.set(None);
            set_selected_channel.set(None);
            sync_shared();
            bump();
            set_deploy_status.set("Project loaded".into());
        }
        Err(e) => set_deploy_status.set(format!("Load error: {e}")),
    });

    let on_new = Callback::new(move |()| {
        with_engine(|eng| {
            *eng = GraphEngine::new();
        });
        set_positions.set(HashMap::new());
        set_project_name.set("untitled".into());
        set_selected_id.set(None);
        set_selected_channel.set(None);
        sync_shared();
        bump();
        set_deploy_status.set("New project".into());
    });

    // ── Keyboard handler (Delete key for edge deletion) ─────────────────────

    // We handle keydown on the SVG container div.
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Delete" || ev.key() == "Backspace" {
            if let Some(ch_id) = selected_channel.get_untracked() {
                with_engine(|eng| eng.disconnect(ch_id));
                set_selected_channel.set(None);
                sync_shared();
                bump();
                ev.prevent_default();
            }
        }
    };

    // ── Local state for transport/batch/export ────────────────────────────
    let (speed, set_speed) = signal(1.0_f64);
    let (dt, set_dt) = signal(0.01_f64);
    let (batch_input, set_batch_input) = signal("100".to_string());

    // Export target checkboxes
    let (export_host, set_export_host) = signal(true);
    let (export_rp2040, set_export_rp2040) = signal(false);
    let (export_stm32f4, set_export_stm32f4) = signal(false);
    let (export_esp32c3, set_export_esp32c3) = signal(false);

    // Batch run handler
    let on_batch_run = move |_| {
        if let Ok(n) = batch_input.get_untracked().parse::<u32>() {
            if n > 0 {
                // Build DAG first
                let result = with_engine_ref(|eng| eng.build_dag());
                if let Err(e) = result {
                    set_deploy_status.set(e);
                    return;
                }
                for _ in 0..n {
                    let _ = with_engine(|eng| eng.tick());
                }
                let (topics, count) = with_engine_ref(|eng| (eng.topics(), eng.tick_count()));
                set_sim_topics.set(topics);
                set_sim_tick_count.set(count);
                set_deploy_status.set(format!("Batch: {} ticks done (tick {})", n, count));
            }
        }
    };

    // ── View ────────────────────────────────────────────────────────────────

    view! {
        <div class="dag-editor-layout">
            // ── Left sidebar: all controls stacked ──────────────────────────
            <div class="dag-sidebar">
                // Project section
                <section>
                    <ProjectSidebar
                        project_name=project_name
                        set_project_name=set_project_name
                        on_save=on_save
                        on_load=on_load
                        on_new=on_new
                    />
                </section>

                // Blocks palette section
                <section>
                    <h2 class="dag-section-head">"BLOCKS"</h2>
                    <BlockPalette on_add=on_add_block />
                </section>

                // Inspector section
                <section>
                    <h2 class="dag-section-head">"INSPECTOR"</h2>
                    <ConfigPanel
                        block_type=selected_block_type
                        config_fields=config_fields
                        config_values=config_values
                        on_change=on_config_change
                        channels_text=channels_text
                        il_text=il_text
                    />
                    <button class="btn btn-danger btn-sm" style="margin-top:6px" on:click=on_delete>
                        "Delete Block"
                    </button>
                </section>

                // Transport section
                <section>
                    <h2 class="dag-section-head">"TRANSPORT"</h2>
                    <div class="dag-transport-row">
                        <button
                            class=move || if sim_running.get() { "btn btn-danger btn-sm" } else { "btn btn-primary btn-sm" }
                            on:click=on_play_pause
                        >
                            {move || if sim_running.get() { "Pause" } else { "Play" }}
                        </button>
                        <button class="btn btn-secondary btn-sm" on:click=on_step>"Step"</button>
                        <button class="btn btn-secondary btn-sm" on:click=on_reset>"Reset"</button>
                    </div>
                    <div class="dag-transport-row">
                        <div class="dag-transport-field">
                            "dt:"
                            <input
                                type="number"
                                prop:value=move || format!("{}", dt.get())
                                step="0.001"
                                min="0.0001"
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    if let Ok(d) = val.parse::<f64>() {
                                        if d > 0.0 { set_dt.set(d); }
                                    }
                                }
                            />
                        </div>
                        <div class="dag-transport-field">
                            "Speed:"
                            <select
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    if let Ok(s) = val.parse::<f64>() { set_speed.set(s); }
                                }
                            >
                                {SPEED_PRESETS.iter().map(|(val, label)| {
                                    let selected = *val == 1.0;
                                    let val_str = val.to_string();
                                    let label_str = label.to_string();
                                    view! {
                                        <option value=val_str selected=selected>{label_str}</option>
                                    }
                                }).collect_view()}
                            </select>
                        </div>
                    </div>
                    <div class="dag-transport-info">
                        {move || format!(
                            "Tick {} | t = {} | {}x",
                            sim_tick_count.get(),
                            format_sim_time(sim_tick_count.get(), dt.get()),
                            speed.get(),
                        )}
                    </div>
                    <span class="dag-status">{move || deploy_status.get()}</span>
                </section>

                // Batch section
                <section>
                    <h2 class="dag-section-head">"BATCH"</h2>
                    <div class="dag-batch-row">
                        <span style="font-size:0.75rem;color:var(--text-dim)">"Steps:"</span>
                        <input
                            type="number"
                            prop:value=move || batch_input.get()
                            min="1"
                            on:input=move |ev| { set_batch_input.set(event_target_value(&ev)); }
                        />
                        <button class="btn btn-primary btn-sm" on:click=on_batch_run>"Run"</button>
                    </div>
                </section>

                // Export section
                <section>
                    <h2 class="dag-section-head">"EXPORT"</h2>
                    <div class="dag-export-targets">
                        <label class="dag-export-target">
                            <input type="checkbox" prop:checked=move || export_host.get()
                                on:change=move |ev| { set_export_host.set(event_target_checked(&ev)); } />
                            "Host (Sim)"
                        </label>
                        <label class="dag-export-target">
                            <input type="checkbox" prop:checked=move || export_rp2040.get()
                                on:change=move |ev| { set_export_rp2040.set(event_target_checked(&ev)); } />
                            "RP2040"
                        </label>
                        <label class="dag-export-target">
                            <input type="checkbox" prop:checked=move || export_stm32f4.get()
                                on:change=move |ev| { set_export_stm32f4.set(event_target_checked(&ev)); } />
                            "STM32F4"
                        </label>
                        <label class="dag-export-target">
                            <input type="checkbox" prop:checked=move || export_esp32c3.get()
                                on:change=move |ev| { set_export_esp32c3.set(event_target_checked(&ev)); } />
                            "ESP32-C3"
                        </label>
                    </div>
                    <button class="btn btn-primary btn-sm" style="width:100%">"Generate & Download"</button>
                </section>

                // HIL section
                <section>
                    <h2 class="dag-section-head">"HIL"</h2>
                    <div class="dag-hil-url">"ws://169.254.1.61:8080"</div>
                    <div class="dag-hil-btns">
                        <button class="btn btn-secondary btn-sm">"Connect"</button>
                        <button class="btn btn-primary btn-sm" on:click=on_deploy>"Deploy MCU"</button>
                    </div>
                </section>

                // Monitor section (collapsible)
                <section>
                    <MonitorPanel topics=sim_topics tick_count=sim_tick_count />
                </section>
            </div>

            // ── Center canvas ───────────────────────────────────────────────
            <div
                class="dag-canvas-container"
                tabindex="0"
                on:keydown=on_keydown
            >
                <svg
                    class="dag-canvas"
                    viewBox="0 0 900 500"
                    on:mousedown=move |ev: web_sys::MouseEvent| {
                        let target = match ev.target() {
                            Some(t) => t,
                            None => return,
                        };
                        let el: web_sys::Element = match target.dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };

                        // 1. Edge selection (data-channel-id).
                        if let Some(ch_id_str) = el.get_attribute("data-channel-id") {
                            if let Ok(ch_id) = ch_id_str.parse::<u32>() {
                                set_selected_channel.set(Some(ch_id));
                                set_selected_id.set(None);
                                ev.prevent_default();
                                return;
                            }
                        }

                        // 2. Wire drag from output port (data-side="out").
                        let side = el.get_attribute("data-side").unwrap_or_default();
                        if side == "out" {
                            let block_id: BlockId = match el.get_attribute("data-block-id")
                                .and_then(|s| s.parse().ok()) {
                                Some(v) => v,
                                None => return,
                            };
                            let port_idx: usize = match el.get_attribute("data-port-idx")
                                .and_then(|s| s.parse().ok()) {
                                Some(v) => v,
                                None => return,
                            };
                            let svg_el = match el.closest("svg") {
                                Ok(Some(s)) => s,
                                _ => return,
                            };
                            let (mx, my) = client_to_world(
                                &svg_el,
                                ev.client_x() as f64,
                                ev.client_y() as f64,
                                pan_x.get_untracked(),
                                pan_y.get_untracked(),
                                zoom.get_untracked(),
                            );
                            set_dragging_wire.set(Some(DraggingWire {
                                from_block: block_id,
                                from_port: port_idx,
                                mouse_x: mx,
                                mouse_y: my,
                            }));
                            ev.prevent_default();
                            return;
                        }

                        // Also allow wire drag start from input ports — skip them for node drag.
                        if side == "in" {
                            return;
                        }

                        // 3. Node drag: walk up the DOM looking for data-block-id on a <g>.
                        {
                            let mut current: Option<web_sys::Element> = Some(el.clone());
                            while let Some(node) = current {
                                if node.has_attribute("data-block-id") {
                                    if let Some(bid_str) = node.get_attribute("data-block-id") {
                                        if let Ok(bid) = bid_str.parse::<BlockId>() {
                                            let svg_el = match node.closest("svg") {
                                                Ok(Some(s)) => s,
                                                _ => return,
                                            };
                                            let (wx, wy) = client_to_world(
                                                &svg_el,
                                                ev.client_x() as f64,
                                                ev.client_y() as f64,
                                                pan_x.get_untracked(),
                                                pan_y.get_untracked(),
                                                zoom.get_untracked(),
                                            );
                                            let pos = positions.get_untracked();
                                            let (nx, ny) = pos.get(&bid).copied().unwrap_or((0.0, 0.0));
                                            set_dragging_node.set(Some(DraggingNode {
                                                block_id: bid,
                                                start_mouse_x: wx,
                                                start_mouse_y: wy,
                                                start_node_x: nx,
                                                start_node_y: ny,
                                                moved: false,
                                            }));
                                            ev.prevent_default();
                                            return;
                                        }
                                    }
                                }
                                current = node.parent_element();
                            }
                        }

                        // 4. Pan: shift+click or middle mouse button.
                        if ev.shift_key() || ev.button() == 1 {
                            let svg_el = match el.closest("svg") {
                                Ok(Some(s)) => s,
                                _ => return,
                            };
                            let rect = svg_el.get_bounding_client_rect();
                            let sx = (ev.client_x() as f64 - rect.left()) * 900.0 / rect.width();
                            let sy = (ev.client_y() as f64 - rect.top()) * 500.0 / rect.height();
                            set_panning.set(Some(Panning {
                                start_mouse_x: sx,
                                start_mouse_y: sy,
                                start_pan_x: pan_x.get_untracked(),
                                start_pan_y: pan_y.get_untracked(),
                            }));
                            ev.prevent_default();
                            return;
                        }

                        // 5. Clicked on empty canvas — deselect.
                        set_selected_channel.set(None);
                        set_selected_id.set(None);
                    }
                    on:mousemove=move |ev: web_sys::MouseEvent| {
                        // Node drag.
                        if let Some(dn) = dragging_node.get_untracked() {
                            let svg_el_opt = ev.current_target()
                                .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                            let svg_el = match svg_el_opt {
                                Some(e) => e,
                                None => return,
                            };
                            let (wx, wy) = client_to_world(
                                &svg_el,
                                ev.client_x() as f64,
                                ev.client_y() as f64,
                                pan_x.get_untracked(),
                                pan_y.get_untracked(),
                                zoom.get_untracked(),
                            );
                            let dx = wx - dn.start_mouse_x;
                            let dy = wy - dn.start_mouse_y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            let moved = dn.moved || dist > DRAG_THRESHOLD;
                            if moved {
                                let new_x = dn.start_node_x + dx;
                                let new_y = dn.start_node_y + dy;
                                set_positions.update(|p| {
                                    p.insert(dn.block_id, (new_x, new_y));
                                });
                            }
                            set_dragging_node.set(Some(DraggingNode {
                                moved,
                                ..dn
                            }));
                            ev.prevent_default();
                            return;
                        }

                        // Pan drag.
                        if let Some(pan) = panning.get_untracked() {
                            let svg_el_opt = ev.current_target()
                                .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                            let svg_el = match svg_el_opt {
                                Some(e) => e,
                                None => return,
                            };
                            let rect = svg_el.get_bounding_client_rect();
                            let sx = (ev.client_x() as f64 - rect.left()) * 900.0 / rect.width();
                            let sy = (ev.client_y() as f64 - rect.top()) * 500.0 / rect.height();
                            set_pan_x.set(pan.start_pan_x + (sx - pan.start_mouse_x));
                            set_pan_y.set(pan.start_pan_y + (sy - pan.start_mouse_y));
                            ev.prevent_default();
                            return;
                        }

                        // Wire drag.
                        if dragging_wire.get_untracked().is_some() {
                            let target = match ev.current_target() {
                                Some(t) => t,
                                None => return,
                            };
                            let svg_el: web_sys::Element = match target.dyn_into() {
                                Ok(e) => e,
                                Err(_) => return,
                            };
                            let (mx, my) = client_to_world(
                                &svg_el,
                                ev.client_x() as f64,
                                ev.client_y() as f64,
                                pan_x.get_untracked(),
                                pan_y.get_untracked(),
                                zoom.get_untracked(),
                            );
                            set_dragging_wire.update(|dw| {
                                if let Some(ref mut w) = dw {
                                    w.mouse_x = mx;
                                    w.mouse_y = my;
                                }
                            });
                        }
                    }
                    on:mouseup=move |ev: web_sys::MouseEvent| {
                        // Finish node drag.
                        if let Some(dn) = dragging_node.get_untracked() {
                            set_dragging_node.set(None);
                            if dn.moved {
                                // Position already updated in mousemove; just bump revision.
                                bump();
                            } else {
                                // No movement — treat as a click to select.
                                set_selected_id.set(Some(dn.block_id));
                                set_selected_channel.set(None);
                            }
                            return;
                        }

                        // Finish panning.
                        if panning.get_untracked().is_some() {
                            set_panning.set(None);
                            return;
                        }

                        // Finish wire drag.
                        let wire = match dragging_wire.get_untracked() {
                            Some(w) => w,
                            None => return,
                        };
                        set_dragging_wire.set(None);

                        let target = match ev.target() {
                            Some(t) => t,
                            None => return,
                        };
                        let el: web_sys::Element = match target.dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let side = el.get_attribute("data-side").unwrap_or_default();
                        if side != "in" {
                            return;
                        }
                        let to_block: BlockId = match el.get_attribute("data-block-id")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };
                        let to_port: usize = match el.get_attribute("data-port-idx")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };

                        if wire.from_block == to_block {
                            return;
                        }

                        // Store edge in GraphEngine.
                        let ch_id = with_engine(|eng| {
                            eng.connect(wire.from_block, wire.from_port, to_block, to_port)
                        });

                        if ch_id.is_some() {
                            // Also update block configs with auto-topic names for codegen compat.
                            let auto_topic = with_engine_ref(|eng| {
                                eng.channels()
                                    .iter()
                                    .find(|ch| Some(ch.id) == ch_id)
                                    .map(|ch| ch.topic.clone())
                                    .unwrap_or_default()
                            });
                            update_block_config_topic(wire.from_block, wire.from_port, ChannelDirection::Output, &auto_topic);
                            update_block_config_topic(to_block, to_port, ChannelDirection::Input, &auto_topic);
                            sync_shared();
                            bump();
                        }
                    }
                    on:wheel=move |ev: web_sys::WheelEvent| {
                        ev.prevent_default();
                        let svg_el_opt = ev.current_target()
                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                        let svg_el = match svg_el_opt {
                            Some(e) => e,
                            None => return,
                        };
                        let rect = svg_el.get_bounding_client_rect();
                        // Mouse position in SVG viewport coords.
                        let mx = (ev.client_x() as f64 - rect.left()) * 900.0 / rect.width();
                        let my = (ev.client_y() as f64 - rect.top()) * 500.0 / rect.height();

                        let old_zoom = zoom.get_untracked();
                        let direction = if ev.delta_y() < 0.0 { 1.0 } else { -1.0 };
                        let new_zoom = (old_zoom * ZOOM_FACTOR.powf(direction)).clamp(ZOOM_MIN, ZOOM_MAX);

                        // Zoom toward mouse: adjust pan so the world point under cursor stays fixed.
                        let old_pan_x = pan_x.get_untracked();
                        let old_pan_y = pan_y.get_untracked();
                        set_pan_x.set(mx - (mx - old_pan_x) * new_zoom / old_zoom);
                        set_pan_y.set(my - (my - old_pan_y) * new_zoom / old_zoom);
                        set_zoom.set(new_zoom);
                    }
                >
                    <g class="dag-world" transform=move || format!("translate({},{}) scale({})", pan_x.get(), pan_y.get(), zoom.get())>
                    // Dashed drag line
                    {move || {
                        let dw = dragging_wire.get();
                        let pos = positions.get();
                        dw.and_then(|w| {
                            let (sx, sy) = pos.get(&w.from_block)?;
                            let x1 = sx + NODE_WIDTH;
                            let y1 = sy + PORT_Y_START + w.from_port as f64 * PORT_SPACING;
                            let x2 = w.mouse_x;
                            let y2 = w.mouse_y;
                            let cpx = f64::max((x2 - x1).abs() * 0.4, 30.0);
                            let d = format!(
                                "M {},{} C {},{} {},{} {},{}",
                                x1, y1,
                                x1 + cpx, y1,
                                x2 - cpx, y2,
                                x2, y2
                            );
                            Some(view! {
                                <path
                                    d=d
                                    fill="none"
                                    stroke="#f59e0b"
                                    stroke-width="2"
                                    stroke-dasharray="6 3"
                                    class="dag-edge-drag"
                                />
                            })
                        })
                    }}
                    {move || {
                        let _rev = revision.get();
                        let pos = positions.get();
                        let edge_list = edges.get();
                        let sel_ch = selected_channel.get();

                        // Edge paths
                        let edge_views = edge_list.iter().filter_map(|edge| {
                            let (sx, sy) = pos.get(&edge.from_block)?;
                            let (dx, dy) = pos.get(&edge.to_block)?;
                            let x1 = sx + NODE_WIDTH;
                            let y1 = sy + PORT_Y_START + edge.from_port as f64 * PORT_SPACING;
                            let x2 = *dx;
                            let y2 = dy + PORT_Y_START + edge.to_port as f64 * PORT_SPACING;
                            let cpx = f64::max((x2 - x1).abs() * 0.4, 30.0);
                            let d = format!(
                                "M {},{} C {},{} {},{} {},{}",
                                x1, y1,
                                x1 + cpx, y1,
                                x2 - cpx, y2,
                                x2, y2
                            );
                            let is_selected = sel_ch == Some(edge.id);
                            let stroke = if is_selected { "#ef4444" } else { "#6b7280" };
                            let width = if is_selected { "3" } else { "2" };
                            let ch_id_str = edge.id.to_string();
                            // Invisible fat hit area for easier click target
                            let hit_d = d.clone();
                            let hit_ch_id = ch_id_str.clone();
                            Some(view! {
                                <path
                                    d=hit_d
                                    fill="none"
                                    stroke="transparent"
                                    stroke-width="12"
                                    attr:data-channel-id=hit_ch_id
                                    class="dag-edge-hit"
                                    style="cursor:pointer"
                                />
                                <path
                                    d=d
                                    fill="none"
                                    stroke=stroke
                                    stroke-width=width
                                    attr:data-channel-id=ch_id_str
                                    class="dag-edge"
                                    style="pointer-events:none"
                                />
                            })
                        }).collect_view();

                        // Block nodes
                        let blocks_data: Vec<_> = with_engine_ref(|eng| {
                            eng.blocks().iter().map(|b| {
                                let block = b.reconstruct();
                                let name = block.as_ref()
                                    .map(|bl| bl.display_name().to_string())
                                    .unwrap_or_else(|| b.block_type.clone());
                                let bt = b.block_type.clone();
                                let channels = block.as_ref()
                                    .map(|bl| bl.declared_channels())
                                    .unwrap_or_default();
                                (b.id, name, bt, channels)
                            }).collect()
                        });

                        let node_views = blocks_data.into_iter().map(|(id, name, bt, channels)| {
                            let (x, y) = pos.get(&id).copied().unwrap_or((30.0, 30.0));
                            let is_selected = move || selected_id.get() == Some(id);
                            let in_count = channels.iter()
                                .filter(|c| c.direction == ChannelDirection::Input).count();
                            let out_count = channels.iter()
                                .filter(|c| c.direction == ChannelDirection::Output).count();
                            let height = 50.0 + (in_count.max(out_count) as f64) * PORT_SPACING;
                            let node_w_str = NODE_WIDTH.to_string();

                            view! {
                                <g
                                    class=move || if is_selected() { "dag-node selected" } else { "dag-node" }
                                    transform=format!("translate({},{})", x, y)
                                    attr:data-block-id=id.to_string()
                                >
                                    <rect
                                        width=node_w_str.clone() height=height rx="6" ry="6"
                                        class="dag-node-rect"
                                    />
                                    <text x="95" y="18" class="dag-node-title">{name}</text>
                                    <text x="95" y="32" class="dag-node-type">{bt}</text>
                                    // Input ports
                                    {channels.iter().filter(|c| c.direction == ChannelDirection::Input).enumerate().map(|(i, ch)| {
                                        let py = PORT_Y_START + i as f64 * PORT_SPACING;
                                        let label = ch.name.clone();
                                        let bid = id.to_string();
                                        let pidx = i.to_string();
                                        view! {
                                            <circle
                                                cx="0" cy=py r=PORT_RADIUS
                                                class="dag-port dag-port-in"
                                                attr:data-block-id=bid
                                                attr:data-port-idx=pidx
                                                attr:data-side="in"
                                            />
                                            <text x="8" y=py + 4.0 class="dag-port-label">{label}</text>
                                        }
                                    }).collect_view()}
                                    // Output ports
                                    {channels.iter().filter(|c| c.direction == ChannelDirection::Output).enumerate().map(|(i, ch)| {
                                        let py = PORT_Y_START + i as f64 * PORT_SPACING;
                                        let label = ch.name.clone();
                                        let bid = id.to_string();
                                        let pidx = i.to_string();
                                        view! {
                                            <circle
                                                cx=node_w_str.clone() cy=py r=PORT_RADIUS
                                                class="dag-port dag-port-out"
                                                attr:data-block-id=bid
                                                attr:data-port-idx=pidx
                                                attr:data-side="out"
                                            />
                                            <text x="182" y=py + 4.0 class="dag-port-label dag-port-label-right">{label}</text>
                                        }
                                    }).collect_view()}
                                </g>
                            }
                        }).collect_view();

                        view! {
                            <g class="dag-edges">{edge_views}</g>
                            <g class="dag-nodes">{node_views}</g>
                        }
                    }}
                    </g>
                </svg>
            </div>

            // ── Right pane (placeholder for Plot/Pins/I2C) ──────────────────
            <div class="dag-right-pane">
                <div class="dag-right-tabs">
                    <button class="dag-right-tab active">"PLOT"</button>
                    <button class="dag-right-tab">"PINS"</button>
                    <button class="dag-right-tab">"I2C"</button>
                </div>
                <div class="dag-right-content">
                    <p class="text-dim" style="font-size: 12px;">
                        "Plot will appear here during simulation"
                    </p>
                </div>
            </div>
        </div>
    }
}

/// Update a block's config in the engine to set the topic name for a specific port.
///
/// Finds the config key corresponding to the given channel direction and port index,
/// then sets it to `topic`.
fn update_block_config_topic(
    block_id: BlockId,
    port_idx: usize,
    direction: ChannelDirection,
    topic: &str,
) {
    with_engine(|eng| {
        let config = match eng.block(block_id) {
            Some(b) => b.config.clone(),
            None => return,
        };
        let block = match eng.block(block_id).and_then(|b| b.reconstruct()) {
            Some(b) => b,
            None => return,
        };
        let channels = block.declared_channels();
        let filtered: Vec<_> = channels
            .iter()
            .filter(|c| c.direction == direction)
            .collect();
        if let Some(ch) = filtered.get(port_idx) {
            if let Some(key) = find_config_key_for_channel(&config, &ch.name) {
                eng.update_config(block_id, key, serde_json::Value::String(topic.to_string()));
            }
        }
    });
}

/// Find the config key whose current value matches `channel_name`.
fn find_config_key_for_channel(config: &serde_json::Value, channel_name: &str) -> Option<String> {
    let obj = config.as_object()?;
    for (key, val) in obj {
        if let Some(s) = val.as_str() {
            if s == channel_name {
                return Some(key.clone());
            }
        }
    }
    None
}

/// Convert mouse client coordinates to world coordinates (SVG viewport -> inverse of pan/zoom).
fn client_to_world(
    svg: &web_sys::Element,
    client_x: f64,
    client_y: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> (f64, f64) {
    let rect = svg.get_bounding_client_rect();
    let rect_w = rect.width();
    let rect_h = rect.height();
    // Map to SVG viewport coords (viewBox is 900x500).
    let vb_w = 900.0;
    let vb_h = 500.0;
    let sx = (client_x - rect.left()) * vb_w / rect_w;
    let sy = (client_y - rect.top()) * vb_h / rect_h;
    // Invert the world transform: world = (svg - pan) / zoom
    ((sx - pan_x) / zoom, (sy - pan_y) / zoom)
}

/// POST CBOR DAG to the Pico2 HTTP API.
async fn deploy_to_mcu(cbor_bytes: &[u8]) -> Result<String, String> {
    use js_sys::Uint8Array;

    let window = web_sys::window().ok_or("no window")?;

    let array = Uint8Array::from(cbor_bytes);

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&array.into());

    let headers = web_sys::Headers::new().map_err(|e| format!("{:?}", e))?;
    headers
        .set("Content-Type", "application/cbor")
        .map_err(|e| format!("{:?}", e))?;
    opts.set_headers(&headers);

    let url = "http://169.254.1.61:8080/api/dag";
    let request =
        web_sys::Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{:?}", e))?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "not a Response".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("{:?}", e))?;

    Ok(text.as_string().unwrap_or_default())
}

/// POST /api/tick to evaluate the deployed DAG on the MCU.
async fn tick_mcu() -> Result<String, String> {
    let window = web_sys::window().ok_or("no window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");

    let url = "http://169.254.1.61:8080/api/tick";
    let request =
        web_sys::Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{:?}", e))?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "not a Response".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("{:?}", e))?;

    Ok(text.as_string().unwrap_or_default())
}

/// Extract the checked state from a checkbox change event.
fn event_target_checked(ev: &leptos::ev::Event) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}
