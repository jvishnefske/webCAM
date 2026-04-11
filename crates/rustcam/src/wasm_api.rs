//! WASM API wrappers for the rustcam crate.
//!
//! This module re-exports all public functions with `#[wasm_bindgen]` attributes.
//! It is conditionally compiled only for `target_arch = "wasm32"`, keeping the
//! auto-generated wasm-bindgen glue code out of native test/coverage builds.

use js_sys::Function;
use wasm_bindgen::prelude::*;

// ── Machine profiles and config ────────────────────────────────────────

#[wasm_bindgen]
pub fn available_profiles() -> String {
    super::available_profiles()
}

#[wasm_bindgen]
pub fn default_config(machine_type: &str) -> String {
    super::default_config(machine_type)
}

// ── CAM processing ─────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn process_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    super::process_stl(data, config_json)
}

#[wasm_bindgen]
pub fn process_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    super::process_svg(svg_text, config_json)
}

#[wasm_bindgen]
pub fn process_stl_progress(
    data: &[u8],
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    super::process_stl_progress(data, config_json, on_progress)
}

#[wasm_bindgen]
pub fn process_svg_progress(
    svg_text: &str,
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    super::process_svg_progress(svg_text, config_json, on_progress)
}

// ── Preview ────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn preview_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    super::preview_stl(data, config_json)
}

#[wasm_bindgen]
pub fn preview_svg(svg_text: &str) -> Result<String, JsValue> {
    super::preview_svg(svg_text)
}

// ── Simulation data ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn sim_moves_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    super::sim_moves_stl(data, config_json)
}

#[wasm_bindgen]
pub fn sim_moves_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    super::sim_moves_svg(svg_text, config_json)
}

// ── Sketch actor ───────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn sketch_reset() {
    super::sketch_reset()
}

#[wasm_bindgen]
pub fn sketch_add_point(x: f64, y: f64) -> String {
    super::sketch_add_point(x, y)
}

#[wasm_bindgen]
pub fn sketch_add_fixed_point(x: f64, y: f64) -> String {
    super::sketch_add_fixed_point(x, y)
}

#[wasm_bindgen]
pub fn sketch_move_point(id: u32, x: f64, y: f64) {
    super::sketch_move_point(id, x, y)
}

#[wasm_bindgen]
pub fn sketch_remove_point(id: u32) {
    super::sketch_remove_point(id)
}

#[wasm_bindgen]
pub fn sketch_set_fixed(id: u32, fixed: bool) {
    super::sketch_set_fixed(id, fixed)
}

#[wasm_bindgen]
pub fn sketch_add_constraint(
    kind: &str,
    ids_json: &str,
    value: f64,
    value2: f64,
) -> Result<String, JsValue> {
    super::sketch_add_constraint(kind, ids_json, value, value2)
}

#[wasm_bindgen]
pub fn sketch_remove_constraint(id: u32) {
    super::sketch_remove_constraint(id)
}

#[wasm_bindgen]
pub fn sketch_solve() -> Result<String, JsValue> {
    super::sketch_solve()
}

#[wasm_bindgen]
pub fn sketch_pump() -> Result<String, JsValue> {
    super::sketch_pump()
}

#[wasm_bindgen]
pub fn sketch_snapshot() -> Result<String, JsValue> {
    super::sketch_snapshot()
}
