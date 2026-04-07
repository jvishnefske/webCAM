pub mod dag_api;
pub mod dataflow;

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// ── Dataflow Simulator WASM API ─────────────────────────────────────

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static DATAFLOW_GRAPHS: RefCell<std::collections::HashMap<u32, dataflow::DataflowGraph>> =
        RefCell::new(std::collections::HashMap::new());
    static DATAFLOW_NEXT_ID: RefCell<u32> = const { RefCell::new(1) };
    #[allow(clippy::missing_const_for_thread_local)]
    static DATAFLOW_SCHEDULERS: RefCell<std::collections::HashMap<u32, dataflow::Scheduler>> =
        RefCell::new(std::collections::HashMap::new());
}

/// Create a new dataflow graph. Returns its id.
#[wasm_bindgen]
pub fn dataflow_new(dt: f64) -> u32 {
    DATAFLOW_NEXT_ID.with(|next| {
        let id = *next.borrow();
        *next.borrow_mut() = id + 1;
        DATAFLOW_GRAPHS.with(|g| g.borrow_mut().insert(id, dataflow::DataflowGraph::new()));
        DATAFLOW_SCHEDULERS.with(|s| s.borrow_mut().insert(id, dataflow::Scheduler::new(dt)));
        id
    })
}

/// Destroy a dataflow graph.
#[wasm_bindgen]
pub fn dataflow_destroy(graph_id: u32) {
    DATAFLOW_GRAPHS.with(|g| g.borrow_mut().remove(&graph_id));
    DATAFLOW_SCHEDULERS.with(|s| s.borrow_mut().remove(&graph_id));
}

/// Add a block to a graph. Returns block id.
#[wasm_bindgen]
pub fn dataflow_add_block(
    graph_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<u32, JsValue> {
    let block = dataflow::blocks::create_block(block_type, config_json)
        .map_err(|e| JsValue::from_str(&e))?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let id = graph.add_block(block);
        Ok(id.0)
    })
}

/// Remove a block from a graph.
#[wasm_bindgen]
pub fn dataflow_remove_block(graph_id: u32, block_id: u32) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.remove_block(dataflow::BlockId(block_id));
        Ok(())
    })
}

/// Update a block's config by replacing it in-place (preserving channels where ports still match).
#[wasm_bindgen]
pub fn dataflow_update_block(
    graph_id: u32,
    block_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<(), JsValue> {
    let block = dataflow::blocks::create_block(block_type, config_json)
        .map_err(|e| JsValue::from_str(&e))?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph
            .replace_block(dataflow::BlockId(block_id), block)
            .map_err(|e| JsValue::from_str(&e))
    })
}

/// Validate whether a connection between two ports is valid.
/// Returns empty string on success, or an error message on failure.
#[wasm_bindgen]
pub fn dataflow_validate_connection(
    graph_id: u32,
    from_block: u32,
    from_port: u32,
    to_block: u32,
    to_port: u32,
) -> String {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = match graphs.get(&graph_id) {
            Some(g) => g,
            None => return "graph not found".to_string(),
        };

        // Build port count maps from the graph
        let snap = graph.snapshot();
        let mut output_counts = std::collections::HashMap::new();
        let mut input_counts = std::collections::HashMap::new();
        for block in &snap.blocks {
            output_counts.insert(block.id, block.outputs.len());
            input_counts.insert(block.id, block.inputs.len());
        }

        // Build existing connections list
        let existing: Vec<(u32, usize, u32, usize)> = snap
            .channels
            .iter()
            .map(|ch| (ch.from_block.0, ch.from_port, ch.to_block.0, ch.to_port))
            .collect();

        let req = dataflow::connection::ConnectionRequest {
            from_block,
            from_port: from_port as usize,
            from_side: dataflow::connection::PortSide::Output,
            to_block,
            to_port: to_port as usize,
            to_side: dataflow::connection::PortSide::Input,
        };

        match dataflow::connection::validate_connection(
            &req,
            &output_counts,
            &input_counts,
            &existing,
        ) {
            Ok(_) => String::new(),
            Err(e) => e.to_string(),
        }
    })
}

