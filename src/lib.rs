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
pub mod gcode_parser;
pub mod geometry;
pub mod machine;
pub mod sketch_actor;
pub mod slicer;
pub mod stl;
pub mod svg;
pub mod tool;
pub mod toolpath;
pub mod units;

use gcode::{emit_gcode, emit_gcode_with_profile, GcodeParams, LaserParams};
use geometry::Toolpath;
use js_sys::Function;
use machine::{MachineProfile, MachineType};
use serde::{Deserialize, Serialize};
use tool::Tool;
use toolpath::{
    ContourStrategy, CutParams, LaserCutStrategy, LaserEngraveStrategy, PerimeterStrategy,
    PocketStrategy, ScanDirection, SurfaceParams, ToolpathStrategy, ZigzagSurfaceStrategy,
};
use wasm_bindgen::prelude::*;

// ── Public parameter struct (JSON from JS) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamConfig {
    #[serde(default = "default_tool_diameter")]
    pub tool_diameter: f64,
    #[serde(default = "default_tool_type")]
    pub tool_type: String,
    #[serde(default)]
    pub corner_radius: f64,
    #[serde(default)]
    pub effective_diameter: Option<f64>,
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
    #[serde(default)]
    pub climb_cut: bool,
    #[serde(default = "default_perimeter_passes")]
    pub perimeter_passes: u32,
    #[serde(default = "default_scan_direction")]
    pub scan_direction: String,
    #[serde(default = "default_machine_type")]
    pub machine_type: String,
    #[serde(default)]
    pub laser_power: Option<f64>,
    #[serde(default)]
    pub passes: Option<u32>,
    #[serde(default)]
    pub air_assist: Option<bool>,
}

fn default_tool_diameter() -> f64 {
    3.175
}
fn default_tool_type() -> String {
    "end_mill".into()
}
fn default_perimeter_passes() -> u32 {
    1
}
fn default_step_over() -> f64 {
    1.5
}
fn default_step_down() -> f64 {
    1.0
}
fn default_feed_rate() -> f64 {
    800.0
}
fn default_plunge_rate() -> f64 {
    300.0
}
fn default_spindle_speed() -> f64 {
    12000.0
}
fn default_safe_z() -> f64 {
    5.0
}
fn default_cut_depth() -> f64 {
    -1.0
}
fn default_scan_direction() -> String {
    "x".into()
}
fn default_strategy() -> String {
    "contour".into()
}
fn default_machine_type() -> String {
    "cnc_mill".into()
}

impl Default for CamConfig {
    fn default() -> Self {
        Self {
            tool_diameter: default_tool_diameter(),
            tool_type: default_tool_type(),
            corner_radius: 0.0,
            effective_diameter: None,
            step_over: default_step_over(),
            step_down: default_step_down(),
            feed_rate: default_feed_rate(),
            plunge_rate: default_plunge_rate(),
            spindle_speed: default_spindle_speed(),
            safe_z: default_safe_z(),
            cut_depth: default_cut_depth(),
            strategy: default_strategy(),
            climb_cut: false,
            perimeter_passes: default_perimeter_passes(),
            scan_direction: default_scan_direction(),
            machine_type: default_machine_type(),
            laser_power: None,
            passes: None,
            air_assist: None,
        }
    }
}

/// Parse scan direction from config string.
fn scan_direction_from_config(config: &CamConfig) -> ScanDirection {
    match config.scan_direction.as_str() {
        "y" | "Y" => ScanDirection::Y,
        _ => ScanDirection::X,
    }
}

/// Create a Tool from CamConfig fields.
fn tool_from_config(config: &CamConfig) -> Tool {
    match config.tool_type.as_str() {
        "ball_end" => Tool::ball_end(config.tool_diameter, 10.0),
        "face_mill" => Tool::face_mill(
            config.tool_diameter,
            config.effective_diameter.unwrap_or(config.tool_diameter),
            10.0,
        ),
        _ => Tool::new(
            tool::ToolType::EndMill,
            config.tool_diameter,
            10.0,
            config.corner_radius,
        ),
    }
}

/// Resolve a MachineProfile from the config's machine_type field.
fn profile_from_config(config: &CamConfig) -> MachineProfile {
    match config.machine_type.as_str() {
        "laser_cutter" => MachineProfile::laser_cutter(),
        _ => MachineProfile::cnc_mill(),
    }
}

