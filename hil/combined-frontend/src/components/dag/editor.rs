//! DAG editor panel: palette, canvas, config, deploy.
//!
//! Uses [`GraphState`] for block/channel management and simulation,
//! [`storage`] for localStorage persistence, and [`ProjectSidebar`]
//! for project load/save UI.
//!
//! The canvas is split into three stacked layers:
//! 1. Grid background (CSS background-image)
//! 2. SVG edge layer (`EdgePath` components + wire drag preview)
//! 3. HTML node layer (`BlockNode` components)

use std::cell::RefCell;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::lower;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;

use crate::graph_engine::{BlockId, ChannelId};
use crate::graph_state::GraphState;
use crate::sim_util::{format_sim_time, SPEED_PRESETS};
use crate::types::BlockSet;

use super::config_panel::ConfigPanel;
use super::edge::EdgePath;
use super::monitor::MonitorPanel;
use super::node::{BlockNode, PortDef};
use super::palette::BlockPalette;
use super::port::WireDrag;
use super::sidebar::ProjectSidebar;
use super::storage::{self, SavedProject};

// -- Thread-local GraphState (not Send, same pattern as SimState) ------------

thread_local! {
    static STATE: RefCell<GraphState> = RefCell::new(GraphState::new());
}

/// Run a closure with a mutable reference to the state.
fn with_state<R>(f: impl FnOnce(&mut GraphState) -> R) -> R {
    STATE.with(|cell| f(&mut cell.borrow_mut()))
}

/// Run a closure with an immutable reference to the state.
fn with_state_ref<R>(f: impl FnOnce(&GraphState) -> R) -> R {
    STATE.with(|cell| f(&cell.borrow()))
}

