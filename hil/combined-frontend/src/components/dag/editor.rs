//! DAG editor panel: palette, canvas, config, deploy.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::lower;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;

use crate::types::BlockSet;

use super::config_panel::ConfigPanel;
use super::monitor::MonitorPanel;
use super::palette::BlockPalette;

/// An edge connecting an output port on one block to an input port on another.
///
/// Edges are auto-detected by matching `declared_channels()` topic names:
/// a block with an Output channel named "foo" connects to any block with
/// an Input channel named "foo".
#[derive(Clone)]
struct Edge {
    /// Block id of the source (output) block.
    from_block: usize,
    /// Index of the output port on the source block (0-based among outputs).
    from_port: usize,
    /// Block id of the destination (input) block.
    to_block: usize,
    /// Index of the input port on the destination block (0-based among inputs).
    to_port: usize,
}

/// State of an in-progress wire drag from an output port.
#[derive(Clone, Copy)]
struct DraggingWire {
    /// Block id of the source block.
    from_block: usize,
    /// Output port index on the source block.
    from_port: usize,
    /// Current mouse X in SVG coordinates.
    mouse_x: f64,
    /// Current mouse Y in SVG coordinates.
    mouse_y: f64,
}

/// Instance of a placed block on the canvas.
///
/// Stores block type + config as serializable data (Send+Sync safe).
/// The trait object is reconstructed from the registry when needed.
#[derive(Clone)]
struct PlacedBlock {
    id: usize,
    block_type: String,
    config: serde_json::Value,
    x: f64,
    y: f64,
}

impl PlacedBlock {
    /// Reconstruct the ConfigurableBlock trait object from the registry.
    fn reconstruct(&self) -> Option<Box<dyn lower::ConfigurableBlock>> {
        let mut block = registry::create_block(&self.block_type)?;
        block.apply_config(&self.config);
        Some(block)
    }
}

