//! WASM API wrappers for the rustsim crate.
//!
//! This module re-exports all public functions with `#[wasm_bindgen]` attributes.
//! It is conditionally compiled only for `target_arch = "wasm32"`, keeping the
//! auto-generated wasm-bindgen glue code out of native test/coverage builds.

use wasm_bindgen::prelude::*;

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
    super::dataflow_add_block(graph_id, block_type, config_json)
}

#[wasm_bindgen]
pub fn dataflow_remove_block(graph_id: u32, block_id: u32) -> Result<(), JsValue> {
    super::dataflow_remove_block(graph_id, block_id)
}

#[wasm_bindgen]
pub fn dataflow_update_block(
    graph_id: u32,
    block_id: u32,
    block_type: &str,
    config_json: &str,
) -> Result<(), JsValue> {
    super::dataflow_update_block(graph_id, block_id, block_type, config_json)
}

#[wasm_bindgen]
pub fn dataflow_connect(
    graph_id: u32,
    from_block: u32,
    from_port: u32,
    to_block: u32,
    to_port: u32,
) -> Result<u32, JsValue> {
    super::dataflow_connect(graph_id, from_block, from_port, to_block, to_port)
}

#[wasm_bindgen]
pub fn dataflow_disconnect(graph_id: u32, channel_id: u32) -> Result<(), JsValue> {
    super::dataflow_disconnect(graph_id, channel_id)
}

#[wasm_bindgen]
pub fn dataflow_advance(graph_id: u32, elapsed: f64) -> Result<JsValue, JsValue> {
    super::dataflow_advance(graph_id, elapsed)
}

#[wasm_bindgen]
pub fn dataflow_run(graph_id: u32, steps: u32, dt: f64) -> Result<JsValue, JsValue> {
    super::dataflow_run(graph_id, steps, dt)
}

#[wasm_bindgen]
pub fn dataflow_set_speed(graph_id: u32, speed: f64) -> Result<(), JsValue> {
    super::dataflow_set_speed(graph_id, speed)
}

#[wasm_bindgen]
pub fn dataflow_snapshot(graph_id: u32) -> Result<JsValue, JsValue> {
    super::dataflow_snapshot(graph_id)
}

#[wasm_bindgen]
pub fn dataflow_block_types() -> JsValue {
    super::dataflow_block_types()
}

// ── Function def and MCU schema APIs ───────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_function_defs() -> Result<JsValue, JsValue> {
    super::dataflow_function_defs()
}

#[wasm_bindgen]
pub fn mcu_families() -> JsValue {
    super::mcu_families()
}

#[wasm_bindgen]
pub fn mcu_definition(family: &str) -> Result<JsValue, JsValue> {
    super::mcu_definition(family)
}

#[wasm_bindgen]
pub fn mcu_pins(family: &str) -> Result<JsValue, JsValue> {
    super::mcu_pins(family)
}

#[wasm_bindgen]
pub fn mcu_peripherals(family: &str) -> Result<JsValue, JsValue> {
    super::mcu_peripherals(family)
}

// ── Code generation ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_codegen(graph_id: u32, dt: f64) -> Result<String, JsValue> {
    super::dataflow_codegen(graph_id, dt)
}

#[wasm_bindgen]
pub fn dataflow_codegen_multi(
    graph_id: u32,
    dt: f64,
    targets_json: &str,
) -> Result<String, JsValue> {
    super::dataflow_codegen_multi(graph_id, dt, targets_json)
}

// ── Simulation mode ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_set_simulation_mode(graph_id: u32, enabled: bool) -> Result<(), JsValue> {
    super::dataflow_set_simulation_mode(graph_id, enabled)
}

#[wasm_bindgen]
pub fn dataflow_set_sim_adc(graph_id: u32, channel: u8, voltage: f64) -> Result<(), JsValue> {
    super::dataflow_set_sim_adc(graph_id, channel, voltage)
}

#[wasm_bindgen]
pub fn dataflow_get_sim_pwm(graph_id: u32, channel: u8) -> Result<f64, JsValue> {
    super::dataflow_get_sim_pwm(graph_id, channel)
}

// ── I2C simulation ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_add_i2c_device(
    graph_id: u32,
    bus: u8,
    addr: u8,
    name: &str,
) -> Result<(), JsValue> {
    super::dataflow_add_i2c_device(graph_id, bus, addr, name)
}

#[wasm_bindgen]
pub fn dataflow_remove_i2c_device(graph_id: u32, bus: u8, addr: u8) -> Result<(), JsValue> {
    super::dataflow_remove_i2c_device(graph_id, bus, addr)
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
    super::dataflow_configure_serial(graph_id, port, baud, data_bits, parity, stop_bits)
}

// ── TCP socket simulation ─────────────────────────────────────────────

#[wasm_bindgen]
pub fn dataflow_tcp_inject(graph_id: u32, socket_id: u8, data: &[u8]) -> Result<(), JsValue> {
    super::dataflow_tcp_inject(graph_id, socket_id, data)
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
    super::panel_load(json)
}

#[wasm_bindgen]
pub fn panel_save(panel_id: u32) -> Result<String, JsValue> {
    super::panel_save(panel_id)
}

#[wasm_bindgen]
pub fn panel_add_widget(panel_id: u32, config_json: &str) -> Result<u32, JsValue> {
    super::panel_add_widget(panel_id, config_json)
}

#[wasm_bindgen]
pub fn panel_remove_widget(panel_id: u32, widget_id: u32) -> Result<bool, JsValue> {
    super::panel_remove_widget(panel_id, widget_id)
}

#[wasm_bindgen]
pub fn panel_update_widget(
    panel_id: u32,
    widget_id: u32,
    config_json: &str,
) -> Result<(), JsValue> {
    super::panel_update_widget(panel_id, widget_id, config_json)
}

#[wasm_bindgen]
pub fn panel_snapshot(panel_id: u32) -> Result<String, JsValue> {
    super::panel_snapshot(panel_id)
}

#[wasm_bindgen]
pub fn panel_set_topic(panel_id: u32, topic: &str, value: f64) -> Result<(), JsValue> {
    super::panel_set_topic(panel_id, topic, value)
}

#[wasm_bindgen]
pub fn panel_get_values(panel_id: u32) -> Result<String, JsValue> {
    super::panel_get_values(panel_id)
}

#[wasm_bindgen]
pub fn panel_merge_values(panel_id: u32, values_json: &str) -> Result<(), JsValue> {
    super::panel_merge_values(panel_id, values_json)
}

#[wasm_bindgen]
pub fn panel_collect_outputs(panel_id: u32) -> Result<String, JsValue> {
    super::panel_collect_outputs(panel_id)
}