// -- Wire drag state --------------------------------------------------------

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
    // -- Layout / interaction constants -------------------------------------
    const NODE_WIDTH: f64 = 190.0;
    const PORT_SPACING: f64 = 20.0;
    const PORT_Y_START: f64 = 46.0;
    const ZOOM_FACTOR: f64 = 1.1;
    const ZOOM_MIN: f64 = 0.2;
    const ZOOM_MAX: f64 = 5.0;
    const DRAG_THRESHOLD: f64 = 3.0;

    // Revision counter -- bumped after every state mutation to trigger re-reads.
    let (revision, set_revision) = signal(0_u64);
    let bump = move || set_revision.update(|r| *r += 1);

    // Pan / zoom state.
    let (pan_x, set_pan_x) = signal(0.0_f64);
    let (pan_y, set_pan_y) = signal(0.0_f64);
    let (zoom, set_zoom) = signal(1.0_f64);

    // Node drag state.
    let (dragging_node, set_dragging_node) = signal(None::<DraggingNode>);

    // Panning state.
    let (panning, set_panning) = signal(None::<Panning>);

    // Wire drag state.
    let (dragging_wire, set_dragging_wire) = signal(None::<WireDrag>);

    // Shared block-set context: push (block_type, config) pairs to deploy panel.
    let set_shared_blocks = use_context::<WriteSignal<BlockSet>>();

    // Sync engine blocks -> shared context.
    let sync_shared = move || {
        if let Some(setter) = set_shared_blocks {
            let block_set: BlockSet = with_state_ref(|s| {
                s.engine()
                    .blocks()
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

    // Project name.
    let (project_name, set_project_name) = signal("untitled".to_string());

    // -- Auto-save debounce -------------------------------------------------

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
        let positions = with_state_ref(|s| s.positions().clone());
        let timeout = gloo_timers::callback::Timeout::new(2_000, move || {
            let snapshot = with_state_ref(|s| s.engine().snapshot());
            let project = SavedProject {
                name: name.clone(),
                snapshot,
                positions,
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

    // -- Config signals derived from selection ------------------------------

    let selected_block_type = Signal::derive(move || {
        let _rev = revision.get();
        let sel = selected_id.get()?;
        with_state_ref(|s| {
            let blk = s.engine().block(sel)?;
            let block = blk.reconstruct()?;
            Some(block.display_name().to_string())
        })
    });

    let config_fields = Signal::derive(move || {
        let _rev = revision.get();
        let sel = selected_id.get();
        match sel {
            Some(id) => with_state_ref(|s| {
                s.engine()
                    .block(id)
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
            Some(id) => with_state_ref(|s| {
                s.engine()
                    .block(id)
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
        with_state_ref(|s| {
            let eng = s.engine();
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
        with_state_ref(|s| {
            s.engine()
                .block(sel)
                .and_then(|b| b.reconstruct())
                .map(|block| {
                    lower::lower_to_il_text(block.as_ref())
                        .unwrap_or_else(|e| format!("Error: {}", e))
                })
                .unwrap_or_default()
        })
    });

    // -- Deploy status ------------------------------------------------------
    let (deploy_status, set_deploy_status) = signal(String::new());

    // -- Sim state signals --------------------------------------------------
    let (sim_topics, set_sim_topics) = signal(std::collections::BTreeMap::<String, f64>::new());
    let (sim_tick_count, set_sim_tick_count) = signal(0_u64);
    let (sim_running, set_sim_running) = signal(false);

    // -- Callbacks ----------------------------------------------------------

    // Add block from palette.
    let on_add_block = Callback::new(move |block_type: String| {
        if let Some(block) = registry::create_block(&block_type) {
            let config = block.config_json();
            let count = with_state_ref(|s| s.engine().block_count());
            let x = 30.0 + (count % 3) as f64 * 220.0;
            let y = 30.0 + (count / 3) as f64 * 120.0;
            let id = with_state(|s| s.add_block(&block_type, config, x, y));
            if let Some(id) = id {
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
        with_state(|s| s.update_config(sel, key, value));
        sync_shared();
        bump();
    });

    // Delete selected block or channel.
    let on_delete = move |_| {
        // If a channel is selected, delete the channel.
        if let Some(ch_id) = selected_channel.get_untracked() {
            with_state(|s| s.disconnect(ch_id));
            set_selected_channel.set(None);
            sync_shared();
            bump();
            return;
        }
        // Otherwise delete selected block.
        if let Some(sel) = selected_id.get_untracked() {
            with_state(|s| s.remove_block(sel));
            sync_shared();
            bump();
            set_selected_id.set(None);
        }
    };

    // -- Simulation handlers ------------------------------------------------

    let on_step = move |_| {
        let result = with_state(|s| s.tick());
        match result {
            Ok(()) => {
                let (topics, count) = with_state_ref(|s| (s.topics(), s.tick_count()));
                set_sim_topics.set(topics);
                set_sim_tick_count.set(count);
                set_deploy_status.set(format!(
                    "Tick {} ({} topics)",
                    count,
                    sim_topics.get_untracked().len()
                ));
                bump();
            }
            Err(e) => set_deploy_status.set(e),
        }
    };

    let on_reset = move |_| {
        with_state(|s| s.engine_mut().reset_sim());
        set_sim_topics.set(std::collections::BTreeMap::new());
        set_sim_tick_count.set(0);
        set_sim_running.set(false);
        set_deploy_status.set("Reset".into());
        bump();
    };

    let on_play_pause = move |_| {
        let running = sim_running.get();
        if running {
            set_sim_running.set(false);
            set_deploy_status.set("Paused".into());
        } else {
            // Verify DAG can build.
            let result = with_state_ref(|s| s.engine().build_dag());
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
                let result = with_state(|s| s.tick());
                if result.is_ok() {
                    let (topics, count) = with_state_ref(|s| (s.topics(), s.tick_count()));
                    set_sim_topics.set(topics);
                    set_sim_tick_count.set(count);
                    bump();
                }
            })
            .forget();
        }
    };

    // Deploy: lower all blocks, merge DAGs, CBOR encode, POST to MCU.
    let on_deploy = move |_| {
        let dag_result = with_state_ref(|s| s.engine().build_dag());
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

    // -- Sidebar callbacks --------------------------------------------------

    let on_save = Callback::new(move |()| {
        let name = project_name.get_untracked();
        if name.is_empty() {
            set_deploy_status.set("Enter a project name first".into());
            return;
        }
        let (snapshot, positions) =
            with_state_ref(|s| (s.engine().snapshot(), s.positions().clone()));
        let project = SavedProject {
            name,
            snapshot,
            positions,
            saved_at: String::new(),
        };
        match storage::save_project(&project) {
            Ok(()) => set_deploy_status.set("Project saved".into()),
            Err(e) => set_deploy_status.set(format!("Save error: {e}")),
        }
    });

    let on_load = Callback::new(move |name: String| match storage::load_project(&name) {
        Ok(project) => {
            with_state(|s| {
                s.engine_mut().restore(&project.snapshot);
                for (&id, &(x, y)) in &project.positions {
                    s.set_position(id, x, y);
                }
                s.set_selected_block(None);
                s.set_selected_edge(None);
            });
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
        STATE.with(|cell| {
            *cell.borrow_mut() = GraphState::new();
        });
        set_project_name.set("untitled".into());
        set_selected_id.set(None);
        set_selected_channel.set(None);
        sync_shared();
        bump();
        set_deploy_status.set("New project".into());
    });

    // -- Keyboard handler ---------------------------------------------------

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Delete" || ev.key() == "Backspace" {
            if let Some(ch_id) = selected_channel.get_untracked() {
                with_state(|s| s.disconnect(ch_id));
                set_selected_channel.set(None);
                sync_shared();
                bump();
                ev.prevent_default();
            } else if let Some(sel) = selected_id.get_untracked() {
                with_state(|s| s.remove_block(sel));
                set_selected_id.set(None);
                sync_shared();
                bump();
                ev.prevent_default();
            }
        }
    };

    // -- Local state for transport/batch ------------------------------------
    let (speed, set_speed) = signal(1.0_f64);
    let (dt, set_dt) = signal(0.01_f64);
    let (batch_input, set_batch_input) = signal("100".to_string());

    // Batch run handler.
    let on_batch_run = move |_| {
        if let Ok(n) = batch_input.get_untracked().parse::<u32>() {
            if n > 0 {
                // Build DAG first.
                let result = with_state_ref(|s| s.engine().build_dag());
                if let Err(e) = result {
                    set_deploy_status.set(e);
                    return;
                }
                for _ in 0..n {
                    let _ = with_state(|s| s.tick());
                }
                let (topics, count) = with_state_ref(|s| (s.topics(), s.tick_count()));
                set_sim_topics.set(topics);
                set_sim_tick_count.set(count);
                set_deploy_status.set(format!("Batch: {} ticks done (tick {})", n, count));
                bump();
            }
        }
    };

    // -- Wire drag callbacks for Port components ----------------------------

    let on_wire_start = Callback::new(move |wd: WireDrag| {
        set_dragging_wire.set(Some(wd));
    });

    let on_wire_end = Callback::new(move |(to_block, to_port): (u32, usize)| {
        let wire = match dragging_wire.get_untracked() {
            Some(w) => w,
            None => return,
        };
        set_dragging_wire.set(None);

        if wire.from_block == to_block {
            return;
        }

        // Store edge in GraphState.
        let ch_id = with_state(|s| s.connect(wire.from_block, wire.from_port, to_block, to_port));

        if ch_id.is_some() {
            // Also update block configs with auto-topic names for codegen compat.
            let auto_topic = with_state_ref(|s| {
                s.engine()
                    .channels()
                    .iter()
                    .find(|ch| Some(ch.id) == ch_id)
                    .map(|ch| ch.topic.clone())
                    .unwrap_or_default()
            });
            update_block_config_topic(
                wire.from_block,
                wire.from_port,
                ChannelDirection::Output,
                &auto_topic,
            );
            update_block_config_topic(to_block, to_port, ChannelDirection::Input, &auto_topic);
            sync_shared();
            bump();
        }
    });

    // -- Node select callback for BlockNode components ----------------------

    let on_node_select = Callback::new(move |block_id: u32| {
        set_selected_id.set(Some(block_id));
        set_selected_channel.set(None);
    });

    // -- Edge select callback for EdgePath components -----------------------

    let on_edge_select = Callback::new(move |channel_id: u32| {
        set_selected_channel.set(Some(channel_id));
        set_selected_id.set(None);
    });

    // -- Workspace mouse handlers -------------------------------------------

    let on_workspace_mousedown = move |ev: web_sys::MouseEvent| {
        let target = match ev.target() {
            Some(t) => t,
            None => return,
        };
        let el: web_sys::Element = match target.dyn_into() {
            Ok(e) => e,
            Err(_) => return,
        };

        // Skip port elements (handled by Port component).
        let side = el.get_attribute("data-side").unwrap_or_default();
        if side == "out" || side == "in" {
            return;
        }

        // Check if clicked on a node (walk up DOM for df-node class).
        let mut current: Option<web_sys::Element> = Some(el.clone());
        while let Some(node) = current {
            if node.class_list().contains("df-node") {
                // Start node drag.
                if let Some(bid_str) = node.get_attribute("data-block-id") {
                    if let Ok(bid) = bid_str.parse::<BlockId>() {
                        let (px, py) = with_state_ref(|s| {
                            s.positions().get(&bid).copied().unwrap_or((0.0, 0.0))
                        });
                        set_dragging_node.set(Some(DraggingNode {
                            block_id: bid,
                            start_mouse_x: ev.client_x() as f64,
                            start_mouse_y: ev.client_y() as f64,
                            start_node_x: px,
                            start_node_y: py,
                            moved: false,
                        }));
                        ev.prevent_default();
                        return;
                    }
                }
            }
            current = node.parent_element();
        }

        // Pan: middle mouse button or shift+click.
        if ev.shift_key() || ev.button() == 1 {
            set_panning.set(Some(Panning {
                start_mouse_x: ev.client_x() as f64,
                start_mouse_y: ev.client_y() as f64,
                start_pan_x: pan_x.get_untracked(),
                start_pan_y: pan_y.get_untracked(),
            }));
            ev.prevent_default();
            return;
        }

        // Clicked on empty canvas -- deselect.
        set_selected_channel.set(None);
        set_selected_id.set(None);
    };

    let on_workspace_mousemove = move |ev: web_sys::MouseEvent| {
        let cx = ev.client_x() as f64;
        let cy = ev.client_y() as f64;

        // Node drag.
        if let Some(dn) = dragging_node.get_untracked() {
            let z = zoom.get_untracked();
            let dx = (cx - dn.start_mouse_x) / z;
            let dy = (cy - dn.start_mouse_y) / z;
            let dist = (dx * dx + dy * dy).sqrt();
            let moved = dn.moved || dist > DRAG_THRESHOLD;
            if moved {
                let new_x = dn.start_node_x + dx;
                let new_y = dn.start_node_y + dy;
                with_state(|s| s.set_position(dn.block_id, new_x, new_y));
                bump();
            }
            set_dragging_node.set(Some(DraggingNode { moved, ..dn }));
            ev.prevent_default();
            return;
        }

        // Pan drag.
        if let Some(pan) = panning.get_untracked() {
            let dx = cx - pan.start_mouse_x;
            let dy = cy - pan.start_mouse_y;
            set_pan_x.set(pan.start_pan_x + dx);
            set_pan_y.set(pan.start_pan_y + dy);
            ev.prevent_default();
            return;
        }

        // Wire drag: update cursor position.
        if dragging_wire.get_untracked().is_some() {
            set_dragging_wire.update(|dw| {
                if let Some(ref mut w) = dw {
                    w.cursor_x = cx;
                    w.cursor_y = cy;
                }
            });
        }
    };

    let on_workspace_mouseup = move |ev: web_sys::MouseEvent| {
        // Finish node drag.
        if let Some(dn) = dragging_node.get_untracked() {
            set_dragging_node.set(None);
            if !dn.moved {
                // No movement -- treat as a click to select.
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

        // Finish wire drag (drop on empty space cancels it; Port mouseup handles connections).
        if dragging_wire.get_untracked().is_some() {
            set_dragging_wire.set(None);
            let _ = ev; // consume event
        }
    };

    let on_workspace_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let old_zoom = zoom.get_untracked();
        let direction = if ev.delta_y() < 0.0 { 1.0 } else { -1.0 };
        let new_zoom = (old_zoom * ZOOM_FACTOR.powf(direction)).clamp(ZOOM_MIN, ZOOM_MAX);

        // Zoom toward mouse: adjust pan so the world point under cursor stays fixed.
        let old_px = pan_x.get_untracked();
        let old_py = pan_y.get_untracked();
        let cx = ev.client_x() as f64;
        let cy = ev.client_y() as f64;

        // Get workspace element bounding rect.
        if let Some(target) = ev.current_target() {
            if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                let rect = el.get_bounding_client_rect();
                let mx = cx - rect.left();
                let my = cy - rect.top();

                // World point under cursor: (mx - pan) / zoom.
                // After zoom change, we want the same world point under cursor:
                // mx - new_pan = (mx - old_pan) * new_zoom / old_zoom
                set_pan_x.set(mx - (mx - old_px) * new_zoom / old_zoom);
                set_pan_y.set(my - (my - old_py) * new_zoom / old_zoom);
            }
        }
        set_zoom.set(new_zoom);
    };

    // -- View ---------------------------------------------------------------

    view! {
        <div class="dag-editor-layout">
            // -- Left sidebar: all controls stacked -------------------------
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

            // -- Center canvas: 3 stacked layers ----------------------------
            <div
                class="df-workspace"
                tabindex="0"
                on:keydown=on_keydown
                on:mousedown=on_workspace_mousedown
                on:mousemove=on_workspace_mousemove
                on:mouseup=on_workspace_mouseup
                on:wheel=on_workspace_wheel
            >
                // Layer 1: Grid background (CSS only, no element needed beyond the parent div).

                // Layer 2: SVG edge layer.
                <svg
                    class="df-edge-layer"
                    style="position:absolute;inset:0;z-index:1;pointer-events:none;width:100%;height:100%"
                >
                    <g
                        class="dag-world"
                        transform=move || format!(
                            "translate({},{}) scale({})",
                            pan_x.get(), pan_y.get(), zoom.get()
                        )
                    >
                        // Wire drag preview path.
                        {move || {
                            let dw = dragging_wire.get();
                            dw.and_then(|w| {
                                let (sx, sy) = with_state_ref(|s| {
                                    s.positions().get(&w.from_block).copied()
                                })?;
                                let x1 = sx + NODE_WIDTH;
                                let y1 = sy + PORT_Y_START + w.from_port as f64 * PORT_SPACING;
                                // Convert cursor from client to world coords.
                                // Approximate: cursor_client - pan, then / zoom.
                                let px = pan_x.get();
                                let py = pan_y.get();
                                let z = zoom.get();
                                let x2 = (w.cursor_x - px) / z;
                                let y2 = (w.cursor_y - py) / z;
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
                        // Rendered edges from GraphState channels.
                        {move || {
                            let _rev = revision.get();

                            let edge_data: Vec<_> = with_state_ref(|s| {
                                let eng = s.engine();
                                let positions = s.positions();
                                eng.channels().iter().filter_map(|ch| {
                                    let (sx, sy) = positions.get(&ch.from_block)?;
                                    let (dx, dy) = positions.get(&ch.to_block)?;
                                    let x1 = sx + NODE_WIDTH;
                                    let y1 = sy + PORT_Y_START + ch.from_port as f64 * PORT_SPACING;
                                    let x2 = *dx;
                                    let y2 = dy + PORT_Y_START + ch.to_port as f64 * PORT_SPACING;
                                    let cpx = f64::max((x2 - x1).abs() * 0.4, 30.0);
                                    let d = format!(
                                        "M {},{} C {},{} {},{} {},{}",
                                        x1, y1,
                                        x1 + cpx, y1,
                                        x2 - cpx, y2,
                                        x2, y2
                                    );
                                    Some((ch.id, d))
                                }).collect()
                            });

                            edge_data.into_iter().map(|(ch_id, path_d)| {
                                let is_sel = Signal::derive(move || selected_channel.get() == Some(ch_id));
                                view! {
                                    <EdgePath
                                        channel_id=ch_id
                                        path_d=path_d
                                        is_selected=is_sel
                                        on_select=on_edge_select
                                    />
                                }
                            }).collect_view()
                        }}
                    </g>
                </svg>

                // Layer 3: HTML node layer.
                <div
                    class="df-node-layer"
                    style="position:absolute;inset:0;z-index:2"
                >
                    <div
                        class="dag-world"
                        style=move || format!(
                            "transform:translate({}px,{}px) scale({});transform-origin:0 0",
                            pan_x.get(), pan_y.get(), zoom.get()
                        )
                    >
                        {move || {
                            let _rev = revision.get();

                            // Extract block data from GraphState.
                            let blocks_data: Vec<_> = with_state_ref(|s| {
                                let eng = s.engine();
                                let positions = s.positions();
                                eng.blocks().iter().map(|b| {
                                    let block = b.reconstruct();
                                    let name = block.as_ref()
                                        .map(|bl| bl.display_name().to_string())
                                        .unwrap_or_else(|| b.block_type.clone());
                                    let bt = b.block_type.clone();
                                    let channels = block.as_ref()
                                        .map(|bl| bl.declared_channels())
                                        .unwrap_or_default();
                                    let (x, y) = positions.get(&b.id).copied().unwrap_or((30.0, 30.0));
                                    (b.id, name, bt, channels, x, y)
                                }).collect()
                            });

                            blocks_data.into_iter().map(|(id, name, bt, channels, x, y)| {
                                let inputs: Vec<PortDef> = channels.iter()
                                    .filter(|c| c.direction == ChannelDirection::Input)
                                    .map(|c| PortDef { name: c.name.clone(), side: "input" })
                                    .collect();
                                let outputs: Vec<PortDef> = channels.iter()
                                    .filter(|c| c.direction == ChannelDirection::Output)
                                    .map(|c| PortDef { name: c.name.clone(), side: "output" })
                                    .collect();
                                let is_selected = Signal::derive(move || selected_id.get() == Some(id));

                                view! {
                                    <BlockNode
                                        block_id=id
                                        name=name
                                        block_type=bt
                                        x=x
                                        y=y
                                        selected=is_selected
                                        inputs=inputs
                                        outputs=outputs
                                        on_select=on_node_select
                                        wire_drag=dragging_wire
                                        on_wire_start=on_wire_start
                                        on_wire_end=on_wire_end
                                    />
                                }
                            }).collect_view()
                        }}
                    </div>
                </div>
            </div>

            // -- Right pane (placeholder for Plot/Pins/I2C) -----------------
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
    with_state(|s| {
        let eng = s.engine();
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
                s.update_config(block_id, key, serde_json::Value::String(topic.to_string()));
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

/// Extract the value string from an input/select change event.
fn event_target_value(ev: &leptos::ev::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .or_else(|| {
            ev.target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|el| el.value())
        })
        .unwrap_or_default()
}