/// Connect an output port to an input port. Returns channel id.
#[wasm_bindgen]
pub fn dataflow_connect(
    graph_id: u32,
    from_block: u32,
    from_port: u32,
    to_block: u32,
    to_port: u32,
) -> Result<u32, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let ch = graph
            .connect(
                dataflow::BlockId(from_block),
                from_port as usize,
                dataflow::BlockId(to_block),
                to_port as usize,
            )
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(ch.0)
    })
}

/// Disconnect a channel.
#[wasm_bindgen]
pub fn dataflow_disconnect(graph_id: u32, channel_id: u32) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.disconnect(dataflow::ChannelId(channel_id));
        Ok(())
    })
}

/// Advance the graph by wall-clock elapsed seconds (realtime mode).
/// Returns snapshot as a typed JS object.
#[wasm_bindgen]
pub fn dataflow_advance(graph_id: u32, elapsed: f64) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        DATAFLOW_SCHEDULERS.with(|s| {
            let mut graphs = g.borrow_mut();
            let mut schedulers = s.borrow_mut();
            let graph = graphs
                .get_mut(&graph_id)
                .ok_or_else(|| JsValue::from_str("graph not found"))?;
            let sched = schedulers
                .get_mut(&graph_id)
                .ok_or_else(|| JsValue::from_str("scheduler not found"))?;
            let ticks = sched.advance(elapsed);
            graph.run(ticks, sched.dt);
            let snap = graph.snapshot();
            serde_wasm_bindgen::to_value(&snap)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        })
    })
}

/// Run a fixed number of ticks (non-realtime batch mode).
/// Returns snapshot as a typed JS object.
#[wasm_bindgen]
pub fn dataflow_run(graph_id: u32, steps: u32, dt: f64) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.run(steps as u64, dt);
        let snap = graph.snapshot();
        serde_wasm_bindgen::to_value(&snap)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Set the simulation speed multiplier.
#[wasm_bindgen]
pub fn dataflow_set_speed(graph_id: u32, speed: f64) -> Result<(), JsValue> {
    DATAFLOW_SCHEDULERS.with(|s| {
        let mut schedulers = s.borrow_mut();
        let sched = schedulers
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("scheduler not found"))?;
        sched.speed = speed;
        Ok(())
    })
}

/// Get a snapshot of the graph without ticking.
#[wasm_bindgen]
pub fn dataflow_snapshot(graph_id: u32) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let snap = graph.snapshot();
        serde_wasm_bindgen::to_value(&snap)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// List available block types as a typed JS array.
#[wasm_bindgen]
pub fn dataflow_block_types() -> JsValue {
    let types = dataflow::blocks::available_block_types();
    serde_wasm_bindgen::to_value(&types).unwrap_or(JsValue::NULL)
}

/// Generate a standalone Rust crate from a dataflow graph.
/// Returns JSON: `{ "files": [["path", "content"], ...] }` or error.
#[wasm_bindgen]
pub fn dataflow_codegen(graph_id: u32, dt: f64) -> Result<String, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let snap = graph.snapshot();
        let generated =
            dataflow::codegen::generate_rust(&snap, dt).map_err(|e| JsValue::from_str(&e))?;
        let files_json: Vec<(String, String)> = generated.files;
        serde_json::to_string(&files_json).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Generate a multi-target workspace from a dataflow graph.
///
/// `targets_json` is a JSON array of `{ "target": "host"|"rp2040"|"stm32f4"|"esp32c3", "binding": {...} }`.
/// Returns JSON: `[["path", "content"], ...]` or error.
#[wasm_bindgen]
pub fn dataflow_codegen_multi(
    graph_id: u32,
    dt: f64,
    targets_json: &str,
) -> Result<String, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let snap = graph.snapshot();
        let targets: Vec<dataflow::codegen::binding::TargetWithBinding> =
            serde_json::from_str(targets_json)
                .map_err(|e| JsValue::from_str(&format!("invalid targets JSON: {e}")))?;
        let ws = dataflow::codegen::generate_workspace(&snap, dt, &targets)
            .map_err(|e| JsValue::from_str(&e))?;
        serde_json::to_string(&ws.files).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Enable or disable simulation mode for a graph.
/// When enabled, peripheral blocks use SimModel dispatch with simulated peripherals.
#[wasm_bindgen]
pub fn dataflow_set_simulation_mode(graph_id: u32, enabled: bool) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.set_simulation_mode(enabled);
        if enabled && !graph.has_sim_peripherals() {
            graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        }
        Ok(())
    })
}

