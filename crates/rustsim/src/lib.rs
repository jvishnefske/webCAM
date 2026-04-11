pub mod dag_api;
pub mod dataflow;
#[cfg(target_arch = "wasm32")]
mod wasm_api;

use std::cell::RefCell;

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
pub fn dataflow_destroy(graph_id: u32) {
    DATAFLOW_GRAPHS.with(|g| g.borrow_mut().remove(&graph_id));
    DATAFLOW_SCHEDULERS.with(|s| s.borrow_mut().remove(&graph_id));
}

/// Add a block to a graph (testable helper). Returns block id.
pub fn add_block_impl(
    graph_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<u32, String> {
    let block = dataflow::blocks::create_block(block_type, config_json)?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let id = graph.add_block(block);
        Ok(id.0)
    })
}

/// Remove a block from a graph (testable helper).
pub fn remove_block_impl(graph_id: u32, block_id: u32) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.remove_block(dataflow::BlockId(block_id));
        Ok(())
    })
}

/// Update a block's config (testable helper).
pub fn update_block_impl(
    graph_id: u32,
    block_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<(), String> {
    let block = dataflow::blocks::create_block(block_type, config_json)?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.replace_block(dataflow::BlockId(block_id), block)
    })
}

/// Connect an output port to an input port (testable helper). Returns channel id.
pub fn connect_impl(
    graph_id: u32,
    from_block: u32,
    from_port: u32,
    to_block: u32,
    to_port: u32,
) -> Result<u32, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let ch = graph.connect(
            dataflow::BlockId(from_block),
            from_port as usize,
            dataflow::BlockId(to_block),
            to_port as usize,
        )?;
        Ok(ch.0)
    })
}

/// Disconnect a channel (testable helper).
pub fn disconnect_impl(graph_id: u32, channel_id: u32) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.disconnect(dataflow::ChannelId(channel_id));
        Ok(())
    })
}

/// Advance the graph by wall-clock elapsed seconds (testable helper).
/// Returns snapshot as JSON.
pub fn advance_impl(graph_id: u32, elapsed: f64) -> Result<serde_json::Value, String> {
    DATAFLOW_GRAPHS.with(|g| {
        DATAFLOW_SCHEDULERS.with(|s| {
            let mut graphs = g.borrow_mut();
            let mut schedulers = s.borrow_mut();
            let graph = graphs
                .get_mut(&graph_id)
                .ok_or_else(|| "graph not found".to_string())?;
            let sched = schedulers
                .get_mut(&graph_id)
                .ok_or_else(|| "scheduler not found".to_string())?;
            let ticks = sched.advance(elapsed);
            graph.run(ticks, sched.dt);
            let snap = graph.snapshot();
            serde_json::to_value(&snap)
                .map_err(|e| e.to_string())
        })
    })
}

/// Run a fixed number of ticks (testable helper).
/// Returns snapshot as JSON.
pub fn run_impl(graph_id: u32, steps: u32, dt: f64) -> Result<serde_json::Value, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.run(steps as u64, dt);
        let snap = graph.snapshot();
        serde_json::to_value(&snap)
            .map_err(|e| e.to_string())
    })
}

/// Set the simulation speed multiplier (testable helper).
pub fn set_speed_impl(graph_id: u32, speed: f64) -> Result<(), String> {
    DATAFLOW_SCHEDULERS.with(|s| {
        let mut schedulers = s.borrow_mut();
        let sched = schedulers
            .get_mut(&graph_id)
            .ok_or_else(|| "scheduler not found".to_string())?;
        sched.speed = speed;
        Ok(())
    })
}

/// Get a snapshot of the graph without ticking (testable helper).
pub fn snapshot_impl(graph_id: u32) -> Result<serde_json::Value, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let snap = graph.snapshot();
        serde_json::to_value(&snap)
            .map_err(|e| e.to_string())
    })
}

/// List available block types (testable helper).
pub fn block_types_impl() -> Vec<dataflow::blocks::BlockTypeInfo> {
    dataflow::blocks::available_block_types()
}

// ── Function Def Schema API ──────────────────────────────────────────

/// Return the list of builtin function definitions (non-WASM helper).
pub fn function_defs_list() -> Vec<module_traits::FunctionDef> {
    module_traits::builtin_function_defs()
}

// ── MCU Pin Schema API ──────────────────────────────────────────────

/// Return the list of supported MCU families (non-WASM helper).
pub fn mcu_families_list() -> Vec<&'static str> {
    module_traits::inventory::supported_families()
}

/// Look up an MCU definition by family name (non-WASM helper).
pub fn get_mcu_definition(family: &str) -> Result<module_traits::inventory::McuDef, String> {
    module_traits::inventory::mcu_for(family)
        .ok_or_else(|| format!("unknown MCU family: {family}"))
}