/// Build LaserParams from config, if applicable.
fn laser_params_from_config(config: &CamConfig) -> Option<LaserParams> {
    if config.machine_type == "laser_cutter" {
        Some(LaserParams {
            power: config.laser_power.unwrap_or(100.0),
            passes: config.passes.unwrap_or(1),
            air_assist: config.air_assist.unwrap_or(false),
        })
    } else {
        None
    }
}

/// Select the right strategy based on config and profile.
fn strategy_from_config(config: &CamConfig) -> Box<dyn ToolpathStrategy> {
    match config.strategy.as_str() {
        "pocket" => Box::new(PocketStrategy),
        "perimeter" => Box::new(PerimeterStrategy),
        "laser_cut" => Box::new(LaserCutStrategy::new(config.laser_power.unwrap_or(100.0))),
        "laser_engrave" => Box::new(LaserEngraveStrategy::new(
            config.laser_power.unwrap_or(100.0),
            config.step_over,
        )),
        _ => Box::new(ContourStrategy),
    }
}

// ── New WASM API ─────────────────────────────────────────────────────

/// Return JSON list of available machine profiles.
#[wasm_bindgen]
pub fn available_profiles() -> String {
    let profiles = vec![MachineProfile::cnc_mill(), MachineProfile::laser_cutter()];
    serde_json::to_string(&profiles).unwrap_or_else(|_| "[]".into())
}

/// Return a default config JSON for the given machine type.
#[wasm_bindgen]
pub fn default_config(machine_type: &str) -> String {
    let config = if machine_type == "laser_cutter" {
        CamConfig {
            machine_type: machine_type.into(),
            strategy: "laser_cut".into(),
            laser_power: Some(100.0),
            passes: Some(1),
            ..CamConfig::default()
        }
    } else {
        CamConfig {
            machine_type: machine_type.into(),
            ..CamConfig::default()
        }
    };
    serde_json::to_string(&config).unwrap_or_else(|_| "{}".into())
}

// ── WASM entry points ────────────────────────────────────────────────

/// Process an STL file (binary bytes) and return G-code.
#[wasm_bindgen]
pub fn process_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let profile = profile_from_config(&config);
    profile
        .validate_strategy(&config.strategy)
        .map_err(|e| JsValue::from_str(&e))?;

    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool: tool_from_config(&config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
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
        "zigzag" => {
            // 3D surface zigzag raster
            let strategy = ZigzagSurfaceStrategy;
            let surface_params =
                SurfaceParams::new(&mesh, cut_params, scan_direction_from_config(&config));
            strategy.generate_surface(&surface_params)
        }
        "perimeter" => {
            // Perimeter-follow each slice layer
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let strategy = PerimeterStrategy;
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
                let contours =
                    slicer::slice_at_z(&mesh, mesh.bounds.as_ref().map_or(0.0, |b| b.min.z + 0.01));
                all.extend(strategy.generate(&contours, &cut_params));
            }
            all
        }
    };

    let laser = laser_params_from_config(&config);
    Ok(emit_gcode_with_profile(
        &toolpaths,
        &gcode_params,
        &profile,
        laser.as_ref(),
    ))
}

/// Process an SVG string and return G-code.
#[wasm_bindgen]
pub fn process_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let profile = profile_from_config(&config);
    profile
        .validate_strategy(&config.strategy)
        .map_err(|e| JsValue::from_str(&e))?;

    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool: tool_from_config(&config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
    };

    let gcode_params = GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let strategy = strategy_from_config(&config);

    // Laser strategies run single pass at Z=0
    let is_laser = profile.machine_type == MachineType::LaserCutter;
    let mut all_toolpaths = Vec::new();

    if is_laser {
        all_toolpaths.extend(strategy.generate(&polylines, &cut_params));
    } else {
        // For 2-D SVG, step down from 0 to cut_depth
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
    }

    let laser = laser_params_from_config(&config);
    Ok(emit_gcode_with_profile(
        &all_toolpaths,
        &gcode_params,
        &profile,
        laser.as_ref(),
    ))
}

/// Helper: call a JS progress callback with (completed, total).
fn report_progress(cb: &Function, completed: u32, total: u32) {
    let _ = cb.call2(
        &JsValue::NULL,
        &JsValue::from(completed),
        &JsValue::from(total),
    );
}

