//! Bridge to the rustcam sketch constraint solver.
//!
//! The rustcam crate exports its sketch API via `wasm_bindgen`.  We import
//! those functions here so the Leptos components can call them at runtime.
//! On non-wasm targets these stubs are never compiled (the entire
//! `components` tree is `cfg(target_arch = "wasm32")`).

use wasm_bindgen::prelude::*;

// ── Imported rustcam WASM functions ────────────────────────────────

#[wasm_bindgen(js_namespace = ["__rustcam"])]
extern "C" {
    /// Reset the sketch actor to a blank state.
    #[wasm_bindgen(js_name = "sketch_reset", catch)]
    fn js_sketch_reset() -> Result<(), JsValue>;

    /// Add a free point. Returns JSON `{"id": <u32>}`.
    #[wasm_bindgen(js_name = "sketch_add_point")]
    fn js_sketch_add_point(x: f64, y: f64) -> String;

    /// Add a constraint. Returns JSON `{"id": <u32>}`.
    #[wasm_bindgen(js_name = "sketch_add_constraint", catch)]
    fn js_sketch_add_constraint(
        kind: &str,
        ids_json: &str,
        value: f64,
        value2: f64,
    ) -> Result<String, JsValue>;

    /// Run the solver. Returns JSON snapshot.
    #[wasm_bindgen(js_name = "sketch_solve", catch)]
    fn js_sketch_solve() -> Result<String, JsValue>;

    /// Remove a constraint by id.
    #[wasm_bindgen(js_name = "sketch_remove_constraint", catch)]
    fn js_sketch_remove_constraint(id: u32) -> Result<(), JsValue>;
}

// ── Snapshot types (deserialized from the solver JSON) ─────────────

use serde::Deserialize;
use std::collections::HashMap;

/// Mirrors `sketch_actor::SketchSnapshot` for deserialization.
#[derive(Debug, Clone, Deserialize)]
pub struct SketchSnapshot {
    pub points: Vec<(u32, SketchPoint)>,
    pub constraints: Vec<(u32, serde_json::Value)>,
    pub dof: i32,
    pub dof_status: String,
    #[serde(default)]
    pub point_status: HashMap<u32, String>,
}

/// Mirrors `sketch_actor::Point`.
#[derive(Debug, Clone, Deserialize)]
pub struct SketchPoint {
    pub x: f64,
    pub y: f64,
    pub fixed: bool,
}

// ── Safe wrappers ──────────────────────────────────────────────────

/// Reset the sketch solver to a blank state.
pub fn reset() {
    let _ = js_sketch_reset();
}

/// Add a free point, returning its solver-assigned id.
pub fn add_point(x: f64, y: f64) -> Option<u32> {
    let json = js_sketch_add_point(x, y);
    parse_id(&json)
}

/// Add a constraint, returning its solver-assigned id.
pub fn add_constraint(kind: &str, ids: &[u32], value: f64, value2: f64) -> Option<u32> {
    let ids_json = serde_json::to_string(ids).ok()?;
    let json = js_sketch_add_constraint(kind, &ids_json, value, value2).ok()?;
    parse_id(&json)
}

/// Run the solver and return the snapshot.
pub fn solve() -> Option<SketchSnapshot> {
    let json = js_sketch_solve().ok()?;
    serde_json::from_str(&json).ok()
}

/// Remove a constraint by id.
pub fn remove_constraint(id: u32) {
    let _ = js_sketch_remove_constraint(id);
}

/// Parse `{"id": N}` from a JSON string.
fn parse_id(json: &str) -> Option<u32> {
    #[derive(Deserialize)]
    struct IdResp {
        id: u32,
    }
    serde_json::from_str::<IdResp>(json).ok().map(|r| r.id)
}