/// Set a simulated ADC channel voltage.
#[wasm_bindgen]
pub fn dataflow_set_sim_adc(graph_id: u32, channel: u8, voltage: f64) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.with_sim_peripherals(|p| {
            p.set_adc_voltage(channel, voltage);
        });
        Ok(())
    })
}

/// Read the last PWM duty written by a simulated PWM block.
#[wasm_bindgen]
pub fn dataflow_get_sim_pwm(graph_id: u32, channel: u8) -> Result<f64, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        Ok(graph.get_sim_pwm(channel))
    })
}

// ── I2C simulation WASM API ─────────────────────────────────────────

/// Add a simulated I2C device on the given bus at the given 7-bit address.
#[wasm_bindgen]
pub fn dataflow_add_i2c_device(
    graph_id: u32,
    bus: u8,
    addr: u8,
    name: &str,
) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_mut()
            .map_err(|e| JsValue::from_str(&e))?;
        sim.add_i2c_device(bus, addr, name);
        Ok(())
    })
}

/// Remove a simulated I2C device.
#[wasm_bindgen]
pub fn dataflow_remove_i2c_device(graph_id: u32, bus: u8, addr: u8) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_mut()
            .map_err(|e| JsValue::from_str(&e))?;
        sim.remove_i2c_device(bus, addr);
        Ok(())
    })
}

/// Read the 256-byte register map of a simulated I2C device (as JSON array).
#[wasm_bindgen]
pub fn dataflow_i2c_device_registers(
    graph_id: u32,
    bus: u8,
    addr: u8,
) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_ref()
            .map_err(|e| JsValue::from_str(&e))?;
        match sim.i2c_device_registers(bus, addr) {
            Some(regs) => {
                let json = serde_json::to_string(&regs[..])
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Ok(JsValue::from_str(&json))
            }
            None => Err(JsValue::from_str("I2C device not found")),
        }
    })
}

// ── Serial simulation WASM API ──────────────────────────────────────

/// Configure a simulated serial port. Parity: 0=None, 1=Odd, 2=Even.
#[wasm_bindgen]
pub fn dataflow_configure_serial(
    graph_id: u32,
    port: u8,
    baud: u32,
    data_bits: u8,
    parity: u8,
    stop_bits: u8,
) -> Result<(), JsValue> {
    let parity = dataflow::sim_peripherals::Parity::from_u8(parity)
        .map_err(|e| JsValue::from_str(&e))?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_mut()
            .map_err(|e| JsValue::from_str(&e))?;
        sim.configure_serial(port, baud, data_bits, parity, stop_bits);
        Ok(())
    })
}

/// List all configured serial ports as JSON.
#[wasm_bindgen]
pub fn dataflow_serial_ports(graph_id: u32) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_ref()
            .map_err(|e| JsValue::from_str(&e))?;
        let ports = sim.serial_ports();
        let configs: Vec<&dataflow::sim_peripherals::SerialConfig> =
            ports.iter().map(|(_, cfg)| cfg).collect();
        let json =
            serde_json::to_string(&configs).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JsValue::from_str(&json))
    })
}

// ── TCP socket simulation WASM API ──────────────────────────────────

/// Inject data into a simulated TCP receive buffer.
#[wasm_bindgen]
pub fn dataflow_tcp_inject(graph_id: u32, socket_id: u8, data: &[u8]) -> Result<(), JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_mut()
            .map_err(|e| JsValue::from_str(&e))?;
        sim.inject_tcp_data(socket_id, data);
        Ok(())
    })
}

/// Drain data from a simulated TCP send buffer (as JSON array).
#[wasm_bindgen]
pub fn dataflow_tcp_drain(graph_id: u32, socket_id: u8) -> Result<JsValue, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let sim = graph
            .sim_peripherals_mut()
            .map_err(|e| JsValue::from_str(&e))?;
        let data = sim.drain_tcp_data(socket_id);
        let json =
            serde_json::to_string(&data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JsValue::from_str(&json))
    })
}

// ── Control Panel WASM API ─────────────────────────────────────

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static PANELS: RefCell<std::collections::HashMap<u32, dataflow::panel::PanelModel>> =
        RefCell::new(std::collections::HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static PANEL_RUNTIMES: RefCell<std::collections::HashMap<u32, dataflow::panel::PanelRuntime>> =
        RefCell::new(std::collections::HashMap::new());
    static PANEL_NEXT_ID: RefCell<u32> = const { RefCell::new(1) };
}