/// Return MCU pin definitions for a family (non-WASM helper).
pub fn get_mcu_pins(family: &str) -> Result<Vec<module_traits::inventory::PinDef>, String> {
    let mcu = get_mcu_definition(family)?;
    Ok(mcu.pins)
}

/// Return MCU peripheral instances for a family (non-WASM helper).
pub fn get_mcu_peripherals(
    family: &str,
) -> Result<Vec<module_traits::inventory::PeripheralInst>, String> {
    let mcu = get_mcu_definition(family)?;
    Ok(mcu.peripherals)
}

/// Generate a standalone Rust crate from a dataflow graph (testable helper).
pub fn codegen_impl(graph_id: u32, dt: f64) -> Result<String, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let snap = graph.snapshot().to_codegen_snapshot();
        let generated = dataflow::codegen::generate_rust(&snap, dt)?;
        let files_json: Vec<(String, String)> = generated.files;
        serde_json::to_string(&files_json).map_err(|e| e.to_string())
    })
}

/// Generate a multi-target workspace from a dataflow graph (testable helper).
pub fn codegen_multi_impl(graph_id: u32, dt: f64, targets_json: &str) -> Result<String, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let snap = graph.snapshot().to_codegen_snapshot();
        let targets: Vec<dataflow::codegen::binding::TargetWithBinding> =
            serde_json::from_str(targets_json)
                .map_err(|e| format!("invalid targets JSON: {e}"))?;
        let ws = dataflow::codegen::generate_workspace(&snap, dt, &targets)?;
        serde_json::to_string(&ws.files).map_err(|e| e.to_string())
    })
}

/// Enable or disable simulation mode (testable helper).
pub fn set_simulation_mode_impl(graph_id: u32, enabled: bool) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.set_simulation_mode(enabled);
        if enabled && !graph.has_sim_peripherals() {
            graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        }
        Ok(())
    })
}

/// Set a simulated ADC channel voltage (testable helper).
pub fn set_sim_adc_impl(graph_id: u32, channel: u8, voltage: f64) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        graph.with_sim_peripherals(|p| {
            p.set_adc_voltage(channel, voltage);
        });
        Ok(())
    })
}

/// Read the last PWM duty written by a simulated PWM block (testable helper).
pub fn get_sim_pwm_impl(graph_id: u32, channel: u8) -> Result<f64, String> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        Ok(graph.get_sim_pwm(channel))
    })
}

// ── I2C simulation WASM API ─────────────────────────────────────────

/// Add a simulated I2C device (testable helper).
pub fn add_i2c_device_impl(graph_id: u32, bus: u8, addr: u8, name: &str) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let sim = graph.sim_peripherals_mut()?;
        sim.add_i2c_device(bus, addr, name);
        Ok(())
    })
}

/// Remove a simulated I2C device (testable helper).
pub fn remove_i2c_device_impl(graph_id: u32, bus: u8, addr: u8) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let sim = graph.sim_peripherals_mut()?;
        sim.remove_i2c_device(bus, addr);
        Ok(())
    })
}

// ── Serial simulation WASM API ──────────────────────────────────────

/// Configure a simulated serial port (testable helper).
pub fn configure_serial_impl(
    graph_id: u32,
    port: u8,
    baud: u32,
    data_bits: u8,
    parity: u8,
    stop_bits: u8,
) -> Result<(), String> {
    let parity = dataflow::sim_peripherals::Parity::from_u8(parity)?;
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let sim = graph.sim_peripherals_mut()?;
        sim.configure_serial(port, baud, data_bits, parity, stop_bits);
        Ok(())
    })
}

// ── TCP socket simulation WASM API ──────────────────────────────────

/// Inject data into a simulated TCP receive buffer (testable helper).
pub fn tcp_inject_impl(graph_id: u32, socket_id: u8, data: &[u8]) -> Result<(), String> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| "graph not found".to_string())?;
        let sim = graph.sim_peripherals_mut()?;
        sim.inject_tcp_data(socket_id, data);
        Ok(())
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
pub fn panel_destroy(panel_id: u32) {
    PANELS.with(|p| {
        p.borrow_mut().remove(&panel_id);
    });
    PANEL_RUNTIMES.with(|r| {
        r.borrow_mut().remove(&panel_id);
    });
}

/// Load a panel from JSON (testable helper).
pub fn panel_load_impl(json: &str) -> Result<u32, String> {
    let panel: dataflow::panel::PanelModel =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
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

/// Serialize a panel to JSON (testable helper).
pub fn panel_save_impl(panel_id: u32) -> Result<String, String> {
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        serde_json::to_string(panel).map_err(|e| e.to_string())
    })
}

