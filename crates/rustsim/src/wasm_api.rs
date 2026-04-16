//! WASM API wrappers for the rustsim crate.
//!
//! This module re-exports all public functions with `#[wasm_bindgen]` attributes.
//! It is conditionally compiled only for `target_arch = "wasm32"`, keeping the
//! auto-generated wasm-bindgen glue code out of native test/coverage builds.

use wasm_bindgen::prelude::*;

use super::dag_api::DagHandle;

// ── DAG handle WASM API ──────────────────────────────────────────────

#[wasm_bindgen]
pub struct WasmDagHandle {
    inner: DagHandle,
}

#[wasm_bindgen]
impl WasmDagHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: DagHandle::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn constant(&mut self, value: f64) -> Result<u16, JsValue> {
        self.inner
            .constant(value)
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn input(&mut self, name: &str) -> Result<u16, JsValue> {
        self.inner.input(name).map_err(|e| JsValue::from_str(&e))
    }
    pub fn output(&mut self, name: &str, src: u16) -> Result<u16, JsValue> {
        self.inner
            .output(name, src)
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn add(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.inner.add(a, b).map_err(|e| JsValue::from_str(&e))
    }
    pub fn mul(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.inner.mul(a, b).map_err(|e| JsValue::from_str(&e))
    }
    pub fn sub(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.inner.sub(a, b).map_err(|e| JsValue::from_str(&e))
    }
    pub fn div(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.inner.div(a, b).map_err(|e| JsValue::from_str(&e))
    }
    pub fn pow(&mut self, base: u16, exp: u16) -> Result<u16, JsValue> {
        self.inner.pow(base, exp).map_err(|e| JsValue::from_str(&e))
    }
    pub fn neg(&mut self, a: u16) -> Result<u16, JsValue> {
        self.inner.neg(a).map_err(|e| JsValue::from_str(&e))
    }
    pub fn relu(&mut self, a: u16) -> Result<u16, JsValue> {
        self.inner.relu(a).map_err(|e| JsValue::from_str(&e))
    }
    pub fn subscribe(&mut self, topic: &str) -> Result<u16, JsValue> {
        self.inner
            .subscribe(topic)
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn publish(&mut self, topic: &str, src: u16) -> Result<u16, JsValue> {
        self.inner
            .publish(topic, src)
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn evaluate(&self) -> Vec<f64> {
        self.inner.evaluate()
    }
    pub fn evaluate_node(&self, node_id: u16) -> f64 {
        self.inner.evaluate_node(node_id)
    }
    pub fn to_cbor(&self) -> Vec<u8> {
        self.inner.to_cbor()
    }
    pub fn from_cbor(bytes: &[u8]) -> Result<WasmDagHandle, JsValue> {
        DagHandle::from_cbor(bytes)
            .map(|inner| WasmDagHandle { inner })
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn to_json(&self) -> String {
        self.inner.to_json_impl()
    }
}

// ── Dataflow graph lifecycle ───────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_new(dt: f64) -> u32 {
    super::dataflow_new(dt)
}

#[wasm_bindgen]
pub fn dataflow_destroy(graph_id: u32) {
    super::dataflow_destroy(graph_id)
}

#[wasm_bindgen]
pub fn dataflow_add_block(
    graph_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<u32, JsValue> {
    super::add_block_impl(graph_id, block_type, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_remove_block(graph_id: u32, block_id: u32) -> Result<(), JsValue> {
    super::remove_block_impl(graph_id, block_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_update_block(
    graph_id: u32,
    block_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<(), JsValue> {
    super::update_block_impl(graph_id, block_id, block_type, config_json)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_connect(
    graph_id: u32,
    from_block: u32,
    from_port: u32,
    to_block: u32,
    to_port: u32,
) -> Result<u32, JsValue> {
    super::connect_impl(graph_id, from_block, from_port, to_block, to_port)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_disconnect(graph_id: u32, channel_id: u32) -> Result<(), JsValue> {
    super::disconnect_impl(graph_id, channel_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_advance(graph_id: u32, elapsed: f64) -> Result<JsValue, JsValue> {
    let snap = super::advance_impl(graph_id, elapsed).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn dataflow_run(graph_id: u32, steps: u32, dt: f64) -> Result<JsValue, JsValue> {
    let snap = super::run_impl(graph_id, steps, dt).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn dataflow_set_speed(graph_id: u32, speed: f64) -> Result<(), JsValue> {
    super::set_speed_impl(graph_id, speed).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_snapshot(graph_id: u32) -> Result<JsValue, JsValue> {
    let snap = super::snapshot_impl(graph_id).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn dataflow_block_types() -> JsValue {
    let types = super::block_types_impl();
    serde_wasm_bindgen::to_value(&types).unwrap_or(JsValue::NULL)
}

// ── Function def and MCU schema APIs ───────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_function_defs() -> Result<JsValue, JsValue> {
    let defs = super::function_defs_list();
    serde_wasm_bindgen::to_value(&defs).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn mcu_families() -> JsValue {
    let families = super::mcu_families_list();
    serde_wasm_bindgen::to_value(&families).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn mcu_definition(family: &str) -> Result<JsValue, JsValue> {
    let mcu = super::get_mcu_definition(family).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&mcu).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn mcu_pins(family: &str) -> Result<JsValue, JsValue> {
    let pins = super::get_mcu_pins(family).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&pins).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn mcu_peripherals(family: &str) -> Result<JsValue, JsValue> {
    let periphs = super::get_mcu_peripherals(family).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&periphs).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── Code generation ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_codegen(graph_id: u32, dt: f64) -> Result<String, JsValue> {
    super::codegen_impl(graph_id, dt).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_codegen_multi(
    graph_id: u32,
    dt: f64,
    targets_json: &str,
) -> Result<String, JsValue> {
    super::codegen_multi_impl(graph_id, dt, targets_json).map_err(|e| JsValue::from_str(&e))
}

// ── Simulation mode ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_set_simulation_mode(graph_id: u32, enabled: bool) -> Result<(), JsValue> {
    super::set_simulation_mode_impl(graph_id, enabled).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_set_sim_adc(graph_id: u32, channel: u8, voltage: f64) -> Result<(), JsValue> {
    super::set_sim_adc_impl(graph_id, channel, voltage).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_get_sim_pwm(graph_id: u32, channel: u8) -> Result<f64, JsValue> {
    super::get_sim_pwm_impl(graph_id, channel).map_err(|e| JsValue::from_str(&e))
}

// ── I2C simulation ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_add_i2c_device(
    graph_id: u32,
    bus: u8,
    addr: u8,
    name: &str,
) -> Result<(), JsValue> {
    super::add_i2c_device_impl(graph_id, bus, addr, name).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn dataflow_remove_i2c_device(graph_id: u32, bus: u8, addr: u8) -> Result<(), JsValue> {
    super::remove_i2c_device_impl(graph_id, bus, addr).map_err(|e| JsValue::from_str(&e))
}

// ── Serial simulation ─────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_configure_serial(
    graph_id: u32,
    port: u8,
    baud: u32,
    data_bits: u8,
    parity: u8,
    stop_bits: u8,
) -> Result<(), JsValue> {
    super::configure_serial_impl(graph_id, port, baud, data_bits, parity, stop_bits)
        .map_err(|e| JsValue::from_str(&e))
}

// ── TCP socket simulation ─────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_tcp_inject(graph_id: u32, socket_id: u8, data: &[u8]) -> Result<(), JsValue> {
    super::tcp_inject_impl(graph_id, socket_id, data).map_err(|e| JsValue::from_str(&e))
}

// ── Control panel ─────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn panel_new(name: &str) -> u32 {
    super::panel_new(name)
}

#[wasm_bindgen]
pub fn panel_destroy(panel_id: u32) {
    super::panel_destroy(panel_id)
}

#[wasm_bindgen]
pub fn panel_load(json: &str) -> Result<u32, JsValue> {
    super::panel_load_impl(json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_save(panel_id: u32) -> Result<String, JsValue> {
    super::panel_save_impl(panel_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_add_widget(panel_id: u32, config_json: &str) -> Result<u32, JsValue> {
    super::panel_add_widget_impl(panel_id, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_remove_widget(panel_id: u32, widget_id: u32) -> Result<bool, JsValue> {
    super::panel_remove_widget_impl(panel_id, widget_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_update_widget(
    panel_id: u32,
    widget_id: u32,
    config_json: &str,
) -> Result<(), JsValue> {
    super::panel_update_widget_impl(panel_id, widget_id, config_json)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_snapshot(panel_id: u32) -> Result<String, JsValue> {
    super::panel_snapshot_impl(panel_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_set_topic(panel_id: u32, topic: &str, value: f64) -> Result<(), JsValue> {
    super::panel_set_topic_impl(panel_id, topic, value).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_get_values(panel_id: u32) -> Result<String, JsValue> {
    super::panel_get_values_impl(panel_id).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_merge_values(panel_id: u32, values_json: &str) -> Result<(), JsValue> {
    super::panel_merge_values_impl(panel_id, values_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn panel_collect_outputs(panel_id: u32) -> Result<String, JsValue> {
    super::panel_collect_outputs_impl(panel_id).map_err(|e| JsValue::from_str(&e))
}