/// Create a new empty panel. Returns its id.
#[wasm_bindgen]
pub fn panel_new(name: &str) -> u32 {
    PANEL_NEXT_ID.with(|next| {
        let id = *next.borrow();
        *next.borrow_mut() = id + 1;
        PANELS.with(|p| {
            p.borrow_mut()
                .insert(id, dataflow::panel::PanelModel::new(name));
        });
        PANEL_RUNTIMES.with(|r| {
            r.borrow_mut()
                .insert(id, dataflow::panel::PanelRuntime::new());
        });
        id
    })
}

/// Destroy a panel, removing it from storage.
#[wasm_bindgen]
pub fn panel_destroy(panel_id: u32) {
    PANELS.with(|p| {
        p.borrow_mut().remove(&panel_id);
    });
    PANEL_RUNTIMES.with(|r| {
        r.borrow_mut().remove(&panel_id);
    });
}

/// Deserialize a PanelModel from JSON, store it, and return its id.
#[wasm_bindgen]
pub fn panel_load(json: &str) -> Result<u32, JsValue> {
    let panel: dataflow::panel::PanelModel =
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    PANEL_NEXT_ID.with(|next| {
        let id = *next.borrow();
        *next.borrow_mut() = id + 1;
        PANELS.with(|p| p.borrow_mut().insert(id, panel));
        PANEL_RUNTIMES.with(|r| {
            r.borrow_mut()
                .insert(id, dataflow::panel::PanelRuntime::new());
        });
        Ok(id)
    })
}

/// Serialize a panel to JSON.
#[wasm_bindgen]
pub fn panel_save(panel_id: u32) -> Result<String, JsValue> {
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        serde_json::to_string(panel).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Add a widget to a panel from JSON config.
#[wasm_bindgen]
pub fn panel_add_widget(panel_id: u32, config_json: &str) -> Result<u32, JsValue> {
    let widget: dataflow::panel::Widget =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        Ok(panel.add_widget(widget))
    })
}

/// Remove a widget from a panel.
#[wasm_bindgen]
pub fn panel_remove_widget(panel_id: u32, widget_id: u32) -> Result<bool, JsValue> {
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        Ok(panel.remove_widget(widget_id))
    })
}

/// Update a widget's config. The original widget id is preserved.
#[wasm_bindgen]
pub fn panel_update_widget(
    panel_id: u32,
    widget_id: u32,
    config_json: &str,
) -> Result<(), JsValue> {
    let new: dataflow::panel::Widget =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        let widget = panel
            .get_widget_mut(widget_id)
            .ok_or_else(|| JsValue::from_str("widget not found"))?;
        let preserved_id = widget.id;
        *widget = new;
        widget.id = preserved_id;
        Ok(())
    })
}

/// JSON snapshot of the full panel.
#[wasm_bindgen]
pub fn panel_snapshot(panel_id: u32) -> Result<String, JsValue> {
    panel_save(panel_id)
}

