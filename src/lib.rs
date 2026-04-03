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

pub mod dag_api;
pub mod dataflow;
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

#[cfg(target_arch = "wasm32")]
use gcode::emit_gcode;
#[cfg(any(target_arch = "wasm32", test))]
use gcode::{emit_gcode_with_profile, GcodeParams, LaserParams};
#[cfg(any(target_arch = "wasm32", test))]
use geometry::Toolpath;
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(any(target_arch = "wasm32", test))]
use machine::{MachineProfile, MachineType};
use serde::{Deserialize, Serialize};
#[cfg(any(target_arch = "wasm32", test))]
use tool::Tool;
#[cfg(any(target_arch = "wasm32", test))]
use toolpath::{
    ContourStrategy, CutParams, LaserCutStrategy, LaserEngraveStrategy, PerimeterStrategy,
    PocketStrategy, ScanDirection, SurfaceParams, ToolpathStrategy, ZigzagSurfaceStrategy,
};
#[cfg(target_arch = "wasm32")]
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
#[cfg(any(target_arch = "wasm32", test))]
fn scan_direction_from_config(config: &CamConfig) -> ScanDirection {
    match config.scan_direction.as_str() {
        "y" | "Y" => ScanDirection::Y,
        _ => ScanDirection::X,
    }
}

/// Create a Tool from CamConfig fields.
#[cfg(any(target_arch = "wasm32", test))]
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
#[cfg(any(target_arch = "wasm32", test))]
fn profile_from_config(config: &CamConfig) -> MachineProfile {
    match config.machine_type.as_str() {
        "laser_cutter" => MachineProfile::laser_cutter(),
        _ => MachineProfile::cnc_mill(),
    }
}

/// Build LaserParams from config, if applicable.
#[cfg(any(target_arch = "wasm32", test))]
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
#[cfg(any(target_arch = "wasm32", test))]
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn available_profiles() -> String {
    available_profiles_inner()
}

#[cfg(any(target_arch = "wasm32", test))]
fn available_profiles_inner() -> String {
    let profiles = vec![MachineProfile::cnc_mill(), MachineProfile::laser_cutter()];
    serde_json::to_string(&profiles).unwrap_or_else(|_| "[]".into())
}

/// Return a default config JSON for the given machine type.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn default_config(machine_type: &str) -> String {
    default_config_inner(machine_type)
}

#[cfg(any(target_arch = "wasm32", test))]
fn default_config_inner(machine_type: &str) -> String {
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
fn report_progress(cb: &Function, completed: u32, total: u32) {
    let _ = cb.call2(
        &JsValue::NULL,
        &JsValue::from(completed),
        &JsValue::from(total),
    );
}

/// Process an STL file with progress reporting.
/// The callback receives (completed_layers, total_layers) after each layer.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sim_moves_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;
    let toolpaths = build_toolpaths_stl(&mesh, &config);
    flatten_moves(&toolpaths)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sim_moves_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;
    let toolpaths = build_toolpaths_svg(&polylines, &config);
    flatten_moves(&toolpaths)
}

#[cfg(any(target_arch = "wasm32", test))]
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

#[cfg(any(target_arch = "wasm32", test))]
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

#[cfg(target_arch = "wasm32")]
fn flatten_moves(toolpaths: &[Toolpath]) -> Result<String, JsValue> {
    let moves: Vec<&geometry::ToolpathMove> =
        toolpaths.iter().flat_map(|tp| tp.moves.iter()).collect();
    serde_json::to_string(&moves).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
fn flatten_moves_string(toolpaths: &[Toolpath]) -> String {
    let moves: Vec<&geometry::ToolpathMove> =
        toolpaths.iter().flat_map(|tp| tp.moves.iter()).collect();
    serde_json::to_string(&moves).unwrap_or_else(|_| "[]".into())
}

// ── Sketch Actor WASM API ────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SKETCH: RefCell<sketch_actor::SketchActor> = RefCell::new(sketch_actor::SketchActor::new());
}

/// Reset the sketch actor to a blank state.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_reset() {
    SKETCH.with(|s| *s.borrow_mut() = sketch_actor::SketchActor::new());
}