/// Process an STL file with progress reporting.
/// The callback receives (completed_layers, total_layers) after each layer.
#[wasm_bindgen]
pub fn process_stl_progress(
    data: &[u8],
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool: tool_from_config(&config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
    };

    let gcode_params = GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let toolpaths: Vec<Toolpath> = match config.strategy.as_str() {
        "zigzag" => {
            report_progress(on_progress, 0, 1);
            let strategy = ZigzagSurfaceStrategy;
            let surface_params =
                SurfaceParams::new(&mesh, cut_params, scan_direction_from_config(&config));
            let result = strategy.generate_surface(&surface_params);
            report_progress(on_progress, 1, 1);
            result
        }
        other => {
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let total = layers.len() as u32;
            report_progress(on_progress, 0, total);
            let strategy: Box<dyn ToolpathStrategy> = match other {
                "pocket" => Box::new(PocketStrategy),
                "perimeter" => Box::new(PerimeterStrategy),
                "slice" => Box::new(ContourStrategy),
                _ => Box::new(ContourStrategy),
            };
            let mut all = Vec::new();
            for (i, (z, contours)) in layers.iter().enumerate() {
                let mut p = cut_params.clone();
                p.cut_z = *z;
                all.extend(strategy.generate(contours, &p));
                report_progress(on_progress, (i + 1) as u32, total);
            }
            if all.is_empty() && other != "pocket" && other != "perimeter" {
                let contours =
                    slicer::slice_at_z(&mesh, mesh.bounds.as_ref().map_or(0.0, |b| b.min.z + 0.01));
                all.extend(strategy.generate(&contours, &cut_params));
            }
            all
        }
    };

    Ok(emit_gcode(&toolpaths, &gcode_params))
}

