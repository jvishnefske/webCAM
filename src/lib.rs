//! RustCAM — browser-based CAM in WebAssembly.
//!
//! # Swiss Cheese Architecture
//!
//! The pipeline is composed of independent, swappable layers:
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  Input        │ ──▶ │  Geometry     │ ──▶ │  Strategy     │ ──▶ │  Output       │
//! │  (STL / SVG)  │     │  (Mesh/Paths) │     │  (Contour/    │     │  (G-code)     │
//! │               │     │               │     │   Pocket/     │     │               │
//! │  🧀 hole:     │     │  🧀 hole:     │     │   Slice)      │     │  🧀 hole:     │
//! │  add OBJ,3MF  │     │  add NURBS    │     │  🧀 hole:     │     │  add HPGL,    │
//! │  STEP, DXF…   │     │  T-splines…   │     │  trochoidal,  │     │  Marlin, …    │
//! └──────────────┘     └──────────────┘     │  adaptive…    │     └──────────────┘
//!                                            └──────────────┘
//! ```
//!
//! Each layer is a trait / module boundary. Add new formats or strategies
//! without touching existing code.

pub mod gcode;
pub mod geometry;
pub mod slicer;
pub mod stl;
pub mod svg;
pub mod toolpath;

use gcode::{emit_gcode, GcodeParams};
use geometry::Toolpath;
use serde::{Deserialize, Serialize};
use toolpath::{ContourStrategy, CutParams, PocketStrategy, ToolpathStrategy};
use wasm_bindgen::prelude::*;

// ── Public parameter struct (JSON from JS) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamConfig {
    #[serde(default = "default_tool_diameter")]
    pub tool_diameter: f64,
    #[serde(default = "default_step_over")]
    pub step_over: f64,
    #[serde(default = "default_step_down")]
    pub step_down: f64,
    #[serde(default = "default_feed_rate")]
    pub feed_rate: f64,
    #[serde(default = "default_plunge_rate")]
    pub plunge_rate: f64,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: f64,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    #[serde(default = "default_cut_depth")]
    pub cut_depth: f64,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_tool_diameter() -> f64 { 3.175 }
fn default_step_over() -> f64 { 1.5 }
fn default_step_down() -> f64 { 1.0 }
fn default_feed_rate() -> f64 { 800.0 }
fn default_plunge_rate() -> f64 { 300.0 }
fn default_spindle_speed() -> f64 { 12000.0 }
fn default_safe_z() -> f64 { 5.0 }
fn default_cut_depth() -> f64 { -1.0 }
fn default_strategy() -> String { "contour".into() }

impl Default for CamConfig {
    fn default() -> Self {
        Self {
            tool_diameter: default_tool_diameter(),
            step_over: default_step_over(),
            step_down: default_step_down(),
            feed_rate: default_feed_rate(),
            plunge_rate: default_plunge_rate(),
            spindle_speed: default_spindle_speed(),
            safe_z: default_safe_z(),
            cut_depth: default_cut_depth(),
            strategy: default_strategy(),
        }
    }
}

// ── WASM entry points ────────────────────────────────────────────────

/// Process an STL file (binary bytes) and return G-code.
#[wasm_bindgen]
pub fn process_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
    };

    let gcode_params = GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let toolpaths: Vec<Toolpath> = match config.strategy.as_str() {
        "pocket" => {
            // Slice then pocket each layer
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let strategy = PocketStrategy;
            let mut all = Vec::new();
            for (z, contours) in &layers {
                let mut p = cut_params.clone();
                p.cut_z = *z;
                all.extend(strategy.generate(contours, &p));
            }
            all
        }
        "slice" => {
            // Contour-follow each slice layer
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let strategy = ContourStrategy;
            let mut all = Vec::new();
            for (z, contours) in &layers {
                let mut p = cut_params.clone();
                p.cut_z = *z;
                all.extend(strategy.generate(contours, &p));
            }
            all
        }
        _ => {
            // "contour" — slice then contour each layer
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let strategy = ContourStrategy;
            let mut all = Vec::new();
            for (z, contours) in &layers {
                let mut p = cut_params.clone();
                p.cut_z = *z;
                all.extend(strategy.generate(contours, &p));
            }
            if all.is_empty() {
                // Fallback: treat bottom face as a single contour
                let contours = slicer::slice_at_z(&mesh, mesh.bounds.as_ref().map_or(0.0, |b| b.min.z + 0.01));
                all.extend(strategy.generate(&contours, &cut_params));
            }
            all
        }
    };

    Ok(emit_gcode(&toolpaths, &gcode_params))
}

/// Process an SVG string and return G-code.
#[wasm_bindgen]
pub fn process_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
    };

    let gcode_params = GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let strategy: Box<dyn ToolpathStrategy> = match config.strategy.as_str() {
        "pocket" => Box::new(PocketStrategy),
        _ => Box::new(ContourStrategy),
    };

    // For 2-D SVG, step down from 0 to cut_depth
    let mut all_toolpaths = Vec::new();
    let mut z = 0.0;
    while z > config.cut_depth - 0.001 {
        z -= config.step_down;
        if z < config.cut_depth {
            z = config.cut_depth;
        }
        let mut p = cut_params.clone();
        p.cut_z = z;
        all_toolpaths.extend(strategy.generate(&polylines, &p));
        if (z - config.cut_depth).abs() < 0.001 {
            break;
        }
    }

    Ok(emit_gcode(&all_toolpaths, &gcode_params))
}

/// Return toolpath data as JSON (for the 2-D preview canvas).
#[wasm_bindgen]
pub fn preview_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;
    let layers = slicer::slice_mesh(&mesh, config.step_down);

    // Collect all contour points for preview
    let mut preview_paths: Vec<Vec<[f64; 2]>> = Vec::new();
    for (_z, contours) in &layers {
        for c in contours {
            let path: Vec<[f64; 2]> = c.points.iter().map(|p| [p.x, p.y]).collect();
            preview_paths.push(path);
        }
    }
    serde_json::to_string(&preview_paths).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Return toolpath data from SVG as JSON (for the 2-D preview canvas).
#[wasm_bindgen]
pub fn preview_svg(svg_text: &str) -> Result<String, JsValue> {
    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;
    let preview_paths: Vec<Vec<[f64; 2]>> = polylines
        .iter()
        .map(|pl| pl.points.iter().map(|p| [p.x, p.y]).collect())
        .collect();
    serde_json::to_string(&preview_paths).map_err(|e| JsValue::from_str(&e.to_string()))
}