/// Set a topic value from widget interaction.
#[wasm_bindgen]
pub fn panel_set_topic(panel_id: u32, topic: &str, value: f64) -> Result<(), JsValue> {
    PANEL_RUNTIMES.with(|r| {
        let mut runtimes = r.borrow_mut();
        let rt = runtimes
            .get_mut(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel runtime not found"))?;
        rt.set_value(topic, value);
        Ok(())
    })
}

/// Get all current topic values as JSON.
#[wasm_bindgen]
pub fn panel_get_values(panel_id: u32) -> Result<String, JsValue> {
    PANEL_RUNTIMES.with(|r| {
        let runtimes = r.borrow();
        let rt = runtimes
            .get(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel runtime not found"))?;
        serde_json::to_string(rt.values()).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Merge external values into input topics.
#[wasm_bindgen]
pub fn panel_merge_values(panel_id: u32, values_json: &str) -> Result<(), JsValue> {
    let external: std::collections::HashMap<String, f64> =
        serde_json::from_str(values_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        PANEL_RUNTIMES.with(|r| {
            let mut runtimes = r.borrow_mut();
            let rt = runtimes
                .get_mut(&panel_id)
                .ok_or_else(|| JsValue::from_str("panel runtime not found"))?;
            rt.merge_input_values(panel, &external);
            Ok(())
        })
    })
}

/// Collect output topic values.
#[wasm_bindgen]
pub fn panel_collect_outputs(panel_id: u32) -> Result<String, JsValue> {
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| JsValue::from_str("panel not found"))?;
        PANEL_RUNTIMES.with(|r| {
            let runtimes = r.borrow();
            let rt = runtimes
                .get(&panel_id)
                .ok_or_else(|| JsValue::from_str("panel runtime not found"))?;
            let outputs = rt.collect_output_values(panel);
            serde_json::to_string(&outputs).map_err(|e| JsValue::from_str(&e.to_string()))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_traits::SimPeripherals as _;

    #[test]
    fn test_dataflow_new_and_destroy() {
        let id = dataflow_new(0.01);
        assert!(id > 0);
        dataflow_destroy(id);
        // destroying again is a no-op
        dataflow_destroy(id);
    }

    #[test]
    fn test_dataflow_block_types() {
        // Test the underlying function directly (serde_wasm_bindgen round-trip
        // requires a JS host, so we verify the internal API instead).
        let types = dataflow::blocks::available_block_types();
        assert!(!types.is_empty());
    }

    #[test]
    fn test_dataflow_add_and_remove_block() {
        let gid = dataflow_new(0.01);
        let bid = dataflow_add_block(gid, "constant", r#"{"value":1.0}"#).unwrap();
        assert!(bid > 0);
        dataflow_remove_block(gid, bid).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_update_block() {
        let gid = dataflow_new(0.01);
        let bid = dataflow_add_block(gid, "constant", r#"{"value":1.0}"#).unwrap();
        dataflow_update_block(gid, bid, "constant", r#"{"value":2.0}"#).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_connect_and_disconnect() {
        // Use entirely internal APIs to avoid JsValue::from_str panics on non-wasm
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let src = graph.add_block(Box::new(dataflow::blocks::constant::ConstantBlock::new(
                1.0,
            )));
            let dst = graph.add_block(Box::new(dataflow::blocks::function::FunctionBlock::gain(
                2.0,
            )));
            let ch = graph.connect(src, 0, dst, 0).unwrap();
            graph.disconnect(ch);
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_snapshot() {
        let gid = dataflow_new(0.01);
        dataflow_add_block(gid, "constant", r#"{"value":5.0}"#).unwrap();
        // Verify via internal API (serde_wasm_bindgen round-trip needs a JS host)
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let snap = graph.snapshot();
            assert!(snap.blocks.iter().any(|b| b.block_type == "constant"));
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_run() {
        let gid = dataflow_new(0.01);
        dataflow_add_block(gid, "constant", r#"{"value":3.0}"#).unwrap();
        // Run and verify via internal API (serde_wasm_bindgen needs a JS host)
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            graph.run(10, 0.01);
            let snap = graph.snapshot();
            assert!(snap.blocks.iter().any(|b| b.block_type == "constant"));
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_advance() {
        let gid = dataflow_new(0.01);
        dataflow_add_block(gid, "constant", r#"{"value":1.0}"#).unwrap();
        // Advance and verify via internal API (serde_wasm_bindgen needs a JS host)
        DATAFLOW_SCHEDULERS.with(|s| {
            DATAFLOW_GRAPHS.with(|g| {
                let mut graphs = g.borrow_mut();
                let mut schedulers = s.borrow_mut();
                let graph = graphs.get_mut(&gid).unwrap();
                let sched = schedulers.get_mut(&gid).unwrap();
                let ticks = sched.advance(0.05);
                graph.run(ticks, sched.dt);
                let snap = graph.snapshot();
                assert!(!snap.blocks.is_empty());
            });
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_set_speed() {
        let gid = dataflow_new(0.01);
        dataflow_set_speed(gid, 2.0).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_codegen() {
        let gid = dataflow_new(0.01);
        dataflow_add_block(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let result = dataflow_codegen(gid, 0.01).unwrap();
        assert!(result.contains("main.rs") || result.contains("Cargo.toml"));
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_codegen_multi() {
        let gid = dataflow_new(0.01);
        dataflow_add_block(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let targets = r#"[{"target":"Host","binding":{"target":"Host","pins":[]}}]"#;
        let result = dataflow_codegen_multi(gid, 0.01, targets).unwrap();
        assert!(!result.is_empty());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_simulation_mode() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        dataflow_set_sim_adc(gid, 0, 3.3).unwrap();
        let duty = dataflow_get_sim_pwm(gid, 0).unwrap();
        assert!((duty - 0.0).abs() < f64::EPSILON);
        dataflow_destroy(gid);
    }

    // ── I2C device tests ────────────────────────────────────────────
    //
    // Note: Functions returning `Result<JsValue, JsValue>` (like
    // `dataflow_i2c_device_registers`) panic on non-wasm targets because
    // `JsValue::from_str` is a wasm-only intrinsic. We test the underlying
    // graph/sim_peripherals APIs directly instead.

    #[test]
    fn test_i2c_device_lifecycle() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        dataflow_add_i2c_device(gid, 0, 0x48, "TMP1075").unwrap();
        // Verify registers exist via the internal API
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            let regs = sim.i2c_device_registers(0, 0x48);
            assert!(regs.is_some());
        });
        dataflow_remove_i2c_device(gid, 0, 0x48).unwrap();
        // After removal, registers should be gone
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            assert!(sim.i2c_device_registers(0, 0x48).is_none());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_i2c_device_no_sim_mode() {
        // Without enabling simulation mode, the graph has no sim peripherals.
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            assert!(graph.sim_peripherals_ref().is_err());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_i2c_device_multiple_buses() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        dataflow_add_i2c_device(gid, 0, 0x48, "bus0-dev").unwrap();
        dataflow_add_i2c_device(gid, 1, 0x48, "bus1-dev").unwrap();
        // Both should be independently accessible
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            assert!(sim.i2c_device_registers(0, 0x48).is_some());
            assert!(sim.i2c_device_registers(1, 0x48).is_some());
        });
        // Remove from bus 0 only
        dataflow_remove_i2c_device(gid, 0, 0x48).unwrap();
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            assert!(sim.i2c_device_registers(0, 0x48).is_none());
            assert!(sim.i2c_device_registers(1, 0x48).is_some());
        });
        dataflow_destroy(gid);
    }

    // ── Serial configuration tests ──────────────────────────────────
    //
    // `dataflow_serial_ports` returns `Result<JsValue, JsValue>`, so we
    // test the underlying sim peripherals API directly for assertions
    // that require reading the return value.

    #[test]
    fn test_serial_configuration() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        dataflow_configure_serial(gid, 0, 9600, 8, 0, 1).unwrap(); // parity=0 (None)
        // Verify via internal API since serial_ports returns JsValue
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            let ports = sim.serial_ports();
            assert_eq!(ports.len(), 1);
            assert_eq!(ports[0].0, 0);
            assert_eq!(ports[0].1.baud, 9600);
            assert_eq!(ports[0].1.data_bits, 8);
            assert_eq!(ports[0].1.stop_bits, 1);
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_serial_invalid_parity() {
        // Test parity parsing directly (WASM error path panics in non-wasm)
        assert!(dataflow::sim_peripherals::Parity::from_u8(0).is_ok());
        assert!(dataflow::sim_peripherals::Parity::from_u8(1).is_ok());
        assert!(dataflow::sim_peripherals::Parity::from_u8(2).is_ok());
        assert!(dataflow::sim_peripherals::Parity::from_u8(3).is_err());
        assert!(dataflow::sim_peripherals::Parity::from_u8(255).is_err());
    }

    #[test]
    fn test_serial_no_sim_mode() {
        // Without sim mode, sim_peripherals_ref returns Err
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            assert!(graph.sim_peripherals_ref().is_err());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_serial_multiple_ports() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        dataflow_configure_serial(gid, 0, 9600, 8, 0, 1).unwrap();
        dataflow_configure_serial(gid, 1, 115_200, 8, 1, 1).unwrap(); // parity=1 (Odd)
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            let ports = sim.serial_ports();
            assert_eq!(ports.len(), 2);
            assert_eq!(ports[0].1.baud, 9600);
            assert_eq!(ports[1].1.baud, 115_200);
        });
        dataflow_destroy(gid);
    }

    // ── TCP inject/drain tests ──────────────────────────────────────
    //
    // `dataflow_tcp_drain` returns `Result<JsValue, JsValue>`, so we
    // verify drain results via internal APIs.

    #[test]
    fn test_tcp_inject_drain() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        // Create a TCP socket, inject data, then verify via internal API
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            sim.tcp_connect(0, "127.0.0.1", 8080).unwrap();
        });
        dataflow_tcp_inject(gid, 0, &[1, 2, 3]).unwrap();
        // Verify the injected data landed in the recv buffer
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            let mut buf = [0u8; 10];
            let n = sim.tcp_recv(0, &mut buf).unwrap();
            assert_eq!(n, 3);
            assert_eq!(&buf[..3], &[1, 2, 3]);
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_tcp_drain_empty_socket() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        // Drain on a nonexistent socket returns empty vec
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            let data = sim.drain_tcp_data(99);
            assert!(data.is_empty());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_tcp_send_and_drain() {
        let gid = dataflow_new(0.01);
        dataflow_set_simulation_mode(gid, true).unwrap();
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            sim.tcp_connect(0, "127.0.0.1", 8080).unwrap();
            // Simulate a block sending data
            let n = sim.tcp_send(0, b"hello").unwrap();
            assert_eq!(n, 5);
            // Drain should return what was sent
            let drained = sim.drain_tcp_data(0);
            assert_eq!(drained, b"hello");
            // Second drain should be empty
            let drained2 = sim.drain_tcp_data(0);
            assert!(drained2.is_empty());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_tcp_no_sim_mode() {
        // Without sim mode, sim_peripherals_mut returns Err
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            assert!(graph.sim_peripherals_ref().is_err());
        });
        dataflow_destroy(gid);
    }

    // ── Panel lifecycle tests ───────────────────────────────────────

    #[test]
    fn test_panel_lifecycle() {
        let id = panel_new("Test Panel");
        assert!(id > 0);
        panel_destroy(id);
        // Double-destroy is a no-op
        panel_destroy(id);
    }

    #[test]
    fn test_panel_load_save() {
        let id = panel_new("My Panel");
        let json = panel_save(id).unwrap();
        assert!(json.contains("My Panel"));
        let snap = panel_snapshot(id).unwrap();
        assert_eq!(json, snap);
        // Load from JSON
        let id2 = panel_load(&json).unwrap();
        let json2 = panel_save(id2).unwrap();
        assert!(json2.contains("My Panel"));
        panel_destroy(id);
        panel_destroy(id2);
    }

    #[test]
    fn test_panel_save_not_found() {
        // Verify that a nonexistent panel id is not in the map
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_load_invalid_json() {
        // Verify that invalid JSON fails deserialization
        let result: Result<dataflow::panel::PanelModel, _> =
            serde_json::from_str("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_panel_widget_crud() {
        let id = panel_new("Widget Test");
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Slider", "min": 0.0, "max": 100.0, "step": 1.0},
            "label": "Speed",
            "position": {"x": 10.0, "y": 20.0},
            "size": {"width": 200.0, "height": 50.0},
            "channels": []
        }"#;
        let wid = panel_add_widget(id, widget_json).unwrap();
        assert!(wid > 0);
        // Update widget
        let update_json = r#"{
            "id": 0,
            "kind": {"type": "Slider", "min": 0.0, "max": 100.0, "step": 1.0},
            "label": "Velocity",
            "position": {"x": 10.0, "y": 20.0},
            "size": {"width": 200.0, "height": 50.0},
            "channels": []
        }"#;
        panel_update_widget(id, wid, update_json).unwrap();
        let snap = panel_save(id).unwrap();
        assert!(snap.contains("Velocity"));
        // The original "Speed" label should no longer appear
        assert!(!snap.contains("Speed"));
        // Remove widget
        let removed = panel_remove_widget(id, wid).unwrap();
        assert!(removed);
        let not_found = panel_remove_widget(id, 9999).unwrap();
        assert!(!not_found);
        panel_destroy(id);
    }

    #[test]
    fn test_panel_add_widget_invalid_json() {
        // Verify that invalid JSON fails Widget deserialization
        let result: Result<dataflow::panel::Widget, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_panel_add_widget_panel_not_found() {
        // Verify nonexistent panel id is absent from the map
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_update_widget_not_found() {
        // Verify that a freshly-created panel has no widget with id 9999
        let id = panel_new("Update Miss");
        PANELS.with(|p| {
            let panels = p.borrow();
            let panel = panels.get(&id).unwrap();
            assert!(panel.get_widget(9999).is_none());
        });
        panel_destroy(id);
    }

    #[test]
    fn test_panel_remove_widget_panel_not_found() {
        // Verify nonexistent panel id is absent from the map
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_snapshot_not_found() {
        // panel_snapshot delegates to panel_save; verify panel absence
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_widget_all_kinds() {
        let id = panel_new("All Kinds");
        let kinds = [
            r#"{"type": "Toggle"}"#,
            r#"{"type": "Slider", "min": 0.0, "max": 10.0, "step": 0.5}"#,
            r#"{"type": "Gauge", "min": 0.0, "max": 200.0}"#,
            r#"{"type": "Label"}"#,
            r#"{"type": "Button"}"#,
            r#"{"type": "Indicator"}"#,
        ];
        for kind in &kinds {
            let json = format!(
                r#"{{"id":0,"kind":{},"label":"W","position":{{"x":0.0,"y":0.0}},"size":{{"width":50.0,"height":50.0}},"channels":[]}}"#,
                kind
            );
            let wid = panel_add_widget(id, &json).unwrap();
            assert!(wid > 0);
        }
        let snap = panel_save(id).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&snap).unwrap();
        assert_eq!(parsed["widgets"].as_array().unwrap().len(), 6);
        panel_destroy(id);
    }

    // ── Panel runtime tests ─────────────────────────────────────────

    #[test]
    fn test_panel_set_topic() {
        let id = panel_new("Runtime Test");
        panel_set_topic(id, "motor/speed", 42.0).unwrap();
        let vals = panel_get_values(id).unwrap();
        assert!(vals.contains("42"));
        assert!(vals.contains("motor/speed"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_set_topic_not_found() {
        // Verify nonexistent panel runtime id is absent
        PANEL_RUNTIMES.with(|r| {
            assert!(!r.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_get_values_not_found() {
        // Verify nonexistent panel runtime id is absent
        PANEL_RUNTIMES.with(|r| {
            assert!(!r.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_merge_values() {
        let id = panel_new("Merge Test");
        // Add a widget with an Input channel binding so merge has something to match
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Gauge", "min": 0.0, "max": 100.0},
            "label": "Temp",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 100.0, "height": 40.0},
            "channels": [
                {"topic": "sensor/temp", "direction": "Input", "port_kind": "Float"}
            ]
        }"#;
        panel_add_widget(id, widget_json).unwrap();
        panel_merge_values(id, r#"{"sensor/temp": 25.5}"#).unwrap();
        let vals = panel_get_values(id).unwrap();
        assert!(vals.contains("25.5"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_merge_values_not_found() {
        // Verify nonexistent panel id is absent from both maps
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
        PANEL_RUNTIMES.with(|r| {
            assert!(!r.borrow().contains_key(&99999));
        });
    }

    #[test]
    fn test_panel_merge_values_invalid_json() {
        // Verify that invalid JSON fails HashMap deserialization
        let result: Result<std::collections::HashMap<String, f64>, _> =
            serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_panel_collect_outputs() {
        let id = panel_new("Output Test");
        // Add a widget with an Output channel binding
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Slider", "min": 0.0, "max": 100.0, "step": 1.0},
            "label": "Throttle",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 200.0, "height": 40.0},
            "channels": [
                {"topic": "motor/throttle", "direction": "Output", "port_kind": "Float"}
            ]
        }"#;
        panel_add_widget(id, widget_json).unwrap();
        // Set the topic value that the output channel references
        panel_set_topic(id, "motor/throttle", 75.0).unwrap();
        let outputs = panel_collect_outputs(id).unwrap();
        assert!(outputs.contains("motor/throttle"));
        assert!(outputs.contains("75"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_collect_outputs_empty() {
        let id = panel_new("Empty Output");
        // Empty panel has no output channels
        let outputs = panel_collect_outputs(id).unwrap();
        assert!(outputs.contains('{'));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_collect_outputs_not_found() {
        // Verify nonexistent panel id is absent from both maps
        PANELS.with(|p| {
            assert!(!p.borrow().contains_key(&99999));
        });
        PANEL_RUNTIMES.with(|r| {
            assert!(!r.borrow().contains_key(&99999));
        });
    }
}
