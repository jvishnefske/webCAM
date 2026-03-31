//! DAG editor panel: palette, canvas, config, deploy.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::lower;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;

use super::palette::BlockPalette;
use super::config_panel::ConfigPanel;

/// Instance of a placed block on the canvas.
#[derive(Clone)]
struct PlacedBlock {
    id: usize,
    block: std::rc::Rc<std::cell::RefCell<Box<dyn lower::ConfigurableBlock>>>,
    x: f64,
    y: f64,
}

// SAFETY: WASM is single-threaded.
unsafe impl Send for PlacedBlock {}
unsafe impl Sync for PlacedBlock {}

#[component]
pub fn DagEditorPanel() -> impl IntoView {
    // Block instances on the canvas
    let (blocks, set_blocks) = signal(Vec::<PlacedBlock>::new());
    let (next_id, set_next_id) = signal(1_usize);

    // Selected block
    let (selected_id, set_selected_id) = signal(None::<usize>);

    // Config signals derived from selection
    let selected_block_type = Signal::derive(move || {
        let sel = selected_id.get()?;
        let blks = blocks.get();
        let pb = blks.iter().find(|b| b.id == sel)?;
        let name = pb.block.borrow().display_name().to_string();
        Some(name)
    });

    let config_fields = Signal::derive(move || {
        let sel = match selected_id.get() {
            Some(s) => s,
            None => return Vec::new(),
        };
        let blks = blocks.get();
        match blks.iter().find(|b| b.id == sel) {
            Some(pb) => pb.block.borrow().config_schema(),
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
            Some(pb) => pb.block.borrow().config_json(),
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
            Some(pb) => {
                let chs = pb.block.borrow().declared_channels();
                chs.iter().map(|ch| {
                    let dir = match ch.direction {
                        ChannelDirection::Input => "IN",
                        ChannelDirection::Output => "OUT",
                    };
                    let kind = format!("{:?}", ch.kind).to_lowercase();
                    format!("{} {} [{}]", dir, ch.name, kind)
                }).collect::<Vec<_>>().join("\n")
            }
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
            Some(pb) => {
                let borrow = pb.block.borrow();
                lower::lower_to_il_text(borrow.as_ref())
                    .unwrap_or_else(|e| format!("Error: {}", e))
            }
            None => String::new(),
        }
    });

    // Deploy status
    let (deploy_status, set_deploy_status) = signal(String::new());

    // Add block from palette
    let on_add_block = Callback::new(move |block_type: String| {
        if let Some(block) = registry::create_block(&block_type) {
            let id = next_id.get();
            set_next_id.set(id + 1);
            // Place blocks in a grid pattern
            let count = blocks.get().len();
            let x = 30.0 + (count % 3) as f64 * 220.0;
            let y = 30.0 + (count / 3) as f64 * 120.0;
            set_blocks.update(|v| {
                v.push(PlacedBlock {
                    id,
                    block: std::rc::Rc::new(std::cell::RefCell::new(block)),
                    x,
                    y,
                });
            });
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
            if let Some(pb) = blks.iter().find(|b| b.id == sel) {
                let mut partial = serde_json::Map::new();
                partial.insert(key, value);
                pb.block.borrow_mut().apply_config(
                    &serde_json::Value::Object(partial),
                );
            }
        });
    });

    // Delete selected block
    let on_delete = move |_| {
        if let Some(sel) = selected_id.get_untracked() {
            set_blocks.update(|v| v.retain(|b| b.id != sel));
            set_selected_id.set(None);
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
            let borrow = pb.block.borrow();
            let result = match borrow.as_ref().lower() {
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
        set_deploy_status.set(format!("Deploying {} nodes ({} bytes)...", node_count, cbor_bytes.len()));

        let status_setter = set_deploy_status;
        wasm_bindgen_futures::spawn_local(async move {
            match deploy_to_mcu(&cbor_bytes).await {
                Ok(msg) => status_setter.set(format!("Deployed: {}", msg)),
                Err(e) => status_setter.set(format!("Deploy failed: {}", e)),
            }
        });
    };

    // Tick: POST /api/tick to evaluate the deployed DAG once
    let on_tick = move |_| {
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
                    <button class="btn btn-primary" on:click=on_deploy>"Deploy to MCU"</button>
                    <button class="btn btn-secondary" on:click=on_tick>"Tick"</button>
                    <button class="btn btn-danger" on:click=on_delete>"Delete Block"</button>
                    <span class="dag-status">{move || deploy_status.get()}</span>
                </div>
                <svg class="dag-canvas" viewBox="0 0 700 400">
                    {move || {
                        blocks.get().iter().map(|pb| {
                            let id = pb.id;
                            let x = pb.x;
                            let y = pb.y;
                            let name = pb.block.borrow().display_name().to_string();
                            let bt = pb.block.borrow().block_type().to_string();
                            let is_selected = move || selected_id.get() == Some(id);
                            let channels = pb.block.borrow().declared_channels();
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
                                        view! {
                                            <circle cx="0" cy=py r="4" class="dag-port dag-port-in" />
                                            <text x="8" y=py + 4.0 class="dag-port-label">{label}</text>
                                        }
                                    }).collect_view()}
                                    // Output ports
                                    {channels.iter().filter(|c| c.direction == ChannelDirection::Output).enumerate().map(|(i, ch)| {
                                        let py = 46.0 + i as f64 * 16.0;
                                        let label = ch.name.clone();
                                        view! {
                                            <circle cx="190" cy=py r="4" class="dag-port dag-port-out" />
                                            <text x="182" y=py + 4.0 class="dag-port-label dag-port-label-right">{label}</text>
                                        }
                                    }).collect_view()}
                                </g>
                            }
                        }).collect_view()
                    }}
                </svg>
            </div>

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
    headers.set("Content-Type", "application/cbor").map_err(|e| format!("{:?}", e))?;
    opts.set_headers(&headers);

    let url = "http://169.254.1.61:8080/api/dag";
    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("{:?}", e))?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;

    let resp: web_sys::Response = resp_value.dyn_into().map_err(|_| "not a Response".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|e| format!("{:?}", e))?
    ).await.map_err(|e| format!("{:?}", e))?;

    Ok(text.as_string().unwrap_or_default())
}

/// POST /api/tick to evaluate the deployed DAG.
async fn tick_mcu() -> Result<String, String> {
    let window = web_sys::window().ok_or("no window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");

    let url = "http://169.254.1.61:8080/api/tick";
    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("{:?}", e))?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;

    let resp: web_sys::Response = resp_value.dyn_into().map_err(|_| "not a Response".to_string())?;
    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|e| format!("{:?}", e))?
    ).await.map_err(|e| format!("{:?}", e))?;

    Ok(text.as_string().unwrap_or_default())
}