/// Process an SVG string with progress reporting.
/// The callback receives (completed_layers, total_layers) after each layer.
#[wasm_bindgen]
pub fn process_svg_progress(
    svg_text: &str,
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = CutParams {
        tool: tool_from_config(&config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
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
        "perimeter" => Box::new(PerimeterStrategy),
        _ => Box::new(ContourStrategy),
    };

    // Count total layers
    let mut total_layers = 0u32;
    {
        let mut z = 0.0;
        while z > config.cut_depth - 0.001 {
            z -= config.step_down;
            if z < config.cut_depth {
                z = config.cut_depth;
            }
            total_layers += 1;
            if (z - config.cut_depth).abs() < 0.001 {
                break;
            }
        }
    }
    report_progress(on_progress, 0, total_layers);

    let mut all_toolpaths = Vec::new();
    let mut z = 0.0;
    let mut layer_num = 0u32;
    while z > config.cut_depth - 0.001 {
        z -= config.step_down;
        if z < config.cut_depth {
            z = config.cut_depth;
        }
        let mut p = cut_params.clone();
        p.cut_z = z;
        all_toolpaths.extend(strategy.generate(&polylines, &p));
        layer_num += 1;
        report_progress(on_progress, layer_num, total_layers);
        if (z - config.cut_depth).abs() < 0.001 {
            break;
        }
    }

    Ok(emit_gcode(&all_toolpaths, &gcode_params))
}

/// Return toolpath data as JSON (for the 2-D preview canvas).
/// Returns toolpath moves with Z coordinates for 3D visualization.
#[wasm_bindgen]
pub fn preview_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;
    let toolpaths = build_toolpaths_stl(&mesh, &config);

    // Convert toolpaths to preview format with Z coordinates
    let mut preview_paths: Vec<Vec<[f64; 3]>> = Vec::new();
    for tp in &toolpaths {
        let path: Vec<[f64; 3]> = tp
            .moves
            .iter()
            .filter(|m| !m.rapid) // Only show cutting moves
            .map(|m| [m.x, m.y, m.z])
            .collect();
        if !path.is_empty() {
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

// ── Simulation data ──────────────────────────────────────────────────

/// Return flat move list as JSON for the tool simulation.
/// Each move: `{ x, y, z, rapid }`.
#[wasm_bindgen]
pub fn sim_moves_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;
    let toolpaths = build_toolpaths_stl(&mesh, &config);
    flatten_moves(&toolpaths)
}

#[wasm_bindgen]
pub fn sim_moves_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;
    let toolpaths = build_toolpaths_svg(&polylines, &config);
    flatten_moves(&toolpaths)
}

fn build_toolpaths_stl(mesh: &geometry::Mesh, config: &CamConfig) -> Vec<Toolpath> {
    let cut_params = CutParams {
        tool: tool_from_config(config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
    };

    // Handle zigzag separately (3D surface strategy)
    if config.strategy == "zigzag" {
        let strategy = ZigzagSurfaceStrategy;
        let surface_params =
            SurfaceParams::new(mesh, cut_params, scan_direction_from_config(config));
        return strategy.generate_surface(&surface_params);
    }

    let layers = slicer::slice_mesh(mesh, config.step_down);
    let strategy: Box<dyn ToolpathStrategy> = match config.strategy.as_str() {
        "pocket" => Box::new(PocketStrategy),
        "perimeter" => Box::new(PerimeterStrategy),
        _ => Box::new(ContourStrategy),
    };
    let mut all = Vec::new();
    for (z, contours) in &layers {
        let mut p = cut_params.clone();
        p.cut_z = *z;
        all.extend(strategy.generate(contours, &p));
    }
    if all.is_empty() {
        let contours =
            slicer::slice_at_z(mesh, mesh.bounds.as_ref().map_or(0.0, |b| b.min.z + 0.01));
        all.extend(strategy.generate(&contours, &cut_params));
    }
    all
}

fn build_toolpaths_svg(polylines: &[geometry::Polyline], config: &CamConfig) -> Vec<Toolpath> {
    let cut_params = CutParams {
        tool: tool_from_config(config),
        tool_diameter: config.tool_diameter,
        step_over: config.step_over,
        step_down: config.step_down,
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        safe_z: config.safe_z,
        cut_z: config.cut_depth,
        climb_cut: config.climb_cut,
        perimeter_passes: config.perimeter_passes,
    };
    let strategy = strategy_from_config(config);
    let is_laser = config.machine_type == "laser_cutter";

    let mut all = Vec::new();
    if is_laser {
        all.extend(strategy.generate(polylines, &cut_params));
    } else {
        let mut z = 0.0;
        while z > config.cut_depth - 0.001 {
            z -= config.step_down;
            if z < config.cut_depth {
                z = config.cut_depth;
            }
            let mut p = cut_params.clone();
            p.cut_z = z;
            all.extend(strategy.generate(polylines, &p));
            if (z - config.cut_depth).abs() < 0.001 {
                break;
            }
        }
    }
    all
}

fn flatten_moves(toolpaths: &[Toolpath]) -> Result<String, JsValue> {
    let moves: Vec<&geometry::ToolpathMove> =
        toolpaths.iter().flat_map(|tp| tp.moves.iter()).collect();
    serde_json::to_string(&moves).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── Sketch Actor WASM API ────────────────────────────────────────────

use std::cell::RefCell;
thread_local! {
    static SKETCH: RefCell<sketch_actor::SketchActor> = RefCell::new(sketch_actor::SketchActor::new());
}

/// Reset the sketch actor to a blank state.
#[wasm_bindgen]
pub fn sketch_reset() {
    SKETCH.with(|s| *s.borrow_mut() = sketch_actor::SketchActor::new());
}

/// Add a free point. Returns JSON `{"id": <u32>}`.
#[wasm_bindgen]
pub fn sketch_add_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Add a fixed point. Returns JSON `{"id": <u32>}`.
#[wasm_bindgen]
pub fn sketch_add_fixed_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point_fixed(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Move a point to new coordinates.
#[wasm_bindgen]
pub fn sketch_move_point(id: u32, x: f64, y: f64) {
    SKETCH.with(|s| s.borrow_mut().move_point(id, x, y));
}

/// Remove a point and all its constraints.
#[wasm_bindgen]
pub fn sketch_remove_point(id: u32) {
    SKETCH.with(|s| s.borrow_mut().remove_point(id));
}

/// Set a point's fixed flag.
#[wasm_bindgen]
pub fn sketch_set_fixed(id: u32, fixed: bool) {
    SKETCH.with(|s| {
        if let Some(p) = s.borrow_mut().points.get_mut(&id) {
            p.fixed = fixed;
        }
    });
}

/// Add a constraint. `kind` is one of: "coincident", "distance",
/// "horizontal", "vertical", "fixed", "angle", "radius",
/// "perpendicular", "parallel", "midpoint", "equal_length", "symmetric".
///
/// `ids` is a JSON array of point ids, `value` is the numeric parameter
/// (distance, angle, radius, x, y — depends on constraint type).
/// For "fixed", pass `value` as x and `value2` as y.
///
/// Returns JSON `{"id": <u32>}`.
#[wasm_bindgen]
pub fn sketch_add_constraint(
    kind: &str,
    ids_json: &str,
    value: f64,
    value2: f64,
) -> Result<String, JsValue> {
    let ids: Vec<u32> =
        serde_json::from_str(ids_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let constraint = match kind {
        "coincident" if ids.len() >= 2 => sketch_actor::Constraint::Coincident(ids[0], ids[1]),
        "distance" if ids.len() >= 2 => sketch_actor::Constraint::Distance(ids[0], ids[1], value),
        "horizontal" if ids.len() >= 2 => sketch_actor::Constraint::Horizontal(ids[0], ids[1]),
        "vertical" if ids.len() >= 2 => sketch_actor::Constraint::Vertical(ids[0], ids[1]),
        "fixed" if !ids.is_empty() => {
            sketch_actor::Constraint::FixedPosition(ids[0], value, value2)
        }
        "angle" if ids.len() >= 2 => sketch_actor::Constraint::Angle(ids[0], ids[1], value),
        "radius" if ids.len() >= 2 => sketch_actor::Constraint::Radius(ids[0], ids[1], value),
        "perpendicular" if ids.len() >= 4 => {
            sketch_actor::Constraint::Perpendicular(ids[0], ids[1], ids[2], ids[3])
        }
        "parallel" if ids.len() >= 4 => {
            sketch_actor::Constraint::Parallel(ids[0], ids[1], ids[2], ids[3])
        }
        "midpoint" if ids.len() >= 3 => {
            sketch_actor::Constraint::Midpoint(ids[0], ids[1], ids[2])
        }
        "equal_length" if ids.len() >= 4 => {
            sketch_actor::Constraint::EqualLength(ids[0], ids[1], ids[2], ids[3])
        }
        "symmetric" if ids.len() >= 4 => {
            sketch_actor::Constraint::Symmetric(ids[0], ids[1], ids[2], ids[3])
        }
        _ => {
            return Err(JsValue::from_str(&format!(
                "Unknown constraint '{kind}' or wrong number of ids"
            )));
        }
    };

    SKETCH.with(|s| {
        let id = s.borrow_mut().add_constraint(constraint);
        Ok(format!(r#"{{"id":{id}}}"#))
    })
}

/// Remove a constraint by id.
#[wasm_bindgen]
pub fn sketch_remove_constraint(id: u32) {
    SKETCH.with(|s| {
        s.borrow_mut().constraints.remove(&id);
    });
}

/// Run the constraint solver and return a full snapshot as JSON.
/// The snapshot includes points, constraints, DOF, solve status,
/// and per-point coloring status.
#[wasm_bindgen]
pub fn sketch_solve() -> Result<String, JsValue> {
    SKETCH.with(|s| {
        let mut actor = s.borrow_mut();
        actor.solve(200);
        let snap = actor.snapshot();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Process queued messages and return snapshot JSON.
#[wasm_bindgen]
pub fn sketch_pump() -> Result<String, JsValue> {
    SKETCH.with(|s| {
        let mut actor = s.borrow_mut();
        let (_last_id, snap) = actor.pump();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Get current snapshot without solving (read-only query).
#[wasm_bindgen]
pub fn sketch_snapshot() -> Result<String, JsValue> {
    SKETCH.with(|s| {
        let snap = s.borrow().snapshot();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_req_003_config_default_tool_type() {
        let config = CamConfig::default();
        assert_eq!(config.tool_type, "end_mill");
    }

    #[test]
    fn test_req_003_config_parses_tool_type() {
        let json = r#"{"tool_type": "ball_end", "tool_diameter": 6.0}"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.tool_type, "ball_end");
    }

    #[test]
    fn test_req_003_config_parses_corner_radius() {
        let json = r#"{"corner_radius": 0.5}"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert!((config.corner_radius - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_req_003_config_parses_effective_diameter() {
        let json =
            r#"{"tool_type": "face_mill", "tool_diameter": 50.0, "effective_diameter": 40.0}"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.effective_diameter, Some(40.0));
    }

    #[test]
    fn test_req_003_backward_compat_no_tool_type() {
        // Existing configs without tool_type should still work
        let json = r#"{"tool_diameter": 3.175, "feed_rate": 800}"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.tool_type, "end_mill");
        assert!((config.tool_diameter - 3.175).abs() < 0.001);
    }

    #[test]
    fn test_req_003_tool_from_config_end_mill() {
        let config = CamConfig::default();
        let tool = tool_from_config(&config);
        assert_eq!(tool.tool_type, tool::ToolType::EndMill);
    }

    #[test]
    fn test_req_003_tool_from_config_ball_end() {
        let config = CamConfig {
            tool_type: "ball_end".into(),
            tool_diameter: 6.0,
            ..CamConfig::default()
        };
        let tool = tool_from_config(&config);
        assert_eq!(tool.tool_type, tool::ToolType::BallEnd);
        assert!((tool.corner_radius - 3.0).abs() < 0.001); // radius = diameter/2
    }

    #[test]
    fn test_req_003_tool_from_config_face_mill() {
        let config = CamConfig {
            tool_type: "face_mill".into(),
            tool_diameter: 50.0,
            effective_diameter: Some(40.0),
            ..CamConfig::default()
        };
        let tool = tool_from_config(&config);
        assert!((tool.effective_diameter() - 40.0).abs() < 0.001);
    }

    // ── Machine profile integration tests ─────────────────────────────

    #[test]
    fn test_config_machine_type_default() {
        let config = CamConfig::default();
        assert_eq!(config.machine_type, "cnc_mill");
    }

    #[test]
    fn test_config_parses_laser() {
        let json = r#"{"machine_type": "laser_cutter", "laser_power": 80, "passes": 3}"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.machine_type, "laser_cutter");
        assert_eq!(config.laser_power, Some(80.0));
        assert_eq!(config.passes, Some(3));
    }

    #[test]
    fn test_profile_from_config_cnc() {
        let config = CamConfig::default();
        let profile = profile_from_config(&config);
        assert_eq!(profile.machine_type, MachineType::CncMill);
    }

    #[test]
    fn test_profile_from_config_laser() {
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            ..CamConfig::default()
        };
        let profile = profile_from_config(&config);
        assert_eq!(profile.machine_type, MachineType::LaserCutter);
    }

    #[test]
    fn test_laser_rejects_stl_3d_strategy() {
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "zigzag".into(),
            ..CamConfig::default()
        };
        let profile = profile_from_config(&config);
        assert!(profile.validate_strategy(&config.strategy).is_err());
    }

    #[test]
    fn test_laser_rejects_slice_strategy() {
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "slice".into(),
            ..CamConfig::default()
        };
        let profile = profile_from_config(&config);
        assert!(profile.validate_strategy(&config.strategy).is_err());
    }

    #[test]
    fn test_svg_laser_cut_produces_gcode() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#;
        let config_json =
            r#"{"machine_type": "laser_cutter", "strategy": "laser_cut", "laser_power": 80}"#;
        let result = process_svg(svg, config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(gcode.contains("M4 S0"), "Should have laser dynamic mode");
        assert!(gcode.contains("S80"), "Should have power commands");
        assert!(gcode.contains("M5"), "Should turn off laser at end");
    }

    #[test]
    fn test_svg_laser_engrave_produces_scanlines() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#;
        let config_json = r#"{"machine_type": "laser_cutter", "strategy": "laser_engrave", "laser_power": 60, "step_over": 2.0}"#;
        let result = process_svg(svg, config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(gcode.contains("S60"), "Should have engrave power");
    }

    #[test]
    fn test_svg_cnc_mill_still_works() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#;
        let config_json = r#"{"strategy": "contour"}"#;
        let result = process_svg(svg, config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(gcode.contains("M3 S12000"), "Should have spindle on");
    }

    #[test]
    fn test_available_profiles() {
        let json = available_profiles();
        let profiles: Vec<MachineProfile> = serde_json::from_str(&json).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].machine_type, MachineType::CncMill);
        assert_eq!(profiles[1].machine_type, MachineType::LaserCutter);
    }

    #[test]
    fn test_default_config_cnc() {
        let json = default_config("cnc_mill");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.machine_type, "cnc_mill");
        assert_eq!(config.strategy, "contour");
    }

    #[test]
    fn test_default_config_laser() {
        let json = default_config("laser_cutter");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.machine_type, "laser_cutter");
        assert_eq!(config.strategy, "laser_cut");
        assert_eq!(config.laser_power, Some(100.0));
        assert_eq!(config.passes, Some(1));
    }
}