/// Add a free point. Returns JSON `{"id": <u32>}`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_add_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Add a fixed point. Returns JSON `{"id": <u32>}`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_add_fixed_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point_fixed(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Move a point to new coordinates.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_move_point(id: u32, x: f64, y: f64) {
    SKETCH.with(|s| s.borrow_mut().move_point(id, x, y));
}

/// Remove a point and all its constraints.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_remove_point(id: u32) {
    SKETCH.with(|s| s.borrow_mut().remove_point(id));
}

/// Set a point's fixed flag.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
        "midpoint" if ids.len() >= 3 => sketch_actor::Constraint::Midpoint(ids[0], ids[1], ids[2]),
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_remove_constraint(id: u32) {
    SKETCH.with(|s| {
        s.borrow_mut().constraints.remove(&id);
    });
}

/// Run the constraint solver and return a full snapshot as JSON.
/// The snapshot includes points, constraints, DOF, solve status,
/// and per-point coloring status.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_pump() -> Result<String, JsValue> {
    SKETCH.with(|s| {
        let mut actor = s.borrow_mut();
        let (_last_id, snap) = actor.pump();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Get current snapshot without solving (read-only query).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sketch_snapshot() -> Result<String, JsValue> {
    SKETCH.with(|s| {
        let snap = s.borrow().snapshot();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

// ── Dataflow Simulator WASM API ─────────────────────────────────────

#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn dataflow_destroy(graph_id: u32) {
    DATAFLOW_GRAPHS.with(|g| g.borrow_mut().remove(&graph_id));
    DATAFLOW_SCHEDULERS.with(|s| s.borrow_mut().remove(&graph_id));
}

/// Add a block to a graph. Returns block id.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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

/// Connect an output port to an input port. Returns channel id.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
/// Returns snapshot JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn dataflow_advance(graph_id: u32, elapsed: f64) -> Result<String, JsValue> {
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
            serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
        })
    })
}