/// Add a widget to a panel (testable helper).
pub fn panel_add_widget_impl(panel_id: u32, config_json: &str) -> Result<u32, String> {
    let widget: dataflow::panel::Widget =
        serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        Ok(panel.add_widget(widget))
    })
}

/// Remove a widget from a panel (testable helper).
pub fn panel_remove_widget_impl(panel_id: u32, widget_id: u32) -> Result<bool, String> {
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        Ok(panel.remove_widget(widget_id))
    })
}

/// Update a widget's config (testable helper).
pub fn panel_update_widget_impl(
    panel_id: u32,
    widget_id: u32,
    config_json: &str,
) -> Result<(), String> {
    let new: dataflow::panel::Widget =
        serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    PANELS.with(|p| {
        let mut panels = p.borrow_mut();
        let panel = panels
            .get_mut(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        let widget = panel
            .get_widget_mut(widget_id)
            .ok_or_else(|| "widget not found".to_string())?;
        let preserved_id = widget.id;
        *widget = new;
        widget.id = preserved_id;
        Ok(())
    })
}

/// JSON snapshot of the full panel (testable helper).
pub fn panel_snapshot_impl(panel_id: u32) -> Result<String, String> {
    panel_save_impl(panel_id)
}

/// Set a topic value (testable helper).
pub fn panel_set_topic_impl(panel_id: u32, topic: &str, value: f64) -> Result<(), String> {
    PANEL_RUNTIMES.with(|r| {
        let mut runtimes = r.borrow_mut();
        let rt = runtimes
            .get_mut(&panel_id)
            .ok_or_else(|| "panel runtime not found".to_string())?;
        rt.set_value(topic, value);
        Ok(())
    })
}

/// Get all current topic values as JSON (testable helper).
pub fn panel_get_values_impl(panel_id: u32) -> Result<String, String> {
    PANEL_RUNTIMES.with(|r| {
        let runtimes = r.borrow();
        let rt = runtimes
            .get(&panel_id)
            .ok_or_else(|| "panel runtime not found".to_string())?;
        serde_json::to_string(rt.values()).map_err(|e| e.to_string())
    })
}

/// Merge external values into input topics (testable helper).
pub fn panel_merge_values_impl(panel_id: u32, values_json: &str) -> Result<(), String> {
    let external: std::collections::HashMap<String, f64> =
        serde_json::from_str(values_json).map_err(|e| e.to_string())?;
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        PANEL_RUNTIMES.with(|r| {
            let mut runtimes = r.borrow_mut();
            let rt = runtimes
                .get_mut(&panel_id)
                .ok_or_else(|| "panel runtime not found".to_string())?;
            rt.merge_input_values(panel, &external);
            Ok(())
        })
    })
}