#[component]
pub fn DagEditorPanel() -> impl IntoView {
    // Block instances on the canvas
    let (blocks, set_blocks) = signal(Vec::<PlacedBlock>::new());
    let (next_id, set_next_id) = signal(1_usize);

    // Shared block-set context: push (block_type, config) pairs to deploy panel.
    let set_shared_blocks = use_context::<WriteSignal<BlockSet>>();

    // Sync local blocks → shared context whenever blocks change.
    let sync_shared = move |blks: &[PlacedBlock]| {
        if let Some(setter) = set_shared_blocks {
            let block_set: BlockSet = blks
                .iter()
                .map(|pb| (pb.block_type.clone(), pb.config.clone()))
                .collect();
            setter.set(block_set);
        }
    };

    // Selected block
    let (selected_id, set_selected_id) = signal(None::<usize>);

    // Wire drag state: Some while dragging from an output port.
    let (dragging_wire, set_dragging_wire) = signal(None::<DraggingWire>);

    // Config signals derived from selection
    let selected_block_type = Signal::derive(move || {
        let sel = selected_id.get()?;
        let blks = blocks.get();
        let pb = blks.iter().find(|b| b.id == sel)?;
        let block = pb.reconstruct()?;
        Some(block.display_name().to_string())
    });

    let config_fields = Signal::derive(move || {
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let blks = blocks.get();
        match blks.iter().find(|b| b.id == sel) {
            Some(pb) => pb.reconstruct().map(|b| b.config_schema()).unwrap_or_default(),
            None => Vec::new(),
        }
    });

    let config_values = Signal::derive(move || {
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return serde_json::Value::Object(Default::default()),
        };
        let blks = blocks.get();
        match blks.iter().find(|b| b.id == sel) {
            Some(pb) => pb.config.clone(),
            None => serde_json::Value::Object(Default::default()),
        }
    });

    let channels_text = Signal::derive(move || {
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return String::new(),
        };
        let blks = blocks.get();
        match blks.iter().find(|b| b.id == sel) {
            Some(pb) => match pb.reconstruct() {
                Some(block) => {
                    let chs = block.declared_channels();
                    chs.iter()
                        .map(|ch| {
                            let dir = match ch.direction {
                                ChannelDirection::Input => "IN",
                                ChannelDirection::Output => "OUT",
                            };
                            let kind = format!("{:?}", ch.kind).to_lowercase();
                            format!("{} {} [{}]", dir, ch.name, kind)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                None => String::new(),
            },
            None => String::new(),
        }
    });

    let il_text = Signal::derive(move || {
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return String::new(),
        };
        let blks = blocks.get();
        match blks.iter().find(|b| b.id == sel) {
            Some(pb) => match pb.reconstruct() {
                Some(block) => lower::lower_to_il_text(block.as_ref())
                    .unwrap_or_else(|e| format!("Error: {}", e)),
                None => String::new(),
            },
            None => String::new(),
        }
    });

    // Auto-detect edges by matching output topic names to input topic names.
    let edges = Signal::derive(move || {
        let blks = blocks.get();
        // Collect (block_id, port_index, topic_name) for every output channel.
        let mut outputs: Vec<(usize, usize, String)> = Vec::new();
        // Collect (block_id, port_index, topic_name) for every input channel.
        let mut inputs: Vec<(usize, usize, String)> = Vec::new();

        for pb in blks.iter() {
            if let Some(block) = pb.reconstruct() {
                let channels = block.declared_channels();
                let mut in_idx = 0_usize;
                let mut out_idx = 0_usize;
                for ch in &channels {
                    match ch.direction {
                        ChannelDirection::Output => {
                            outputs.push((pb.id, out_idx, ch.name.clone()));
                            out_idx += 1;
                        }
                        ChannelDirection::Input => {
                            inputs.push((pb.id, in_idx, ch.name.clone()));
                            in_idx += 1;
                        }
                    }
                }
            }
        }

        let mut result = Vec::<Edge>::new();
        for (out_id, out_port, ref topic) in &outputs {
            for (in_id, in_port, ref in_topic) in &inputs {
                if topic == in_topic && out_id != in_id {
                    result.push(Edge {
                        from_block: *out_id,
                        from_port: *out_port,
                        to_block: *in_id,
                        to_port: *in_port,
                    });
                }
            }
        }
        result
    });

    // Deploy status
    let (deploy_status, set_deploy_status) = signal(String::new());

    // Add block from palette
    let on_add_block = Callback::new(move |block_type: String| {
        if let Some(block) = registry::create_block(&block_type) {
            let id = next_id.get();
            set_next_id.set(id + 1);
            let count = blocks.get().len();
            let x = 30.0 + (count % 3) as f64 * 220.0;
            let y = 30.0 + (count / 3) as f64 * 120.0;
            let config = block.config_json();
            set_blocks.update(|v| {
                v.push(PlacedBlock {
                    id,
                    block_type: block_type.clone(),
                    config,
                    x,
                    y,
                });
            });
            sync_shared(&blocks.get());
            set_selected_id.set(Some(id));
        }
    });

    // Config change handler
    let on_config_change = Callback::new(move |(key, value): (String, serde_json::Value)| {
        let sel = match selected_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        set_blocks.update(|blks| {
            if let Some(pb) = blks.iter_mut().find(|b| b.id == sel) {
                // Update the stored config JSON directly
                if let serde_json::Value::Object(ref mut map) = pb.config {
                    map.insert(key, value);
                }
            }
        });
        sync_shared(&blocks.get());
    });

    // Delete selected block
    let on_delete = move |_| {
        if let Some(sel) = selected_id.get_untracked() {
            set_blocks.update(|v| v.retain(|b| b.id != sel));
            sync_shared(&blocks.get());
            set_selected_id.set(None);
        }
    };

    // Simulation state (persists pubsub values across ticks)
    let (sim_topics, set_sim_topics) = signal(std::collections::BTreeMap::<String, f64>::new());
    let (sim_tick_count, set_sim_tick_count) = signal(0_u64);
    let (sim_running, set_sim_running) = signal(false);

    // Helper: build merged DAG from current blocks
    let build_dag = move || -> Result<dag_core::op::Dag, String> {
        let blks = blocks.get();
        if blks.is_empty() {
            return Err("No blocks".into());
        }
        let mut combined = dag_core::op::Dag::new();
        for pb in blks.iter() {
            let block = pb.reconstruct()
                .ok_or_else(|| format!("Unknown block type: {}", pb.block_type))?;
            let result = block.lower()
                .map_err(|e| format!("Lower error: {:?}", e))?;
            let offset = combined.len() as u16;
            for op in result.dag.nodes() {
                let adjusted = offset_op(op, offset);
                combined.add_op(adjusted)
                    .map_err(|e| format!("Merge error: {:?}", e))?;
            }
        }
        Ok(combined)
    };

    // SimState is stored in a thread_local RefCell (not Send, can't be in signal)
    use std::cell::RefCell;
    thread_local! {
        static SIM: RefCell<Option<dag_core::eval::SimState>> = const { RefCell::new(None) };
    }

    // Step: single tick
    let on_step = move |_| {
        let dag = match build_dag() {
            Ok(d) => d,
            Err(e) => { set_deploy_status.set(e); return; }
        };
        SIM.with(|cell| {
            let mut sim = cell.borrow_mut();
            if sim.is_none() || sim.as_ref().is_some_and(|s| s.tick_count() == 0) {
                *sim = Some(dag_core::eval::SimState::new(dag.len()));
            }
            if let Some(ref mut s) = *sim {
                s.tick(&dag);
                set_sim_topics.set(s.topics().clone());
                set_sim_tick_count.set(s.tick_count());
                set_deploy_status.set(format!("Tick {} ({} topics)", s.tick_count(), s.topics().len()));
            }
        });
    };

    // Reset
    let on_reset = move |_| {
        SIM.with(|cell| {
            if let Some(ref mut s) = *cell.borrow_mut() {
                s.reset();
                set_sim_topics.set(std::collections::BTreeMap::new());
                set_sim_tick_count.set(0);
            }
        });
        set_sim_running.set(false);
        set_deploy_status.set("Reset".into());
    };

    // Play/Pause toggle
    let on_play_pause = move |_| {
        let running = sim_running.get();
        if running {
            set_sim_running.set(false);
            set_deploy_status.set("Paused".into());
        } else {
            // Rebuild DAG and start ticking
            let dag = match build_dag() {
                Ok(d) => d,
                Err(e) => { set_deploy_status.set(e); return; }
            };
            SIM.with(|cell| {
                let mut sim = cell.borrow_mut();
                if sim.is_none() {
                    *sim = Some(dag_core::eval::SimState::new(dag.len()));
                }
            });
            set_sim_running.set(true);
            set_deploy_status.set("Running...".into());

            // Start tick loop via gloo_timers (100ms = 10Hz)
            let set_topics = set_sim_topics;
            let set_tick = set_sim_tick_count;
            gloo_timers::callback::Interval::new(100, move || {
                if !sim_running.get_untracked() {
                    return; // paused — interval keeps firing but we skip
                }
                let dag = match build_dag() {
                    Ok(d) => d,
                    Err(_) => return,
                };
                SIM.with(|cell| {
                    if let Some(ref mut s) = *cell.borrow_mut() {
                        s.tick(&dag);
                        set_topics.set(s.topics().clone());
                        set_tick.set(s.tick_count());
                    }
                });
            }).forget();
        }
    };

    // Deploy: lower all blocks, merge DAGs, CBOR encode, POST to MCU
    let on_deploy = move |_| {
        let blks = blocks.get();
        if blks.is_empty() {
            set_deploy_status.set("No blocks to deploy".into());
            return;
        }

        // Merge all blocks into a single DAG
        let mut combined = dag_core::op::Dag::new();
        for pb in blks.iter() {
            let block = match pb.reconstruct() {
                Some(b) => b,
                None => {
                    set_deploy_status.set(format!("Unknown block type: {}", pb.block_type));
                    return;
                }
            };
            let result = match block.lower() {
                Ok(r) => r,
                Err(e) => {
                    set_deploy_status.set(format!("Lower error: {:?}", e));
                    return;
                }
            };
            // Append ops from this block's DAG into combined, adjusting node refs
            let offset = combined.len() as u16;
            for op in result.dag.nodes() {
                let adjusted = offset_op(op, offset);
                if let Err(e) = combined.add_op(adjusted) {
                    set_deploy_status.set(format!("Merge error: {:?}", e));
                    return;
                }
            }
        }

        let cbor_bytes = dag_core::cbor::encode_dag(&combined);
        let node_count = combined.len();

        // POST to MCU via fetch API
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

    // Tick: POST /api/tick to evaluate the deployed DAG once
    let _on_tick = move |_: web_sys::MouseEvent| {
        let status_setter = set_deploy_status;
        wasm_bindgen_futures::spawn_local(async move {
            match tick_mcu().await {
                Ok(msg) => status_setter.set(format!("Tick: {}", msg)),
                Err(e) => status_setter.set(format!("Tick failed: {}", e)),
            }
        });
    };

    view! {
        <h2 class="section-title">"DAG Editor"</h2>
        <div class="dag-editor-layout">
            // Left: palette
            <BlockPalette on_add=on_add_block />

            // Center: canvas
            <div class="dag-canvas-container">
                <div class="dag-toolbar">
                    <button
                        class=move || if sim_running.get() { "btn btn-danger" } else { "btn btn-primary" }
                        on:click=on_play_pause
                    >
                        {move || if sim_running.get() { "Pause" } else { "Play" }}
                    </button>
                    <button class="btn btn-secondary" on:click=on_step>"Step"</button>
                    <button class="btn btn-secondary" on:click=on_reset>"Reset"</button>
                    <button class="btn btn-secondary" on:click=on_deploy>"Deploy"</button>
                    <button class="btn btn-danger" on:click=on_delete>"Delete"</button>
                    <span class="dag-status">{move || deploy_status.get()}</span>
                </div>
                <svg
                    class="dag-canvas"
                    viewBox="0 0 700 400"
                    on:mousedown=move |ev: web_sys::MouseEvent| {
                        // Start wire drag if mousedown is on an output port circle.
                        let target = match ev.target() {
                            Some(t) => t,
                            None => return,
                        };
                        let el: web_sys::Element = match target.dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let side = el.get_attribute("data-side").unwrap_or_default();
                        if side != "out" {
                            return;
                        }
                        let block_id: usize = match el.get_attribute("data-block-id")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };
                        let port_idx: usize = match el.get_attribute("data-port-idx")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };
                        // Compute SVG coordinates from client position
                        let svg_el = match el.closest("svg") {
                            Ok(Some(s)) => s,
                            _ => return,
                        };
                        let (mx, my) = client_to_svg(&svg_el, ev.client_x() as f64, ev.client_y() as f64);
                        set_dragging_wire.set(Some(DraggingWire {
                            from_block: block_id,
                            from_port: port_idx,
                            mouse_x: mx,
                            mouse_y: my,
                        }));
                        ev.prevent_default();
                    }
                    on:mousemove=move |ev: web_sys::MouseEvent| {
                        // Update drag line endpoint while dragging.
                        if dragging_wire.get_untracked().is_none() {
                            return;
                        }
                        let target = match ev.current_target() {
                            Some(t) => t,
                            None => return,
                        };
                        let svg_el: web_sys::Element = match target.dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let (mx, my) = client_to_svg(&svg_el, ev.client_x() as f64, ev.client_y() as f64);
                        set_dragging_wire.update(|dw| {
                            if let Some(ref mut w) = dw {
                                w.mouse_x = mx;
                                w.mouse_y = my;
                            }
                        });
                    }
                    on:mouseup=move |ev: web_sys::MouseEvent| {
                        // Complete or cancel wire drag.
                        let wire = match dragging_wire.get_untracked() {
                            Some(w) => w,
                            None => return,
                        };
                        // Clear drag state first.
                        set_dragging_wire.set(None);

                        // Check if mouseup target is an input port.
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
                            return; // Dropped on empty canvas — cancel.
                        }
                        let to_block: usize = match el.get_attribute("data-block-id")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };
                        let to_port: usize = match el.get_attribute("data-port-idx")
                            .and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => return,
                        };

                        // Do not wire a block to itself.
                        if wire.from_block == to_block {
                            return;
                        }

                        // Generate auto-topic name.
                        let auto_topic = format!("wire_{}_{}", wire.from_block, wire.from_port);

                        // Update configs for both source and target blocks.
                        set_blocks.update(|blks| {
                            // --- Source block: set the output channel's config key ---
                            if let Some(src_pb) = blks.iter().find(|b| b.id == wire.from_block) {
                                if let Some(src_block) = src_pb.reconstruct() {
                                    let channels = src_block.declared_channels();
                                    let out_channels: Vec<_> = channels.iter()
                                        .filter(|c| c.direction == ChannelDirection::Output)
                                        .collect();
                                    if let Some(out_ch) = out_channels.get(wire.from_port) {
                                        if let Some(key) = find_config_key_for_channel(&src_pb.config, &out_ch.name) {
                                            // Now mutably borrow to update
                                            if let Some(src_mut) = blks.iter_mut().find(|b| b.id == wire.from_block) {
                                                if let serde_json::Value::Object(ref mut map) = src_mut.config {
                                                    map.insert(key, serde_json::Value::String(auto_topic.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // --- Target block: set the input channel's config key ---
                            if let Some(dst_pb) = blks.iter().find(|b| b.id == to_block) {
                                if let Some(dst_block) = dst_pb.reconstruct() {
                                    let channels = dst_block.declared_channels();
                                    let in_channels: Vec<_> = channels.iter()
                                        .filter(|c| c.direction == ChannelDirection::Input)
                                        .collect();
                                    if let Some(in_ch) = in_channels.get(to_port) {
                                        if let Some(key) = find_config_key_for_channel(&dst_pb.config, &in_ch.name) {
                                            if let Some(dst_mut) = blks.iter_mut().find(|b| b.id == to_block) {
                                                if let serde_json::Value::Object(ref mut map) = dst_mut.config {
                                                    map.insert(key, serde_json::Value::String(auto_topic.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });
                        sync_shared(&blocks.get());
                    }
                >
                    // Dashed drag line (rendered when dragging a wire)
                    {move || {
                        let dw = dragging_wire.get();
                        let blks = blocks.get();
                        dw.and_then(|w| {
                            let src = blks.iter().find(|b| b.id == w.from_block)?;
                            let x1 = src.x + 190.0;
                            let y1 = src.y + 46.0 + w.from_port as f64 * 16.0;
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
                        let blks = blocks.get();
                        let edge_list = edges.get();

                        // Edge paths (rendered first so they appear behind nodes)
                        let edge_views = edge_list.iter().filter_map(|edge| {
                            let src = blks.iter().find(|b| b.id == edge.from_block)?;
                            let dst = blks.iter().find(|b| b.id == edge.to_block)?;
                            // Output port: right side of src block
                            let x1 = src.x + 190.0;
                            let y1 = src.y + 46.0 + edge.from_port as f64 * 16.0;
                            // Input port: left side of dst block
                            let x2 = dst.x;
                            let y2 = dst.y + 46.0 + edge.to_port as f64 * 16.0;
                            // Cubic bezier control point x-offset
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
                                    stroke="#6b7280"
                                    stroke-width="2"
                                    class="dag-edge"
                                />
                            })
                        }).collect_view();

                        // Block nodes
                        let node_views = blks.iter().map(|pb| {
                            let id = pb.id;
                            let x = pb.x;
                            let y = pb.y;
                            let block = pb.reconstruct();
                            let name = block.as_ref().map(|b| b.display_name().to_string()).unwrap_or_else(|| pb.block_type.clone());
                            let bt = pb.block_type.clone();
                            let is_selected = move || selected_id.get() == Some(id);
                            let channels = block.as_ref().map(|b| b.declared_channels()).unwrap_or_default();
                            let in_count = channels.iter()
                                .filter(|c| c.direction == ChannelDirection::Input).count();
                            let out_count = channels.iter()
                                .filter(|c| c.direction == ChannelDirection::Output).count();
                            let height = 50.0 + (in_count.max(out_count) as f64) * 16.0;

                            view! {
                                <g
                                    class=move || if is_selected() { "dag-node selected" } else { "dag-node" }
                                    transform=format!("translate({},{})", x, y)
                                    on:click=move |_| set_selected_id.set(Some(id))
                                >
                                    <rect
                                        width="190" height=height rx="6" ry="6"
                                        class="dag-node-rect"
                                    />
                                    <text x="95" y="18" class="dag-node-title">{name}</text>
                                    <text x="95" y="32" class="dag-node-type">{bt}</text>
                                    // Input ports
                                    {channels.iter().filter(|c| c.direction == ChannelDirection::Input).enumerate().map(|(i, ch)| {
                                        let py = 46.0 + i as f64 * 16.0;
                                        let label = ch.name.clone();
                                        let bid = id.to_string();
                                        let pidx = i.to_string();
                                        view! {
                                            <circle
                                                cx="0" cy=py r="4"
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
                                        let py = 46.0 + i as f64 * 16.0;
                                        let label = ch.name.clone();
                                        let bid = id.to_string();
                                        let pidx = i.to_string();
                                        view! {
                                            <circle
                                                cx="190" cy=py r="4"
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
                </svg>
            </div>

            // Bottom: live monitor
            <MonitorPanel topics=sim_topics tick_count=sim_tick_count />

            // Right: config panel
            <ConfigPanel
                block_type=selected_block_type
                config_fields=config_fields
                config_values=config_values
                on_change=on_config_change
                channels_text=channels_text
                il_text=il_text
            />
        </div>
    }
}

/// Find the config key whose current value matches `channel_name`.
///
/// Blocks store channel topic names as string values in their config JSON.
/// For example, `{"input_topic": "add/a", "output_topic": "add/out"}`.
/// Given channel_name="add/a", this returns Some("input_topic").
fn find_config_key_for_channel(
    config: &serde_json::Value,
    channel_name: &str,
) -> Option<String> {
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

/// Convert mouse client coordinates to SVG user-space coordinates.
///
/// Uses the SVG element's bounding rect and viewBox to map screen pixels
/// to the SVG coordinate system.
fn client_to_svg(svg: &web_sys::Element, client_x: f64, client_y: f64) -> (f64, f64) {
    let rect = svg.get_bounding_client_rect();
    let rect_w = rect.width();
    let rect_h = rect.height();
    // viewBox is "0 0 700 400" — extract via attribute or use defaults
    let (vb_w, vb_h) = svg
        .get_attribute("viewBox")
        .and_then(|vb| {
            let parts: Vec<f64> = vb.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() == 4 {
                Some((parts[2], parts[3]))
            } else {
                None
            }
        })
        .unwrap_or((700.0, 400.0));

    let scale_x = vb_w / rect_w;
    let scale_y = vb_h / rect_h;
    let x = (client_x - rect.left()) * scale_x;
    let y = (client_y - rect.top()) * scale_y;
    (x, y)
}

/// Offset all NodeId references in an Op by a given amount.
fn offset_op(op: &dag_core::op::Op, offset: u16) -> dag_core::op::Op {
    use dag_core::op::Op;
    match op {
        Op::Const(v) => Op::Const(*v),
        Op::Input(name) => Op::Input(name.clone()),
        Op::Output(name, src) => Op::Output(name.clone(), src + offset),
        Op::Add(a, b) => Op::Add(a + offset, b + offset),
        Op::Mul(a, b) => Op::Mul(a + offset, b + offset),
        Op::Sub(a, b) => Op::Sub(a + offset, b + offset),
        Op::Div(a, b) => Op::Div(a + offset, b + offset),
        Op::Pow(a, b) => Op::Pow(a + offset, b + offset),
        Op::Neg(a) => Op::Neg(a + offset),
        Op::Relu(a) => Op::Relu(a + offset),
        Op::Subscribe(topic) => Op::Subscribe(topic.clone()),
        Op::Publish(topic, src) => Op::Publish(topic.clone(), src + offset),
    }
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