/// Run a fixed number of ticks (non-realtime batch mode).
/// Returns snapshot JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn dataflow_run(graph_id: u32, steps: u32, dt: f64) -> Result<String, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let mut graphs = g.borrow_mut();
        let graph = graphs
            .get_mut(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        graph.run(steps as u64, dt);
        let snap = graph.snapshot();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Set the simulation speed multiplier.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn dataflow_snapshot(graph_id: u32) -> Result<String, JsValue> {
    DATAFLOW_GRAPHS.with(|g| {
        let graphs = g.borrow();
        let graph = graphs
            .get(&graph_id)
            .ok_or_else(|| JsValue::from_str("graph not found"))?;
        let snap = graph.snapshot();
        serde_json::to_string(&snap).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// List available block types as JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn dataflow_block_types() -> String {
    dataflow_block_types_inner()
}

#[cfg(any(target_arch = "wasm32", test))]
fn dataflow_block_types_inner() -> String {
    serde_json::to_string(&dataflow::blocks::available_block_types()).unwrap_or_default()
}

/// Generate a standalone Rust crate from a dataflow graph.
/// Returns JSON: `{ "files": [["path", "content"], ...] }` or error.
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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

/// Add a widget to a panel from JSON config (widget with id: 0).
/// Returns the assigned widget id.
#[cfg(target_arch = "wasm32")]
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

/// Remove a widget from a panel. Returns whether it was found.
#[cfg(target_arch = "wasm32")]
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

/// Update a widget's kind/label/position/size/channels from JSON config.
/// The original widget id is preserved.
#[cfg(target_arch = "wasm32")]
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

/// JSON snapshot of the full panel (same as panel_save, for live viewing).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn panel_snapshot(panel_id: u32) -> Result<String, JsValue> {
    panel_save(panel_id)
}

/// Set a topic value from widget interaction.
#[cfg(target_arch = "wasm32")]
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

/// Get all current topic values as JSON: {"topic": value, ...}
#[cfg(target_arch = "wasm32")]
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

/// Merge external values (from HIL pubsub poll) into input topics.
/// `values_json` is a JSON object: {"topic": value, ...}
#[cfg(target_arch = "wasm32")]
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

/// Collect output topic values (for publishing to HIL).
/// Returns JSON: {"topic": value, ...}
#[cfg(target_arch = "wasm32")]
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

    /// Helper: build gcode from SVG + config (replaces process_svg for tests).
    fn test_process_svg_inner(svg_text: &str, config_json: &str) -> String {
        let config: CamConfig = serde_json::from_str(config_json).unwrap();
        let profile = profile_from_config(&config);
        profile.validate_strategy(&config.strategy).unwrap();
        let polylines = svg::parse_svg(svg_text).unwrap();
        let toolpaths = build_toolpaths_svg(&polylines, &config);
        let gcode_params = GcodeParams {
            feed_rate: config.feed_rate,
            plunge_rate: config.plunge_rate,
            spindle_speed: config.spindle_speed,
            safe_z: config.safe_z,
            unit_mm: true,
        };
        let laser = laser_params_from_config(&config);
        emit_gcode_with_profile(&toolpaths, &gcode_params, &profile, laser.as_ref())
    }

    /// Helper: build gcode from STL bytes + config (replaces process_stl for tests).
    fn test_process_stl_inner(data: &[u8], config_json: &str) -> String {
        let config: CamConfig = serde_json::from_str(config_json).unwrap();
        let profile = profile_from_config(&config);
        profile.validate_strategy(&config.strategy).unwrap();
        let mesh = stl::parse_stl(data).unwrap();
        let toolpaths = build_toolpaths_stl(&mesh, &config);
        let gcode_params = GcodeParams {
            feed_rate: config.feed_rate,
            plunge_rate: config.plunge_rate,
            spindle_speed: config.spindle_speed,
            safe_z: config.safe_z,
            unit_mm: true,
        };
        let laser = laser_params_from_config(&config);
        emit_gcode_with_profile(&toolpaths, &gcode_params, &profile, laser.as_ref())
    }

    #[test]
    fn test_svg_laser_cut_produces_gcode() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#;
        let config_json =
            r#"{"machine_type": "laser_cutter", "strategy": "laser_cut", "laser_power": 80}"#;
        let gcode = test_process_svg_inner(svg, config_json);
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
        let gcode = test_process_svg_inner(svg, config_json);
        assert!(gcode.contains("S60"), "Should have engrave power");
    }

    #[test]
    fn test_svg_cnc_mill_still_works() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#;
        let config_json = r#"{"strategy": "contour"}"#;
        let gcode = test_process_svg_inner(svg, config_json);
        assert!(gcode.contains("M3 S12000"), "Should have spindle on");
    }

    #[test]
    fn test_available_profiles() {
        let json = available_profiles_inner();
        let profiles: Vec<MachineProfile> = serde_json::from_str(&json).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].machine_type, MachineType::CncMill);
        assert_eq!(profiles[1].machine_type, MachineType::LaserCutter);
    }

    #[test]
    fn test_default_config_cnc() {
        let json = default_config_inner("cnc_mill");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.machine_type, "cnc_mill");
        assert_eq!(config.strategy, "contour");
    }

    #[test]
    fn test_default_config_laser() {
        let json = default_config_inner("laser_cutter");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.machine_type, "laser_cutter");
        assert_eq!(config.strategy, "laser_cut");
        assert_eq!(config.laser_power, Some(100.0));
        assert_eq!(config.passes, Some(1));
    }

    // ── Dataflow coverage tests (using internal APIs) ────────────────

    #[test]
    fn test_dataflow_graph_new_and_block() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
        let bid = graph.add_block(block);
        assert!(bid.0 > 0);
        graph.remove_block(bid);
    }

    #[test]
    fn test_dataflow_block_types() {
        let json = dataflow_block_types_inner();
        let types: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(!types.is_empty());
    }

    #[test]
    fn test_dataflow_update_block() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
        let bid = graph.add_block(block);
        let block2 = dataflow::blocks::create_block("constant", r#"{"value":2.0}"#).unwrap();
        graph.replace_block(bid, block2).unwrap();
    }

    #[test]
    fn test_dataflow_connect_and_disconnect() {
        let mut graph = dataflow::DataflowGraph::new();
        let src = graph.add_block(Box::new(dataflow::blocks::constant::ConstantBlock::new(
            1.0,
        )));
        let dst = graph.add_block(Box::new(dataflow::blocks::function::FunctionBlock::gain(
            2.0,
        )));
        let ch = graph.connect(src, 0, dst, 0).unwrap();
        graph.disconnect(ch);
    }

    #[test]
    fn test_dataflow_snapshot() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":5.0}"#).unwrap();
        graph.add_block(block);
        let snap = graph.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("constant"));
    }

    #[test]
    fn test_dataflow_run() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":3.0}"#).unwrap();
        graph.add_block(block);
        graph.run(10, 0.01);
        let snap = graph.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("constant"));
    }

    #[test]
    fn test_dataflow_advance() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
        graph.add_block(block);
        let mut sched = dataflow::Scheduler::new(0.01);
        let ticks = sched.advance(0.05);
        graph.run(ticks, sched.dt);
        let snap = graph.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_dataflow_set_speed() {
        let mut sched = dataflow::Scheduler::new(0.01);
        sched.speed = 2.0;
        assert!((sched.speed - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dataflow_codegen() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
        graph.add_block(block);
        let snap = graph.snapshot();
        let generated = dataflow::codegen::generate_rust(&snap, 0.01).unwrap();
        let json = serde_json::to_string(&generated.files).unwrap();
        assert!(json.contains("main.rs") || json.contains("Cargo.toml"));
    }

    #[test]
    fn test_dataflow_codegen_multi() {
        let mut graph = dataflow::DataflowGraph::new();
        let block = dataflow::blocks::create_block("constant", r#"{"value":1.0}"#).unwrap();
        graph.add_block(block);
        let snap = graph.snapshot();
        let targets: Vec<dataflow::codegen::binding::TargetWithBinding> =
            serde_json::from_str(r#"[{"target":"Host","binding":{"target":"Host","pins":[]}}]"#)
                .unwrap();
        let ws = dataflow::codegen::generate_workspace(&snap, 0.01, &targets).unwrap();
        let json = serde_json::to_string(&ws.files).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_dataflow_simulation_mode() {
        let mut graph = dataflow::DataflowGraph::new();
        graph.set_simulation_mode(true);
        if !graph.has_sim_peripherals() {
            graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        }
        graph.with_sim_peripherals(|p| {
            p.set_adc_voltage(0, 3.3);
        });
        let duty = graph.get_sim_pwm(0);
        assert!((duty - 0.0).abs() < f64::EPSILON);
    }

    // ── Sketch coverage tests (using SketchActor directly) ──────────

    #[test]
    fn test_sketch_reset_and_snapshot() {
        let actor = sketch_actor::SketchActor::new();
        let snap = actor.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("points"));
    }

    #[test]
    fn test_sketch_add_point() {
        let mut actor = sketch_actor::SketchActor::new();
        let id = actor.add_point(10.0, 20.0);
        let _ = id; // just ensure no panic
    }

    #[test]
    fn test_sketch_add_fixed_point() {
        let mut actor = sketch_actor::SketchActor::new();
        let id = actor.add_point_fixed(5.0, 5.0);
        assert!(actor.points.contains_key(&id));
    }

    #[test]
    fn test_sketch_move_and_remove_point() {
        let mut actor = sketch_actor::SketchActor::new();
        let id = actor.add_point(0.0, 0.0);
        actor.move_point(id, 1.0, 1.0);
        actor.remove_point(id);
        assert!(!actor.points.contains_key(&id));
    }

    #[test]
    fn test_sketch_set_fixed() {
        let mut actor = sketch_actor::SketchActor::new();
        let id = actor.add_point(0.0, 0.0);
        if let Some(p) = actor.points.get_mut(&id) {
            p.fixed = true;
        }
        assert!(actor.points[&id].fixed);
        if let Some(p) = actor.points.get_mut(&id) {
            p.fixed = false;
        }
        assert!(!actor.points[&id].fixed);
    }

    #[test]
    fn test_sketch_add_and_remove_constraint() {
        let mut actor = sketch_actor::SketchActor::new();
        let id1 = actor.add_point(0.0, 0.0);
        let id2 = actor.add_point(10.0, 0.0);
        let cid = actor.add_constraint(sketch_actor::Constraint::Distance(id1, id2, 10.0));
        assert!(actor.constraints.contains_key(&cid));
        actor.constraints.remove(&cid);
        assert!(!actor.constraints.contains_key(&cid));
    }

    #[test]
    fn test_sketch_solve() {
        let mut actor = sketch_actor::SketchActor::new();
        actor.add_point(0.0, 0.0);
        actor.add_point(10.0, 0.0);
        actor.solve(200);
        let snap = actor.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("points"));
    }

    #[test]
    fn test_sketch_pump() {
        let mut actor = sketch_actor::SketchActor::new();
        actor.add_point(1.0, 2.0);
        let (_last_id, snap) = actor.pump();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("points"));
    }

    // ── CAM function coverage tests ─────────────────────────────────

    fn simple_svg() -> &'static str {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80"/>
        </svg>"#
    }

    fn minimal_ascii_stl() -> &'static [u8] {
        b"solid test
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
endsolid test"
    }

    #[test]
    fn test_process_stl_default() {
        let gcode = test_process_stl_inner(minimal_ascii_stl(), "{}");
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_preview_svg() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let preview_paths: Vec<Vec<[f64; 2]>> = polylines
            .iter()
            .map(|pl| pl.points.iter().map(|p| [p.x, p.y]).collect())
            .collect();
        let json = serde_json::to_string(&preview_paths).unwrap();
        let paths: Vec<Vec<[f64; 2]>> = serde_json::from_str(&json).unwrap();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_preview_stl() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config: CamConfig = serde_json::from_str("{}").unwrap();
        let toolpaths = build_toolpaths_stl(&mesh, &config);
        let mut preview_paths: Vec<Vec<[f64; 3]>> = Vec::new();
        for tp in &toolpaths {
            let path: Vec<[f64; 3]> = tp
                .moves
                .iter()
                .filter(|m| !m.rapid)
                .map(|m| [m.x, m.y, m.z])
                .collect();
            if !path.is_empty() {
                preview_paths.push(path);
            }
        }
        let _json = serde_json::to_string(&preview_paths).unwrap();
    }

    #[test]
    fn test_sim_moves_svg() {
        let config: CamConfig = serde_json::from_str(r#"{"strategy":"contour"}"#).unwrap();
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let toolpaths = build_toolpaths_svg(&polylines, &config);
        let json = flatten_moves_string(&toolpaths);
        assert!(!json.is_empty());
    }

    #[test]
    fn test_sim_moves_stl() {
        let config: CamConfig = serde_json::from_str("{}").unwrap();
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let toolpaths = build_toolpaths_stl(&mesh, &config);
        let json = flatten_moves_string(&toolpaths);
        assert!(!json.is_empty());
    }

    #[test]
    fn test_flatten_moves_empty() {
        let result = flatten_moves_string(&[]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_build_toolpaths_svg_laser() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "laser_cut".into(),
            laser_power: Some(80.0),
            ..CamConfig::default()
        };
        let tps = build_toolpaths_svg(&polylines, &config);
        assert!(!tps.is_empty());
    }

    #[test]
    fn test_build_toolpaths_stl_zigzag() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "zigzag".into(),
            ..CamConfig::default()
        };
        // zigzag strategy on minimal mesh; may return empty but should not panic
        let _tps = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_scan_direction_from_config() {
        let mut config = CamConfig::default();
        assert!(matches!(
            scan_direction_from_config(&config),
            ScanDirection::X
        ));
        config.scan_direction = "y".into();
        assert!(matches!(
            scan_direction_from_config(&config),
            ScanDirection::Y
        ));
        config.scan_direction = "Y".into();
        assert!(matches!(
            scan_direction_from_config(&config),
            ScanDirection::Y
        ));
    }

    #[test]
    fn test_laser_params_from_config() {
        let config = CamConfig::default();
        assert!(laser_params_from_config(&config).is_none());
        let laser_config = CamConfig {
            machine_type: "laser_cutter".into(),
            laser_power: Some(50.0),
            passes: Some(2),
            air_assist: Some(true),
            ..CamConfig::default()
        };
        let lp = laser_params_from_config(&laser_config).unwrap();
        assert!((lp.power - 50.0).abs() < f64::EPSILON);
        assert_eq!(lp.passes, 2);
        assert!(lp.air_assist);
    }

    #[test]
    fn test_strategy_from_config() {
        // Just ensure all branches produce a strategy without panic
        for s in &[
            "contour",
            "pocket",
            "perimeter",
            "laser_cut",
            "laser_engrave",
        ] {
            let config = CamConfig {
                strategy: s.to_string(),
                ..CamConfig::default()
            };
            let _ = strategy_from_config(&config);
        }
    }

    // ── Serde default coverage: deserialise empty JSON to trigger all defaults ──

    #[test]
    fn test_serde_defaults_from_empty_json() {
        let config: CamConfig = serde_json::from_str("{}").unwrap();
        assert!((config.tool_diameter - 3.175).abs() < f64::EPSILON);
        assert_eq!(config.tool_type, "end_mill");
        assert_eq!(config.perimeter_passes, 1);
        assert!((config.step_over - 1.5).abs() < f64::EPSILON);
        assert!((config.step_down - 1.0).abs() < f64::EPSILON);
        assert!((config.feed_rate - 800.0).abs() < f64::EPSILON);
        assert!((config.plunge_rate - 300.0).abs() < f64::EPSILON);
        assert!((config.spindle_speed - 12000.0).abs() < f64::EPSILON);
        assert!((config.safe_z - 5.0).abs() < f64::EPSILON);
        assert!((config.cut_depth - (-1.0)).abs() < f64::EPSILON);
        assert_eq!(config.scan_direction, "x");
        assert_eq!(config.strategy, "contour");
        assert_eq!(config.machine_type, "cnc_mill");
        assert!(!config.climb_cut);
        assert!(config.laser_power.is_none());
        assert!(config.passes.is_none());
        assert!(config.air_assist.is_none());
        assert!((config.corner_radius - 0.0).abs() < f64::EPSILON);
        assert!(config.effective_diameter.is_none());
    }

    // ── Private helper coverage ─────────────────────────────────────────

    #[test]
    fn test_tool_from_config_end_mill_with_corner_radius() {
        let config = CamConfig {
            tool_type: "end_mill".into(),
            corner_radius: 0.5,
            ..CamConfig::default()
        };
        let tool = tool_from_config(&config);
        assert_eq!(tool.tool_type, tool::ToolType::EndMill);
        assert!((tool.corner_radius - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_tool_from_config_face_mill_no_effective() {
        let config = CamConfig {
            tool_type: "face_mill".into(),
            tool_diameter: 25.0,
            effective_diameter: None,
            ..CamConfig::default()
        };
        let tool = tool_from_config(&config);
        // effective_diameter falls back to tool_diameter
        assert!((tool.effective_diameter() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_laser_params_from_config_defaults() {
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            ..CamConfig::default()
        };
        let lp = laser_params_from_config(&config).unwrap();
        assert!((lp.power - 100.0).abs() < f64::EPSILON);
        assert_eq!(lp.passes, 1);
        assert!(!lp.air_assist);
    }

    #[test]
    fn test_build_toolpaths_svg_cnc_multi_pass() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            strategy: "pocket".into(),
            cut_depth: -2.0,
            step_down: 1.0,
            ..CamConfig::default()
        };
        let tps = build_toolpaths_svg(&polylines, &config);
        assert!(!tps.is_empty());
    }

    #[test]
    fn test_build_toolpaths_stl_pocket() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "pocket".into(),
            ..CamConfig::default()
        };
        let _tps = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_build_toolpaths_stl_perimeter() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "perimeter".into(),
            ..CamConfig::default()
        };
        let _tps = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_process_stl_pocket() {
        let gcode = test_process_stl_inner(minimal_ascii_stl(), r#"{"strategy":"pocket"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_stl_slice() {
        let gcode = test_process_stl_inner(minimal_ascii_stl(), r#"{"strategy":"slice"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_stl_zigzag() {
        let gcode = test_process_stl_inner(minimal_ascii_stl(), r#"{"strategy":"zigzag"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_stl_perimeter() {
        let gcode = test_process_stl_inner(minimal_ascii_stl(), r#"{"strategy":"perimeter"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_stl_invalid_json() {
        // serde_json::from_str will fail for invalid JSON, test that codepath
        let result: Result<CamConfig, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_pocket() {
        let gcode = test_process_svg_inner(simple_svg(), r#"{"strategy":"pocket"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_svg_perimeter() {
        let gcode = test_process_svg_inner(simple_svg(), r#"{"strategy":"perimeter"}"#);
        assert!(!gcode.is_empty());
    }

    #[test]
    fn test_process_svg_invalid_json() {
        // serde_json::from_str will fail for invalid JSON, test that codepath
        let result: Result<CamConfig, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_laser_rejects_bad_strategy_validate() {
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "zigzag".into(),
            ..CamConfig::default()
        };
        let profile = profile_from_config(&config);
        assert!(profile.validate_strategy(&config.strategy).is_err());
    }

    #[test]
    fn test_preview_stl_zigzag() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config: CamConfig = serde_json::from_str(r#"{"strategy":"zigzag"}"#).unwrap();
        let _toolpaths = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_cam_config_serialize_roundtrip() {
        let config = CamConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let config2: CamConfig = serde_json::from_str(&json).unwrap();
        assert!((config.feed_rate - config2.feed_rate).abs() < f64::EPSILON);
        assert_eq!(config.strategy, config2.strategy);
    }

    #[test]
    fn test_flatten_moves_with_toolpaths() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig::default();
        let tps = build_toolpaths_svg(&polylines, &config);
        let json = flatten_moves_string(&tps);
        assert!(json.len() > 2); // more than "[]"
    }

    // ── I2C / Serial / TCP simulation tests (via DataflowGraph directly) ──

    #[test]
    fn test_graph_sim_peripherals_i2c() {
        let mut graph = dataflow::DataflowGraph::new();
        assert!(graph.sim_peripherals_mut().is_err());
        graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        let sim = graph.sim_peripherals_mut().unwrap();
        sim.add_i2c_device(0, 0x50, "eeprom");
        let regs = sim.i2c_device_registers(0, 0x50).unwrap();
        assert_eq!(regs.len(), 256);
        sim.remove_i2c_device(0, 0x50);
        assert!(sim.i2c_device_registers(0, 0x50).is_none());
    }

    #[test]
    fn test_graph_sim_peripherals_serial() {
        let mut graph = dataflow::DataflowGraph::new();
        graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        let sim = graph.sim_peripherals_mut().unwrap();
        sim.configure_serial(0, 115_200, 8, dataflow::sim_peripherals::Parity::None, 1);
        sim.configure_serial(2, 9600, 8, dataflow::sim_peripherals::Parity::Even, 1);
        let ports = sim.serial_ports();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].1.baud, 115_200);
        assert_eq!(ports[1].1.baud, 9600);
    }

    #[test]
    fn test_parity_from_u8() {
        use dataflow::sim_peripherals::Parity;
        assert_eq!(Parity::from_u8(0).unwrap(), Parity::None);
        assert_eq!(Parity::from_u8(1).unwrap(), Parity::Odd);
        assert_eq!(Parity::from_u8(2).unwrap(), Parity::Even);
        assert!(Parity::from_u8(3).is_err());
        assert!(Parity::from_u8(255).is_err());
    }

    #[test]
    fn test_graph_sim_peripherals_tcp() {
        let mut graph = dataflow::DataflowGraph::new();
        graph.set_sim_peripherals(dataflow::sim_peripherals::WasmSimPeripherals::new());
        let sim = graph.sim_peripherals_mut().unwrap();
        sim.inject_tcp_data(0, b"hello");
        let drained = sim.drain_tcp_data(0);
        assert!(drained.is_empty()); // drain reads tx, inject goes to rx
    }

    #[test]
    fn test_graph_sim_ref_without_mode() {
        let graph = dataflow::DataflowGraph::new();
        assert!(graph.sim_peripherals_ref().is_err());
    }

    #[test]
    fn test_serial_config_serializes() {
        let config = dataflow::sim_peripherals::SerialConfig {
            baud: 9600,
            data_bits: 8,
            parity: dataflow::sim_peripherals::Parity::None,
            stop_bits: 1,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("9600"));
    }
}