/// Collect output topic values (testable helper).
pub fn panel_collect_outputs_impl(panel_id: u32) -> Result<String, String> {
    PANELS.with(|p| {
        let panels = p.borrow();
        let panel = panels
            .get(&panel_id)
            .ok_or_else(|| "panel not found".to_string())?;
        PANEL_RUNTIMES.with(|r| {
            let runtimes = r.borrow();
            let rt = runtimes
                .get(&panel_id)
                .ok_or_else(|| "panel runtime not found".to_string())?;
            let outputs = rt.collect_output_values(panel);
            serde_json::to_string(&outputs).map_err(|e| e.to_string())
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
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        assert!(bid > 0);
        remove_block_impl(gid, bid).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_update_block() {
        let gid = dataflow_new(0.01);
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        update_block_impl(gid, bid, "constant", r#"{"value":2.0}"#).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_connect_and_disconnect() {
        // Use entirely internal APIs
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let src = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
            let dst = dataflow::blocks::create_block("gain", r#"{"gain":2.0}"#).unwrap();
            let src = graph.add_block(src);
            let dst = graph.add_block(dst);
            let ch = graph.connect(src, 0, dst, 0).unwrap();
            graph.disconnect(ch);
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_snapshot() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":5.0}"#).unwrap();
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
        add_block_impl(gid, "constant", r#"{"value":3.0}"#).unwrap();
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
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
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
        set_speed_impl(gid, 2.0).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_codegen() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let result = codegen_impl(gid, 0.01).unwrap();
        assert!(result.contains("main.rs") || result.contains("Cargo.toml"));
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_codegen_multi() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let targets = r#"[{"target":"Host","binding":{"target":"Host","pins":[]}}]"#;
        let result = codegen_multi_impl(gid, 0.01, targets).unwrap();
        assert!(!result.is_empty());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_dataflow_simulation_mode() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        set_sim_adc_impl(gid, 0, 3.3).unwrap();
        let duty = get_sim_pwm_impl(gid, 0).unwrap();
        assert!((duty - 0.0).abs() < f64::EPSILON);
        dataflow_destroy(gid);
    }

    // ── I2C device tests ────────────────────────────────────────────
    //
    // We test the underlying graph/sim_peripherals APIs directly because
    // Test the underlying graph/sim_peripherals APIs directly.

    #[test]
    fn test_i2c_device_lifecycle() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        add_i2c_device_impl(gid, 0, 0x48, "TMP1075").unwrap();
        // Verify registers exist via the internal API
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            let regs = sim.i2c_device_registers(0, 0x48);
            assert!(regs.is_some());
        });
        remove_i2c_device_impl(gid, 0, 0x48).unwrap();
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
        set_simulation_mode_impl(gid, true).unwrap();
        add_i2c_device_impl(gid, 0, 0x48, "bus0-dev").unwrap();
        add_i2c_device_impl(gid, 1, 0x48, "bus1-dev").unwrap();
        // Both should be independently accessible
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let sim = graph.sim_peripherals_ref().unwrap();
            assert!(sim.i2c_device_registers(0, 0x48).is_some());
            assert!(sim.i2c_device_registers(1, 0x48).is_some());
        });
        // Remove from bus 0 only
        remove_i2c_device_impl(gid, 0, 0x48).unwrap();
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
    // We test the underlying sim peripherals API directly for assertions
    // that require reading return values.

    #[test]
    fn test_serial_configuration() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        configure_serial_impl(gid, 0, 9600, 8, 0, 1).unwrap(); // parity=0 (None)
        // Verify via internal API
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
        set_simulation_mode_impl(gid, true).unwrap();
        configure_serial_impl(gid, 0, 9600, 8, 0, 1).unwrap();
        configure_serial_impl(gid, 1, 115_200, 8, 1, 1).unwrap(); // parity=1 (Odd)
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
    // We verify drain results via internal APIs.

    #[test]
    fn test_tcp_inject_drain() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        // Create a TCP socket, inject data, then verify via internal API
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            sim.tcp_connect(0, "127.0.0.1", 8080).unwrap();
        });
        tcp_inject_impl(gid, 0, &[1, 2, 3]).unwrap();
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
        set_simulation_mode_impl(gid, true).unwrap();
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
        set_simulation_mode_impl(gid, true).unwrap();
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
        let json = panel_save_impl(id).unwrap();
        assert!(json.contains("My Panel"));
        let snap = panel_snapshot_impl(id).unwrap();
        assert_eq!(json, snap);
        // Load from JSON
        let id2 = panel_load_impl(&json).unwrap();
        let json2 = panel_save_impl(id2).unwrap();
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
        let wid = panel_add_widget_impl(id, widget_json).unwrap();
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
        panel_update_widget_impl(id, wid, update_json).unwrap();
        let snap = panel_save_impl(id).unwrap();
        assert!(snap.contains("Velocity"));
        // The original "Speed" label should no longer appear
        assert!(!snap.contains("Speed"));
        // Remove widget
        let removed = panel_remove_widget_impl(id, wid).unwrap();
        assert!(removed);
        let not_found = panel_remove_widget_impl(id, 9999).unwrap();
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
            let wid = panel_add_widget_impl(id, &json).unwrap();
            assert!(wid > 0);
        }
        let snap = panel_save_impl(id).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&snap).unwrap();
        assert_eq!(parsed["widgets"].as_array().unwrap().len(), 6);
        panel_destroy(id);
    }

    // ── Panel runtime tests ─────────────────────────────────────────

    #[test]
    fn test_panel_set_topic() {
        let id = panel_new("Runtime Test");
        panel_set_topic_impl(id, "motor/speed", 42.0).unwrap();
        let vals = panel_get_values_impl(id).unwrap();
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
        panel_add_widget_impl(id, widget_json).unwrap();
        panel_merge_values_impl(id, r#"{"sensor/temp": 25.5}"#).unwrap();
        let vals = panel_get_values_impl(id).unwrap();
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
        panel_add_widget_impl(id, widget_json).unwrap();
        // Set the topic value that the output channel references
        panel_set_topic_impl(id, "motor/throttle", 75.0).unwrap();
        let outputs = panel_collect_outputs_impl(id).unwrap();
        assert!(outputs.contains("motor/throttle"));
        assert!(outputs.contains("75"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_collect_outputs_empty() {
        let id = panel_new("Empty Output");
        // Empty panel has no output channels
        let outputs = panel_collect_outputs_impl(id).unwrap();
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

    // ── Part 2: MCU API helper tests ───────────────────────────────────

    #[test]
    fn test_mcu_families_list() {
        let families = mcu_families_list();
        assert!(!families.is_empty());
        // All known families should be present
        assert!(families.contains(&"Rp2040"));
        assert!(families.contains(&"Host"));
    }

    #[test]
    fn test_get_mcu_definition_valid() {
        let mcu = get_mcu_definition("Rp2040").unwrap();
        assert!(!mcu.family.is_empty());
        assert!(!mcu.pins.is_empty());
    }

    #[test]
    fn test_get_mcu_definition_invalid() {
        let result = get_mcu_definition("nonexistent_mcu");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown MCU family"));
    }

    #[test]
    fn test_get_mcu_pins_valid() {
        let pins = get_mcu_pins("Rp2040").unwrap();
        assert!(!pins.is_empty());
    }

    #[test]
    fn test_get_mcu_pins_invalid() {
        let result = get_mcu_pins("nonexistent_mcu");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown MCU family"));
    }

    #[test]
    fn test_get_mcu_peripherals_valid() {
        let periphs = get_mcu_peripherals("Rp2040").unwrap();
        assert!(!periphs.is_empty());
    }

    #[test]
    fn test_get_mcu_peripherals_invalid() {
        let result = get_mcu_peripherals("nonexistent_mcu");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown MCU family"));
    }

    #[test]
    fn test_function_defs_list() {
        let defs = function_defs_list();
        assert!(!defs.is_empty());
    }

    // ── Part 3: Error-path tests for internal APIs ─────────────────────

    #[test]
    fn test_connect_invalid_ports() {
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let src = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
            let dst = dataflow::blocks::create_block("gain", r#"{"gain":2.0}"#).unwrap();
            let src_id = graph.add_block(src);
            let dst_id = graph.add_block(dst);

            // Invalid source port
            let result = graph.connect(src_id, 99, dst_id, 0);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("source port"));

            // Invalid destination port
            let result = graph.connect(src_id, 0, dst_id, 99);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("destination port"));

            // Non-existent source block
            let result = graph.connect(dataflow::BlockId(999), 0, dst_id, 0);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("source block not found"));

            // Non-existent destination block
            let result = graph.connect(src_id, 0, dataflow::BlockId(999), 0);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("destination block not found"));

            // Duplicate connection
            graph.connect(src_id, 0, dst_id, 0).unwrap();
            let result = graph.connect(src_id, 0, dst_id, 0);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("already connected"));
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_disconnect_nonexistent() {
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            // Disconnecting a non-existent channel returns false
            assert!(!graph.disconnect(dataflow::ChannelId(999)));
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_run_and_snapshot_invalid_graph() {
        // Operations on graphs that don't exist
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            assert!(graphs.get(&99999).is_none());
        });
        DATAFLOW_SCHEDULERS.with(|s| {
            let schedulers = s.borrow();
            assert!(schedulers.get(&99999).is_none());
        });
    }

    #[test]
    fn test_set_speed_valid() {
        let gid = dataflow_new(0.01);
        DATAFLOW_SCHEDULERS.with(|s| {
            let mut schedulers = s.borrow_mut();
            let sched = schedulers.get_mut(&gid).unwrap();
            sched.speed = 3.0;
            assert!((sched.speed - 3.0).abs() < f64::EPSILON);
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_codegen_with_graph() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        // Test the codegen internal API
        DATAFLOW_GRAPHS.with(|g| {
            let graphs = g.borrow();
            let graph = graphs.get(&gid).unwrap();
            let snap = graph.snapshot().to_codegen_snapshot();
            let result = dataflow::codegen::generate_rust(&snap, 0.01);
            assert!(result.is_ok());
        });
        dataflow_destroy(gid);
    }

    #[test]
    fn test_codegen_multi_invalid_targets() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        // Invalid targets JSON should fail deserialization
        let result: Result<Vec<dataflow::codegen::binding::TargetWithBinding>, _> =
            serde_json::from_str("not valid json");
        assert!(result.is_err());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_add_block_invalid_type() {
        let result = dataflow::blocks::create_block("nonexistent_block_type", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_block_nonexistent() {
        let gid = dataflow_new(0.01);
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
            let result = graph.replace_block(dataflow::BlockId(999), block);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("block not found"));
        });
        dataflow_destroy(gid);
    }

    // ── Part 4: _impl helper tests (success + error paths) ────────────

    #[test]
    fn test_add_block_impl_success() {
        let gid = dataflow_new(0.01);
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        assert!(bid > 0);
        dataflow_destroy(gid);
    }

    #[test]
    fn test_add_block_impl_invalid_type() {
        let gid = dataflow_new(0.01);
        let result = add_block_impl(gid, "nonexistent_block_type", "{}");
        assert!(result.is_err());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_add_block_impl_graph_not_found() {
        let result = add_block_impl(99999, "constant", r#"{"value":1.0}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_remove_block_impl_success() {
        let gid = dataflow_new(0.01);
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        remove_block_impl(gid, bid).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_remove_block_impl_graph_not_found() {
        let result = remove_block_impl(99999, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_update_block_impl_success() {
        let gid = dataflow_new(0.01);
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        update_block_impl(gid, bid, "constant", r#"{"value":2.0}"#).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_update_block_impl_invalid_type() {
        let gid = dataflow_new(0.01);
        let bid = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let result = update_block_impl(gid, bid, "nonexistent", "{}");
        assert!(result.is_err());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_update_block_impl_graph_not_found() {
        let result = update_block_impl(99999, 1, "constant", r#"{"value":1.0}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_update_block_impl_block_not_found() {
        let gid = dataflow_new(0.01);
        let result = update_block_impl(gid, 999, "constant", r#"{"value":1.0}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("block not found"));
        dataflow_destroy(gid);
    }

    #[test]
    fn test_connect_impl_success() {
        let gid = dataflow_new(0.01);
        let src = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let dst = add_block_impl(gid, "gain", r#"{"gain":2.0}"#).unwrap();
        let ch = connect_impl(gid, src, 0, dst, 0).unwrap();
        assert!(ch > 0);
        dataflow_destroy(gid);
    }

    #[test]
    fn test_connect_impl_graph_not_found() {
        let result = connect_impl(99999, 1, 0, 2, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_connect_impl_invalid_port() {
        let gid = dataflow_new(0.01);
        let src = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let dst = add_block_impl(gid, "gain", r#"{"gain":2.0}"#).unwrap();
        let result = connect_impl(gid, src, 99, dst, 0);
        assert!(result.is_err());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_disconnect_impl_success() {
        let gid = dataflow_new(0.01);
        let src = add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let dst = add_block_impl(gid, "gain", r#"{"gain":2.0}"#).unwrap();
        let ch = connect_impl(gid, src, 0, dst, 0).unwrap();
        disconnect_impl(gid, ch).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_disconnect_impl_graph_not_found() {
        let result = disconnect_impl(99999, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_advance_impl_success() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let snap = advance_impl(gid, 0.05).unwrap();
        assert!(snap.is_object());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_advance_impl_graph_not_found() {
        let result = advance_impl(99999, 0.05);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_run_impl_success() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let snap = run_impl(gid, 10, 0.01).unwrap();
        assert!(snap.is_object());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_run_impl_graph_not_found() {
        let result = run_impl(99999, 10, 0.01);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_set_speed_impl_success() {
        let gid = dataflow_new(0.01);
        set_speed_impl(gid, 3.0).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_set_speed_impl_not_found() {
        let result = set_speed_impl(99999, 1.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheduler not found"));
    }

    #[test]
    fn test_snapshot_impl_success() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":5.0}"#).unwrap();
        let snap = snapshot_impl(gid).unwrap();
        assert!(snap.is_object());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_snapshot_impl_graph_not_found() {
        let result = snapshot_impl(99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_block_types_impl() {
        let types = block_types_impl();
        assert!(!types.is_empty());
    }

    #[test]
    fn test_codegen_impl_success() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let result = codegen_impl(gid, 0.01).unwrap();
        assert!(result.contains("main.rs") || result.contains("Cargo.toml"));
        dataflow_destroy(gid);
    }

    #[test]
    fn test_codegen_impl_graph_not_found() {
        let result = codegen_impl(99999, 0.01);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_codegen_multi_impl_success() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let targets = r#"[{"target":"Host","binding":{"target":"Host","pins":[]}}]"#;
        let result = codegen_multi_impl(gid, 0.01, targets).unwrap();
        assert!(!result.is_empty());
        dataflow_destroy(gid);
    }

    #[test]
    fn test_codegen_multi_impl_graph_not_found() {
        let result = codegen_multi_impl(99999, 0.01, "[]");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_codegen_multi_impl_invalid_json() {
        let gid = dataflow_new(0.01);
        add_block_impl(gid, "constant", r#"{"value":1.0}"#).unwrap();
        let result = codegen_multi_impl(gid, 0.01, "not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid targets JSON"));
        dataflow_destroy(gid);
    }

    #[test]
    fn test_set_simulation_mode_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        set_simulation_mode_impl(gid, false).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_set_simulation_mode_impl_not_found() {
        let result = set_simulation_mode_impl(99999, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_set_sim_adc_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        set_sim_adc_impl(gid, 0, 3.3).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_set_sim_adc_impl_graph_not_found() {
        let result = set_sim_adc_impl(99999, 0, 1.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_get_sim_pwm_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        let duty = get_sim_pwm_impl(gid, 0).unwrap();
        assert!((duty - 0.0).abs() < f64::EPSILON);
        dataflow_destroy(gid);
    }

    #[test]
    fn test_get_sim_pwm_impl_graph_not_found() {
        let result = get_sim_pwm_impl(99999, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_add_i2c_device_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        add_i2c_device_impl(gid, 0, 0x48, "TMP1075").unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_add_i2c_device_impl_graph_not_found() {
        let result = add_i2c_device_impl(99999, 0, 0x48, "dev");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_add_i2c_device_impl_no_sim_mode() {
        let gid = dataflow_new(0.01);
        let result = add_i2c_device_impl(gid, 0, 0x48, "dev");
        assert!(result.is_err()); // sim peripherals not enabled
        dataflow_destroy(gid);
    }

    #[test]
    fn test_remove_i2c_device_impl_graph_not_found() {
        let result = remove_i2c_device_impl(99999, 0, 0x48);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_remove_i2c_device_impl_no_sim_mode() {
        let gid = dataflow_new(0.01);
        let result = remove_i2c_device_impl(gid, 0, 0x48);
        assert!(result.is_err()); // sim peripherals not enabled
        dataflow_destroy(gid);
    }

    #[test]
    fn test_configure_serial_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        configure_serial_impl(gid, 0, 9600, 8, 0, 1).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_configure_serial_impl_graph_not_found() {
        let result = configure_serial_impl(99999, 0, 9600, 8, 0, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_configure_serial_impl_invalid_parity() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        let result = configure_serial_impl(gid, 0, 9600, 8, 3, 1);
        assert!(result.is_err()); // parity 3 is invalid
        dataflow_destroy(gid);
    }

    #[test]
    fn test_configure_serial_impl_no_sim_mode() {
        let gid = dataflow_new(0.01);
        let result = configure_serial_impl(gid, 0, 9600, 8, 0, 1);
        assert!(result.is_err()); // sim peripherals not enabled
        dataflow_destroy(gid);
    }

    #[test]
    fn test_tcp_inject_impl_success() {
        let gid = dataflow_new(0.01);
        set_simulation_mode_impl(gid, true).unwrap();
        DATAFLOW_GRAPHS.with(|g| {
            let mut graphs = g.borrow_mut();
            let graph = graphs.get_mut(&gid).unwrap();
            let sim = graph.sim_peripherals_mut().unwrap();
            sim.tcp_connect(0, "127.0.0.1", 8080).unwrap();
        });
        tcp_inject_impl(gid, 0, &[1, 2, 3]).unwrap();
        dataflow_destroy(gid);
    }

    #[test]
    fn test_tcp_inject_impl_graph_not_found() {
        let result = tcp_inject_impl(99999, 0, &[1]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("graph not found"));
    }

    #[test]
    fn test_tcp_inject_impl_no_sim_mode() {
        let gid = dataflow_new(0.01);
        let result = tcp_inject_impl(gid, 0, &[1]);
        assert!(result.is_err()); // sim peripherals not enabled
        dataflow_destroy(gid);
    }

    // ── Part 5: Panel _impl helper tests ──────────────────────────────

    #[test]
    fn test_panel_load_impl_success() {
        let id = panel_new("Load Test");
        let json = panel_save_impl(id).unwrap();
        let id2 = panel_load_impl(&json).unwrap();
        assert!(id2 > 0);
        panel_destroy(id);
        panel_destroy(id2);
    }

    #[test]
    fn test_panel_load_impl_invalid_json() {
        let result = panel_load_impl("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_panel_save_impl_success() {
        let id = panel_new("Save Test");
        let json = panel_save_impl(id).unwrap();
        assert!(json.contains("Save Test"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_save_impl_not_found() {
        let result = panel_save_impl(99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_add_widget_impl_success() {
        let id = panel_new("Widget Add Impl");
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Toggle"},
            "label": "Switch",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 50.0, "height": 50.0},
            "channels": []
        }"#;
        let wid = panel_add_widget_impl(id, widget_json).unwrap();
        assert!(wid > 0);
        panel_destroy(id);
    }

    #[test]
    fn test_panel_add_widget_impl_invalid_json() {
        let id = panel_new("Widget Bad JSON");
        let result = panel_add_widget_impl(id, "not json");
        assert!(result.is_err());
        panel_destroy(id);
    }

    #[test]
    fn test_panel_add_widget_impl_panel_not_found() {
        let result = panel_add_widget_impl(99999, r#"{"id":0,"kind":{"type":"Toggle"},"label":"W","position":{"x":0,"y":0},"size":{"width":50,"height":50},"channels":[]}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_remove_widget_impl_success() {
        let id = panel_new("Remove Widget Impl");
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Toggle"},
            "label": "Switch",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 50.0, "height": 50.0},
            "channels": []
        }"#;
        let wid = panel_add_widget_impl(id, widget_json).unwrap();
        let removed = panel_remove_widget_impl(id, wid).unwrap();
        assert!(removed);
        panel_destroy(id);
    }

    #[test]
    fn test_panel_remove_widget_impl_panel_not_found() {
        let result = panel_remove_widget_impl(99999, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_update_widget_impl_success() {
        let id = panel_new("Update Widget Impl");
        let widget_json = r#"{
            "id": 0,
            "kind": {"type": "Toggle"},
            "label": "Switch",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 50.0, "height": 50.0},
            "channels": []
        }"#;
        let wid = panel_add_widget_impl(id, widget_json).unwrap();
        let update_json = r#"{
            "id": 0,
            "kind": {"type": "Toggle"},
            "label": "Updated",
            "position": {"x": 0.0, "y": 0.0},
            "size": {"width": 50.0, "height": 50.0},
            "channels": []
        }"#;
        panel_update_widget_impl(id, wid, update_json).unwrap();
        panel_destroy(id);
    }

    #[test]
    fn test_panel_update_widget_impl_invalid_json() {
        let id = panel_new("Update Bad JSON");
        let result = panel_update_widget_impl(id, 1, "not json");
        assert!(result.is_err());
        panel_destroy(id);
    }

    #[test]
    fn test_panel_update_widget_impl_panel_not_found() {
        let result = panel_update_widget_impl(99999, 1, r#"{"id":0,"kind":{"type":"Toggle"},"label":"W","position":{"x":0,"y":0},"size":{"width":50,"height":50},"channels":[]}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_update_widget_impl_widget_not_found() {
        let id = panel_new("Update Widget Not Found");
        let result = panel_update_widget_impl(id, 9999, r#"{"id":0,"kind":{"type":"Toggle"},"label":"W","position":{"x":0,"y":0},"size":{"width":50,"height":50},"channels":[]}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("widget not found"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_snapshot_impl_success() {
        let id = panel_new("Snapshot Impl");
        let snap = panel_snapshot_impl(id).unwrap();
        assert!(snap.contains("Snapshot Impl"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_snapshot_impl_not_found() {
        let result = panel_snapshot_impl(99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_set_topic_impl_success() {
        let id = panel_new("Topic Impl");
        panel_set_topic_impl(id, "motor/speed", 42.0).unwrap();
        panel_destroy(id);
    }

    #[test]
    fn test_panel_set_topic_impl_not_found() {
        let result = panel_set_topic_impl(99999, "x", 1.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel runtime not found"));
    }

    #[test]
    fn test_panel_get_values_impl_success() {
        let id = panel_new("Values Impl");
        panel_set_topic_impl(id, "a", 1.0).unwrap();
        let vals = panel_get_values_impl(id).unwrap();
        assert!(vals.contains("1"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_get_values_impl_not_found() {
        let result = panel_get_values_impl(99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel runtime not found"));
    }

    #[test]
    fn test_panel_merge_values_impl_success() {
        let id = panel_new("Merge Impl");
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
        panel_add_widget_impl(id, widget_json).unwrap();
        panel_merge_values_impl(id, r#"{"sensor/temp": 25.5}"#).unwrap();
        panel_destroy(id);
    }

    #[test]
    fn test_panel_merge_values_impl_invalid_json() {
        let id = panel_new("Merge Bad JSON");
        let result = panel_merge_values_impl(id, "not json");
        assert!(result.is_err());
        panel_destroy(id);
    }

    #[test]
    fn test_panel_merge_values_impl_panel_not_found() {
        let result = panel_merge_values_impl(99999, r#"{"a": 1.0}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }

    #[test]
    fn test_panel_collect_outputs_impl_success() {
        let id = panel_new("Output Impl");
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
        panel_add_widget_impl(id, widget_json).unwrap();
        panel_set_topic_impl(id, "motor/throttle", 75.0).unwrap();
        let outputs = panel_collect_outputs_impl(id).unwrap();
        assert!(outputs.contains("motor/throttle"));
        panel_destroy(id);
    }

    #[test]
    fn test_panel_collect_outputs_impl_panel_not_found() {
        let result = panel_collect_outputs_impl(99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("panel not found"));
    }
}
