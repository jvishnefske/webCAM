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
#[cfg(target_arch = "wasm32")]
mod wasm_api;

use gcode::{emit_gcode_with_profile, GcodeParams, LaserParams};
use geometry::Toolpath;
use machine::{MachineProfile, MachineType};
use serde::{Deserialize, Serialize};
use tool::Tool;
use toolpath::{
    ContourStrategy, CutParams, LaserCutStrategy, LaserEngraveStrategy, Pattern, PerimeterStrategy,
    PocketStrategy, ScanDirection, Surface3dStrategy, SurfaceParams, ToolpathStrategy,
};

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
    /// Surface-strategy traversal pattern: `"zigzag"` (default), `"one_way"`,
    /// or `"spiral"`. Ignored for non-surface strategies.
    #[serde(default = "default_pattern")]
    pub pattern: String,
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
fn default_pattern() -> String {
    "zigzag".into()
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
            pattern: default_pattern(),
            machine_type: default_machine_type(),
            laser_power: None,
            passes: None,
            air_assist: None,
        }
    }
}

/// Parse scan direction from config string.
pub(crate) fn scan_direction_from_config(config: &CamConfig) -> ScanDirection {
    match config.scan_direction.as_str() {
        "y" | "Y" => ScanDirection::Y,
        _ => ScanDirection::X,
    }
}

/// Parse the surface traversal pattern from config string. Unknown values
/// fall back to `Pattern::ZigZag`.
pub(crate) fn pattern_from_config(config: &CamConfig) -> Pattern {
    match config.pattern.as_str() {
        "one_way" | "oneway" => Pattern::OneWay,
        "spiral" => Pattern::Spiral,
        _ => Pattern::ZigZag,
    }
}

/// Create a Tool from CamConfig fields.
pub(crate) fn tool_from_config(config: &CamConfig) -> Tool {
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
pub fn available_profiles() -> String {
    let profiles = vec![MachineProfile::cnc_mill(), MachineProfile::laser_cutter()];
    serde_json::to_string(&profiles).unwrap_or_else(|_| "[]".into())
}

/// Return a default config JSON for the given machine type.
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

/// Process an STL file (testable helper).
pub fn process_stl_impl(data: &[u8], config_json: &str) -> Result<String, String> {
    let config: CamConfig = serde_json::from_str(config_json).map_err(|e| e.to_string())?;

    let profile = profile_from_config(&config);
    profile.validate_strategy(&config.strategy)?;

    let mesh = stl::parse_stl(data)?;

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
        "surface3d" | "zigzag" => {
            let surface_params = SurfaceParams::new_with_pattern(
                &mesh,
                cut_params,
                scan_direction_from_config(&config),
                pattern_from_config(&config),
            );
            Surface3dStrategy.generate_surface(&surface_params)
        }
        "perimeter" => {
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
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let strategy = ContourStrategy;
            let mut all = Vec::new();
            for (z, contours) in &layers {
                let mut p = cut_params.clone();
                p.cut_z = *z;
                all.extend(strategy.generate(contours, &p));
            }
            if all.is_empty() {
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

/// Process an SVG string (testable helper).
pub fn process_svg_impl(svg_text: &str, config_json: &str) -> Result<String, String> {
    let config: CamConfig = serde_json::from_str(config_json).map_err(|e| e.to_string())?;

    let profile = profile_from_config(&config);
    profile.validate_strategy(&config.strategy)?;

    let polylines = svg::parse_svg(svg_text)?;

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

    let is_laser = profile.machine_type == MachineType::LaserCutter;
    let mut all_toolpaths = Vec::new();

    if is_laser {
        all_toolpaths.extend(strategy.generate(&polylines, &cut_params));
    } else {
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

/// STL preview (testable helper).
pub fn preview_stl_impl(data: &[u8], config_json: &str) -> Result<String, String> {
    let config: CamConfig = serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    let mesh = stl::parse_stl(data)?;
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
    serde_json::to_string(&preview_paths).map_err(|e| e.to_string())
}

/// SVG preview (testable helper).
pub fn preview_svg_impl(svg_text: &str) -> Result<String, String> {
    let polylines = svg::parse_svg(svg_text)?;
    let preview_paths: Vec<Vec<[f64; 2]>> = polylines
        .iter()
        .map(|pl| pl.points.iter().map(|p| [p.x, p.y]).collect())
        .collect();
    serde_json::to_string(&preview_paths).map_err(|e| e.to_string())
}

// ── Simulation data ──────────────────────────────────────────────────

/// STL sim moves (testable helper).
pub fn sim_moves_stl_impl(data: &[u8], config_json: &str) -> Result<String, String> {
    let config: CamConfig = serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    let mesh = stl::parse_stl(data)?;
    let toolpaths = build_toolpaths_stl(&mesh, &config);
    flatten_moves_impl(&toolpaths)
}

/// SVG sim moves (testable helper).
pub fn sim_moves_svg_impl(svg_text: &str, config_json: &str) -> Result<String, String> {
    let config: CamConfig = serde_json::from_str(config_json).map_err(|e| e.to_string())?;
    let polylines = svg::parse_svg(svg_text)?;
    let toolpaths = build_toolpaths_svg(&polylines, &config);
    flatten_moves_impl(&toolpaths)
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

    // Handle the 3D surface strategy separately (accepts both the legacy
    // "zigzag" name and the new "surface3d" name with pattern selection).
    if config.strategy == "zigzag" || config.strategy == "surface3d" {
        let surface_params = SurfaceParams::new_with_pattern(
            mesh,
            cut_params,
            scan_direction_from_config(config),
            pattern_from_config(config),
        );
        return Surface3dStrategy.generate_surface(&surface_params);
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

/// Flatten moves to JSON (testable helper).
fn flatten_moves_impl(toolpaths: &[Toolpath]) -> Result<String, String> {
    let moves: Vec<&geometry::ToolpathMove> =
        toolpaths.iter().flat_map(|tp| tp.moves.iter()).collect();
    serde_json::to_string(&moves).map_err(|e| e.to_string())
}

// ── Sketch Actor WASM API ────────────────────────────────────────────

use std::cell::RefCell;
thread_local! {
    static SKETCH: RefCell<sketch_actor::SketchActor> = RefCell::new(sketch_actor::SketchActor::new());
}

/// Reset the sketch actor to a blank state.
pub fn sketch_reset() {
    SKETCH.with(|s| *s.borrow_mut() = sketch_actor::SketchActor::new());
}

/// Add a free point. Returns JSON `{"id": <u32>}`.
pub fn sketch_add_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Add a fixed point. Returns JSON `{"id": <u32>}`.
pub fn sketch_add_fixed_point(x: f64, y: f64) -> String {
    SKETCH.with(|s| {
        let id = s.borrow_mut().add_point_fixed(x, y);
        format!(r#"{{"id":{id}}}"#)
    })
}

/// Move a point to new coordinates.
pub fn sketch_move_point(id: u32, x: f64, y: f64) {
    SKETCH.with(|s| s.borrow_mut().move_point(id, x, y));
}

/// Remove a point and all its constraints.
pub fn sketch_remove_point(id: u32) {
    SKETCH.with(|s| s.borrow_mut().remove_point(id));
}

/// Set a point's fixed flag.
pub fn sketch_set_fixed(id: u32, fixed: bool) {
    SKETCH.with(|s| {
        if let Some(p) = s.borrow_mut().points.get_mut(&id) {
            p.fixed = fixed;
        }
    });
}

/// Add a constraint (testable helper).
pub fn sketch_add_constraint_impl(
    kind: &str,
    ids_json: &str,
    value: f64,
    value2: f64,
) -> Result<String, String> {
    let ids: Vec<u32> = serde_json::from_str(ids_json).map_err(|e| e.to_string())?;

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
            return Err(format!(
                "Unknown constraint '{kind}' or wrong number of ids"
            ));
        }
    };

    SKETCH.with(|s| {
        let id = s.borrow_mut().add_constraint(constraint);
        Ok(format!(r#"{{"id":{id}}}"#))
    })
}

/// Remove a constraint by id.
pub fn sketch_remove_constraint(id: u32) {
    SKETCH.with(|s| {
        s.borrow_mut().constraints.remove(&id);
    });
}

/// Solve (testable helper).
pub fn sketch_solve_impl() -> Result<String, String> {
    SKETCH.with(|s| {
        let mut actor = s.borrow_mut();
        actor.solve(200);
        let snap = actor.snapshot();
        serde_json::to_string(&snap).map_err(|e| e.to_string())
    })
}

/// Pump (testable helper).
pub fn sketch_pump_impl() -> Result<String, String> {
    SKETCH.with(|s| {
        let mut actor = s.borrow_mut();
        let (_last_id, snap) = actor.pump();
        serde_json::to_string(&snap).map_err(|e| e.to_string())
    })
}

/// Snapshot (testable helper).
pub fn sketch_snapshot_impl() -> Result<String, String> {
    SKETCH.with(|s| {
        let snap = s.borrow().snapshot();
        serde_json::to_string(&snap).map_err(|e| e.to_string())
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
        let result = process_svg_impl(svg, config_json);
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
        let result = process_svg_impl(svg, config_json);
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
        let result = process_svg_impl(svg, config_json);
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

    // ── Sketch WASM API coverage tests ──────────────────────────────

    #[test]
    fn test_sketch_reset_and_snapshot() {
        sketch_reset();
        let snap = sketch_snapshot_impl().unwrap();
        assert!(snap.contains("points"));
    }

    #[test]
    fn test_sketch_add_point() {
        sketch_reset();
        let r = sketch_add_point(10.0, 20.0);
        assert!(r.contains("\"id\""));
    }

    #[test]
    fn test_sketch_add_fixed_point() {
        sketch_reset();
        let r = sketch_add_fixed_point(5.0, 5.0);
        assert!(r.contains("\"id\""));
    }

    #[test]
    fn test_sketch_move_and_remove_point() {
        sketch_reset();
        let r: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let id = r["id"].as_u64().unwrap() as u32;
        sketch_move_point(id, 1.0, 1.0);
        sketch_remove_point(id);
    }

    #[test]
    fn test_sketch_set_fixed() {
        sketch_reset();
        let r: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let id = r["id"].as_u64().unwrap() as u32;
        sketch_set_fixed(id, true);
        sketch_set_fixed(id, false);
    }

    #[test]
    fn test_sketch_add_and_remove_constraint() {
        sketch_reset();
        let p1: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let p2: serde_json::Value = serde_json::from_str(&sketch_add_point(10.0, 0.0)).unwrap();
        let id1 = p1["id"].as_u64().unwrap() as u32;
        let id2 = p2["id"].as_u64().unwrap() as u32;
        let ids_json = format!("[{id1},{id2}]");
        let cr = sketch_add_constraint_impl("distance", &ids_json, 10.0, 0.0).unwrap();
        assert!(cr.contains("\"id\""));
        let cv: serde_json::Value = serde_json::from_str(&cr).unwrap();
        let cid = cv["id"].as_u64().unwrap() as u32;
        sketch_remove_constraint(cid);
    }

    #[test]
    fn test_sketch_solve() {
        sketch_reset();
        sketch_add_point(0.0, 0.0);
        sketch_add_point(10.0, 0.0);
        let snap = sketch_solve_impl().unwrap();
        assert!(snap.contains("points"));
    }

    #[test]
    fn test_sketch_pump() {
        sketch_reset();
        sketch_add_point(1.0, 2.0);
        let snap = sketch_pump_impl().unwrap();
        assert!(snap.contains("points"));
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
        let result = process_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_preview_svg() {
        let result = preview_svg_impl(simple_svg());
        assert!(result.is_ok());
        let json = result.unwrap();
        let paths: Vec<Vec<[f64; 2]>> = serde_json::from_str(&json).unwrap();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_preview_stl() {
        let result = preview_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_sim_moves_svg() {
        let result = sim_moves_svg_impl(simple_svg(), r#"{"strategy":"contour"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sim_moves_stl() {
        let result = sim_moves_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_flatten_moves_empty() {
        let result = flatten_moves_impl(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "[]");
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

    // ── Default function coverage ───────────────────────────────────

    #[test]
    fn test_all_default_functions() {
        assert_eq!(default_tool_diameter(), 3.175);
        assert_eq!(default_tool_type(), "end_mill");
        assert_eq!(default_step_over(), 1.5);
        assert_eq!(default_step_down(), 1.0);
        assert_eq!(default_feed_rate(), 800.0);
        assert_eq!(default_plunge_rate(), 300.0);
        assert_eq!(default_spindle_speed(), 12000.0);
        assert_eq!(default_safe_z(), 5.0);
        assert_eq!(default_cut_depth(), -1.0);
        assert_eq!(default_strategy(), "contour");
        assert_eq!(default_machine_type(), "cnc_mill");
        assert_eq!(default_scan_direction(), "x");
        assert_eq!(default_perimeter_passes(), 1);
    }

    // ── Additional strategy coverage on STL ─────────────────────────

    #[test]
    fn test_build_toolpaths_stl_pocket() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "pocket".into(),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_stl(&mesh, &config);
        let _ = paths;
    }

    #[test]
    fn test_build_toolpaths_stl_perimeter() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "perimeter".into(),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_stl(&mesh, &config);
        let _ = paths;
    }

    // ── Additional strategy coverage on SVG ─────────────────────────

    #[test]
    fn test_build_toolpaths_svg_contour() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            strategy: "contour".into(),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_build_toolpaths_svg_pocket() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            strategy: "pocket".into(),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        let _ = paths;
    }

    #[test]
    fn test_build_toolpaths_svg_perimeter() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            strategy: "perimeter".into(),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        let _ = paths;
    }

    #[test]
    fn test_build_toolpaths_svg_laser_engrave() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "laser_engrave".into(),
            laser_power: Some(60.0),
            step_over: 2.0,
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        let _ = paths;
    }

    // ── CAM pipeline integration tests ─────────────────────────────

    /// Helper: construct a minimal binary STL with one triangle.
    fn minimal_binary_stl() -> Vec<u8> {
        let mut data = Vec::new();
        // 80-byte header
        data.extend_from_slice(&[0u8; 80]);
        // Triangle count: 1 (little-endian u32)
        data.extend_from_slice(&1u32.to_le_bytes());
        // Normal: (0, 0, 1)
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        // Vertex 0: (0, 0, 0)
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        // Vertex 1: (10, 0, 0)
        data.extend_from_slice(&10.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        // Vertex 2: (0, 10, 0)
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&10.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        // Attribute byte count
        data.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(data.len(), 134);
        data
    }

    /// Helper: SVG with a <path> element instead of <rect>.
    fn svg_with_path() -> &'static str {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M 0 0 L 10 0 L 10 10 L 0 10 Z"/>
        </svg>"#
    }

    // ── process_stl: strategy coverage and G-code validation ────────

    #[test]
    fn test_process_stl_binary_stl_default_strategy() {
        let stl_data = minimal_binary_stl();
        let result = process_stl_impl(&stl_data, "{}");
        assert!(
            result.is_ok(),
            "binary STL with default config should succeed"
        );
        let gcode = result.unwrap();
        assert!(gcode.contains("G21"), "should contain metric unit mode");
        assert!(gcode.contains("G90"), "should contain absolute positioning");
        assert!(gcode.contains("M3 S12000"), "should turn spindle on");
        assert!(gcode.contains("M5"), "should turn spindle off at end");
        assert!(gcode.contains("M2"), "should have program end");
    }

    #[test]
    fn test_process_stl_contour_strategy() {
        let config_json = r#"{"strategy": "contour"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(
            gcode.contains("G0") || gcode.contains("G1"),
            "should have motion commands"
        );
    }

    #[test]
    fn test_process_stl_pocket_strategy() {
        let config_json = r#"{"strategy": "pocket"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_slice_strategy() {
        let config_json = r#"{"strategy": "slice"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_perimeter_strategy() {
        let config_json = r#"{"strategy": "perimeter"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_zigzag_strategy() {
        let config_json = r#"{"strategy": "zigzag"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
    }

    // Error-path tests use the _impl helpers (returning Result<T, String>)
    // which are fully testable in native mode.

    #[test]
    fn test_process_stl_custom_params_in_gcode() {
        let config_json = r#"{
            "feed_rate": 500,
            "spindle_speed": 8000,
            "safe_z": 10.0,
            "strategy": "contour"
        }"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(
            gcode.contains("M3 S8000"),
            "should use custom spindle speed"
        );
        assert!(gcode.contains("Z10.000"), "should use custom safe Z");
    }

    // ── process_svg: strategy coverage and G-code validation ────────

    #[test]
    fn test_process_svg_with_path_element() {
        let config_json = r#"{"strategy": "contour"}"#;
        let result = process_svg_impl(svg_with_path(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(
            gcode.contains("G1"),
            "contour should produce G1 cutting moves"
        );
        assert!(
            gcode.contains("M3 S12000"),
            "CNC mill should turn spindle on"
        );
    }

    #[test]
    fn test_process_svg_pocket_strategy() {
        let config_json = r#"{"strategy": "pocket"}"#;
        let result = process_svg_impl(simple_svg(), config_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_svg_perimeter_strategy() {
        let config_json = r#"{"strategy": "perimeter"}"#;
        let result = process_svg_impl(simple_svg(), config_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_svg_laser_cut_gcode_structure() {
        let config_json = r#"{
            "machine_type": "laser_cutter",
            "strategy": "laser_cut",
            "laser_power": 75
        }"#;
        let result = process_svg_impl(svg_with_path(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(
            gcode.contains("M4 S0"),
            "should have dynamic laser mode preamble"
        );
        assert!(gcode.contains("S75"), "should set laser power to 75");
        assert!(gcode.contains("M5"), "should turn laser off");
        assert!(gcode.contains("M2"), "should end program");
        // Laser cutter should NOT have spindle commands
        assert!(
            !gcode.contains("M3 S12000"),
            "laser should not use M3 spindle on"
        );
    }

    #[test]
    fn test_process_svg_step_down_produces_multiple_layers() {
        // With cut_depth=-3 and step_down=1, should produce 3 layers of toolpaths
        let config_json = r#"{
            "strategy": "contour",
            "cut_depth": -3.0,
            "step_down": 1.0
        }"#;
        let result = process_svg_impl(svg_with_path(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        // Multiple toolpath sections => multiple "(Toolpath N)" comments
        let toolpath_count = gcode.matches("(Toolpath").count();
        assert!(
            toolpath_count >= 3,
            "3mm depth at 1mm step should produce at least 3 toolpath groups, got {}",
            toolpath_count
        );
    }

    // ── preview_stl: JSON structure validation ──────────────────────

    #[test]
    fn test_preview_stl_returns_valid_json_array() {
        let result = preview_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
        let json = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array(), "preview_stl should return a JSON array");
    }

    #[test]
    fn test_preview_stl_binary_format() {
        let stl_data = minimal_binary_stl();
        let result = preview_stl_impl(&stl_data, "{}");
        assert!(result.is_ok());
        let json = result.unwrap();
        let paths: Vec<Vec<[f64; 3]>> = serde_json::from_str(&json).unwrap();
        // Each inner array element has 3 coordinates (x, y, z)
        for path in &paths {
            for point in path {
                assert_eq!(point.len(), 3, "each point should have 3 coordinates");
            }
        }
    }

    // ── preview_svg: JSON structure validation ──────────────────────

    #[test]
    fn test_preview_svg_returns_2d_coordinates() {
        let result = preview_svg_impl(svg_with_path());
        assert!(result.is_ok());
        let json = result.unwrap();
        let paths: Vec<Vec<[f64; 2]>> = serde_json::from_str(&json).unwrap();
        assert!(!paths.is_empty(), "should produce at least one path");
        for path in &paths {
            assert!(!path.is_empty(), "each path should have points");
            for point in path {
                assert_eq!(point.len(), 2, "each point should have 2 coordinates");
            }
        }
    }

    #[test]
    fn test_preview_svg_rect_coordinates_in_range() {
        let result = preview_svg_impl(simple_svg());
        assert!(result.is_ok());
        let json = result.unwrap();
        let paths: Vec<Vec<[f64; 2]>> = serde_json::from_str(&json).unwrap();
        // The rect is at x=10,y=10 width=80 height=80, so coords should be in [10, 90]
        for path in &paths {
            for [x, y] in path {
                assert!(
                    *x >= 9.0 && *x <= 91.0,
                    "x={} should be within rect bounds",
                    x
                );
                assert!(
                    *y >= 9.0 && *y <= 91.0,
                    "y={} should be within rect bounds",
                    y
                );
            }
        }
    }

    // ── available_profiles: content validation ──────────────────────

    #[test]
    fn test_available_profiles_contains_expected_names() {
        let json = available_profiles();
        let profiles: Vec<MachineProfile> = serde_json::from_str(&json).unwrap();
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"CNC Mill"), "should contain CNC Mill");
        assert!(
            names.contains(&"Laser Cutter"),
            "should contain Laser Cutter"
        );
    }

    #[test]
    fn test_available_profiles_cnc_has_spindle() {
        let json = available_profiles();
        let profiles: Vec<MachineProfile> = serde_json::from_str(&json).unwrap();
        let cnc = profiles
            .iter()
            .find(|p| p.machine_type == MachineType::CncMill)
            .expect("should have CNC mill profile");
        assert!(cnc.capabilities.has_spindle);
        assert!(cnc.capabilities.has_z_axis);
        assert!(!cnc.capabilities.has_laser_power);
    }

    #[test]
    fn test_available_profiles_laser_has_laser_power() {
        let json = available_profiles();
        let profiles: Vec<MachineProfile> = serde_json::from_str(&json).unwrap();
        let laser = profiles
            .iter()
            .find(|p| p.machine_type == MachineType::LaserCutter)
            .expect("should have Laser Cutter profile");
        assert!(laser.capabilities.has_laser_power);
        assert!(!laser.capabilities.has_spindle);
        assert!(!laser.capabilities.has_z_axis);
    }

    // ── default_config: content validation ──────────────────────────

    #[test]
    fn test_default_config_cnc_mill_3axis() {
        let json = default_config("cnc_mill_3axis");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        // Unknown profile falls through to cnc_mill defaults
        assert_eq!(config.machine_type, "cnc_mill_3axis");
        assert_eq!(config.strategy, "contour");
        assert!(config.laser_power.is_none());
    }

    #[test]
    fn test_default_config_laser_has_laser_fields() {
        let json = default_config("laser_cutter");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.laser_power, Some(100.0));
        assert_eq!(config.passes, Some(1));
        assert_eq!(config.strategy, "laser_cut");
    }

    #[test]
    fn test_default_config_unknown_profile_returns_cnc_defaults() {
        let json = default_config("unknown_machine");
        let config: CamConfig = serde_json::from_str(&json).unwrap();
        // Unknown profile gets CNC mill defaults
        assert_eq!(config.strategy, "contour");
        assert!(config.laser_power.is_none());
    }

    // ── sim_moves: JSON structure validation ────────────────────────

    #[test]
    fn test_sim_moves_stl_returns_move_objects() {
        let result = sim_moves_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
        let json = result.unwrap();
        let moves: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        // Check that moves have the expected fields
        for mv in &moves {
            assert!(mv.get("x").is_some(), "move should have x field");
            assert!(mv.get("y").is_some(), "move should have y field");
            assert!(mv.get("z").is_some(), "move should have z field");
            assert!(mv.get("rapid").is_some(), "move should have rapid field");
        }
    }

    #[test]
    fn test_sim_moves_svg_returns_move_objects() {
        let result = sim_moves_svg_impl(svg_with_path(), r#"{"strategy":"contour"}"#);
        assert!(result.is_ok());
        let json = result.unwrap();
        let moves: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(!moves.is_empty(), "SVG sim should produce moves");
        for mv in &moves {
            assert!(mv["x"].is_number());
            assert!(mv["y"].is_number());
            assert!(mv["z"].is_number());
            assert!(mv["rapid"].is_boolean());
        }
    }

    // ── End-to-end pipeline: STL -> G-code content checks ───────────

    #[test]
    fn test_process_stl_binary_gcode_has_motion() {
        let stl_data = minimal_binary_stl();
        let config_json = r#"{"strategy": "contour", "step_down": 1.0, "cut_depth": -1.0}"#;
        let result = process_stl_impl(&stl_data, config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        // Should have rapid moves (G0) and cutting moves (G1)
        assert!(gcode.contains("G0"), "should have rapid moves");
        // G-code structure: preamble, toolpaths, postamble
        assert!(
            gcode.starts_with("(RustCAM"),
            "should start with RustCAM header"
        );
    }

    // ── End-to-end pipeline: SVG -> G-code content checks ───────────

    #[test]
    fn test_process_svg_gcode_well_formed() {
        let config_json = r#"{"strategy": "contour"}"#;
        let result = process_svg_impl(svg_with_path(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();

        // Check G-code is well-formed: starts with header, ends with footer
        let lines: Vec<&str> = gcode.lines().collect();
        assert!(
            lines[0].contains("RustCAM"),
            "first line should be RustCAM header"
        );
        assert!(
            lines.last().unwrap().contains("M2"),
            "last line should be program end"
        );

        // Verify ordering: preamble before motion, motion before postamble
        let m3_pos = gcode.find("M3").expect("should have spindle on");
        let g1_pos = gcode.find("G1").expect("should have cutting moves");
        let m5_pos = gcode.find("M5").expect("should have spindle off");
        assert!(m3_pos < g1_pos, "spindle on should be before cutting");
        assert!(g1_pos < m5_pos, "cutting should be before spindle off");
    }

    #[test]
    fn test_process_svg_laser_engrave_with_custom_step_over() {
        let config_json = r#"{
            "machine_type": "laser_cutter",
            "strategy": "laser_engrave",
            "laser_power": 40,
            "step_over": 0.5
        }"#;
        let result = process_svg_impl(simple_svg(), config_json);
        assert!(result.is_ok());
        let gcode = result.unwrap();
        assert!(gcode.contains("S40"), "should use power 40");
        assert!(gcode.contains("M4 S0"), "should start in dynamic mode");
    }

    // ── process_stl with binary STL and all strategies ──────────────

    #[test]
    fn test_process_stl_binary_pocket() {
        let stl_data = minimal_binary_stl();
        let result = process_stl_impl(&stl_data, r#"{"strategy": "pocket"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_binary_zigzag() {
        let stl_data = minimal_binary_stl();
        let result = process_stl_impl(&stl_data, r#"{"strategy": "zigzag"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_binary_slice() {
        let stl_data = minimal_binary_stl();
        let result = process_stl_impl(&stl_data, r#"{"strategy": "slice"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_binary_perimeter() {
        let stl_data = minimal_binary_stl();
        let result = process_stl_impl(&stl_data, r#"{"strategy": "perimeter"}"#);
        assert!(result.is_ok());
    }

    // ── CamConfig edge cases ────────────────────────────────────────

    #[test]
    fn test_camconfig_all_fields_from_json() {
        let json = r#"{
            "tool_diameter": 6.0,
            "tool_type": "ball_end",
            "corner_radius": 1.0,
            "effective_diameter": 5.5,
            "step_over": 2.0,
            "step_down": 0.5,
            "feed_rate": 1200,
            "plunge_rate": 400,
            "spindle_speed": 24000,
            "safe_z": 15.0,
            "cut_depth": -5.0,
            "strategy": "pocket",
            "climb_cut": true,
            "perimeter_passes": 3,
            "scan_direction": "y",
            "machine_type": "cnc_mill",
            "laser_power": null,
            "passes": null,
            "air_assist": null
        }"#;
        let config: CamConfig = serde_json::from_str(json).unwrap();
        assert!((config.tool_diameter - 6.0).abs() < f64::EPSILON);
        assert_eq!(config.tool_type, "ball_end");
        assert_eq!(config.strategy, "pocket");
        assert!(config.climb_cut);
        assert_eq!(config.perimeter_passes, 3);
        assert_eq!(config.scan_direction, "y");
    }

    // ── Error-path coverage: sketch_add_constraint ──────────────────

    #[test]
    fn test_sketch_add_constraint_unknown_kind() {
        sketch_reset();
        sketch_add_point(0.0, 0.0);
        let result = sketch_add_constraint_impl("nonexistent", "[0]", 0.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sketch_add_constraint_too_few_ids() {
        sketch_reset();
        let p1: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let id1 = p1["id"].as_u64().unwrap() as u32;
        // "distance" needs 2 ids, pass only 1
        let result = sketch_add_constraint_impl("distance", &format!("[{id1}]"), 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sketch_add_constraint_invalid_json() {
        sketch_reset();
        let result = sketch_add_constraint_impl("distance", "not valid json", 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sketch_add_constraint_all_kinds() {
        sketch_reset();
        let p1: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let p2: serde_json::Value = serde_json::from_str(&sketch_add_point(10.0, 0.0)).unwrap();
        let p3: serde_json::Value = serde_json::from_str(&sketch_add_point(5.0, 5.0)).unwrap();
        let p4: serde_json::Value = serde_json::from_str(&sketch_add_point(10.0, 10.0)).unwrap();
        let id1 = p1["id"].as_u64().unwrap() as u32;
        let id2 = p2["id"].as_u64().unwrap() as u32;
        let id3 = p3["id"].as_u64().unwrap() as u32;
        let id4 = p4["id"].as_u64().unwrap() as u32;

        // 2-point constraint kinds
        let two_ids = format!("[{id1},{id2}]");
        assert!(sketch_add_constraint_impl("coincident", &two_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("horizontal", &two_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("vertical", &two_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("angle", &two_ids, 45.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("radius", &two_ids, 5.0, 0.0).is_ok());

        // 1-point constraint kind
        let one_id = format!("[{id1}]");
        assert!(sketch_add_constraint_impl("fixed", &one_id, 1.0, 2.0).is_ok());

        // 3-point constraint kind
        let three_ids = format!("[{id1},{id2},{id3}]");
        assert!(sketch_add_constraint_impl("midpoint", &three_ids, 0.0, 0.0).is_ok());

        // 4-point constraint kinds
        let four_ids = format!("[{id1},{id2},{id3},{id4}]");
        assert!(sketch_add_constraint_impl("perpendicular", &four_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("parallel", &four_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("equal_length", &four_ids, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("symmetric", &four_ids, 0.0, 0.0).is_ok());
    }

    #[test]
    fn test_sketch_set_fixed_nonexistent_point() {
        sketch_reset();
        // Should not panic when setting fixed on a nonexistent point
        sketch_set_fixed(9999, true);
    }

    // ── Internal helper coverage ────────────────────────────────────

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
    fn test_tool_from_config_face_mill_no_effective_diameter() {
        // When effective_diameter is None, should use tool_diameter
        let config = CamConfig {
            tool_type: "face_mill".into(),
            tool_diameter: 50.0,
            effective_diameter: None,
            ..CamConfig::default()
        };
        let tool = tool_from_config(&config);
        assert!((tool.effective_diameter() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_laser_params_defaults() {
        // Laser config without optional fields should use defaults
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            laser_power: None,
            passes: None,
            air_assist: None,
            ..CamConfig::default()
        };
        let lp = laser_params_from_config(&config).unwrap();
        assert!((lp.power - 100.0).abs() < f64::EPSILON);
        assert_eq!(lp.passes, 1);
        assert!(!lp.air_assist);
    }

    #[test]
    fn test_build_toolpaths_stl_contour_fallback() {
        // Build a mesh that won't produce any slices at default step_down
        // to exercise the empty-toolpath fallback path
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "contour".into(),
            step_down: 100.0, // very large step, may result in empty layers
            ..CamConfig::default()
        };
        // Should not panic, exercises the fallback path
        let _paths = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_camconfig_serialize_roundtrip() {
        let config = CamConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let config2: CamConfig = serde_json::from_str(&json).unwrap();
        assert!((config.tool_diameter - config2.tool_diameter).abs() < f64::EPSILON);
        assert_eq!(config.strategy, config2.strategy);
        assert_eq!(config.machine_type, config2.machine_type);
    }

    // ── Additional coverage: build_toolpaths internal helpers ───────

    #[test]
    fn test_build_toolpaths_stl_slice_strategy() {
        // "slice" is not a named match in build_toolpaths_stl, falls to default (ContourStrategy)
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "slice".into(),
            ..CamConfig::default()
        };
        let _paths = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_build_toolpaths_stl_zigzag_y_direction() {
        let mesh = stl::parse_stl(minimal_ascii_stl()).unwrap();
        let config = CamConfig {
            strategy: "zigzag".into(),
            scan_direction: "y".into(),
            ..CamConfig::default()
        };
        let _paths = build_toolpaths_stl(&mesh, &config);
    }

    #[test]
    fn test_build_toolpaths_svg_laser_cut() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            strategy: "laser_cut".into(),
            laser_power: Some(90.0),
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_build_toolpaths_svg_perimeter_cnc() {
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig {
            strategy: "perimeter".into(),
            cut_depth: -2.0,
            step_down: 1.0,
            ..CamConfig::default()
        };
        let paths = build_toolpaths_svg(&polylines, &config);
        // Should produce toolpaths at multiple Z levels
        let _ = paths;
    }

    #[test]
    fn test_camconfig_empty_json_uses_all_defaults() {
        // Deserialize from "{}" and verify all serde defaults are applied
        let config: CamConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.tool_diameter, default_tool_diameter());
        assert_eq!(config.tool_type, default_tool_type());
        assert_eq!(config.step_over, default_step_over());
        assert_eq!(config.step_down, default_step_down());
        assert_eq!(config.feed_rate, default_feed_rate());
        assert_eq!(config.plunge_rate, default_plunge_rate());
        assert_eq!(config.spindle_speed, default_spindle_speed());
        assert_eq!(config.safe_z, default_safe_z());
        assert_eq!(config.cut_depth, default_cut_depth());
        assert_eq!(config.strategy, default_strategy());
        assert_eq!(config.machine_type, default_machine_type());
        assert_eq!(config.scan_direction, default_scan_direction());
        assert_eq!(config.perimeter_passes, default_perimeter_passes());
        assert_eq!(config.corner_radius, 0.0);
        assert!(config.effective_diameter.is_none());
        assert!(!config.climb_cut);
        assert!(config.laser_power.is_none());
        assert!(config.passes.is_none());
        assert!(config.air_assist.is_none());
    }

    #[test]
    fn test_flatten_moves_with_toolpaths() {
        // Build actual toolpaths and flatten them
        let polylines = svg::parse_svg(simple_svg()).unwrap();
        let config = CamConfig::default();
        let toolpaths = build_toolpaths_svg(&polylines, &config);
        let result = flatten_moves_impl(&toolpaths);
        assert!(result.is_ok());
        let json = result.unwrap();
        let moves: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(
            !moves.is_empty(),
            "should have moves from non-empty toolpaths"
        );
    }

    #[test]
    fn test_strategy_from_config_unknown_defaults_to_contour() {
        let config = CamConfig {
            strategy: "nonexistent_strategy".into(),
            ..CamConfig::default()
        };
        // Should not panic; defaults to ContourStrategy
        let _strategy = strategy_from_config(&config);
    }

    // ── _impl helper error-path tests ──────────────────────────────────

    #[test]
    fn test_process_stl_impl_success() {
        let result = process_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_stl_impl_invalid_json() {
        let result = process_stl_impl(minimal_ascii_stl(), "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_stl_impl_invalid_stl() {
        let result = process_stl_impl(b"not a valid stl", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_stl_impl_rejected_strategy() {
        let config_json = r#"{"machine_type": "laser_cutter", "strategy": "zigzag"}"#;
        let result = process_stl_impl(minimal_ascii_stl(), config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_impl_success() {
        let result = process_svg_impl(simple_svg(), r#"{"strategy":"contour"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_svg_impl_invalid_json() {
        let result = process_svg_impl(simple_svg(), "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_impl_invalid_svg() {
        let result = process_svg_impl("not svg data", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_impl_rejected_strategy() {
        let config_json = r#"{"machine_type": "laser_cutter", "strategy": "zigzag"}"#;
        let result = process_svg_impl(simple_svg(), config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_stl_impl_success() {
        let result = preview_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_preview_stl_impl_invalid_json() {
        let result = preview_stl_impl(minimal_ascii_stl(), "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_stl_impl_invalid_stl() {
        let result = preview_stl_impl(b"not stl", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_svg_impl_success() {
        let result = preview_svg_impl(simple_svg());
        assert!(result.is_ok());
    }

    #[test]
    fn test_preview_svg_impl_invalid_svg() {
        let result = preview_svg_impl("not svg");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_stl_impl_success() {
        let result = sim_moves_stl_impl(minimal_ascii_stl(), "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_sim_moves_stl_impl_invalid_json() {
        let result = sim_moves_stl_impl(minimal_ascii_stl(), "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_stl_impl_invalid_stl() {
        let result = sim_moves_stl_impl(b"not stl", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_svg_impl_success() {
        let result = sim_moves_svg_impl(simple_svg(), r#"{"strategy":"contour"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sim_moves_svg_impl_invalid_json() {
        let result = sim_moves_svg_impl(simple_svg(), "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_svg_impl_invalid_svg() {
        let result = sim_moves_svg_impl("not svg", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_flatten_moves_impl_empty() {
        let result = flatten_moves_impl(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "[]");
    }

    #[test]
    fn test_sketch_add_constraint_impl_success() {
        sketch_reset();
        let p1: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let p2: serde_json::Value = serde_json::from_str(&sketch_add_point(10.0, 0.0)).unwrap();
        let id1 = p1["id"].as_u64().unwrap() as u32;
        let id2 = p2["id"].as_u64().unwrap() as u32;
        let ids_json = format!("[{id1},{id2}]");
        let result = sketch_add_constraint_impl("distance", &ids_json, 10.0, 0.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sketch_add_constraint_impl_invalid_ids_json() {
        sketch_reset();
        let result = sketch_add_constraint_impl("distance", "not json", 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sketch_add_constraint_impl_unknown_kind() {
        sketch_reset();
        let result = sketch_add_constraint_impl("unknown_kind", "[1,2]", 10.0, 0.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown constraint"));
    }

    #[test]
    fn test_sketch_add_constraint_impl_too_few_ids() {
        sketch_reset();
        let result = sketch_add_constraint_impl("perpendicular", "[1,2]", 0.0, 0.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong number of ids"));
    }

    #[test]
    fn test_sketch_solve_impl() {
        sketch_reset();
        sketch_add_point(0.0, 0.0);
        let snap = sketch_solve_impl().unwrap();
        assert!(snap.contains("points"));
    }

    #[test]
    fn test_sketch_pump_impl() {
        sketch_reset();
        sketch_add_point(1.0, 2.0);
        let snap = sketch_pump_impl().unwrap();
        assert!(snap.contains("points"));
    }

    #[test]
    fn test_sketch_snapshot_impl() {
        sketch_reset();
        let snap = sketch_snapshot_impl().unwrap();
        assert!(snap.contains("points"));
    }

    // ── All constraint kinds via _impl ─────────────────────────────────

    #[test]
    fn test_sketch_constraint_all_kinds_impl() {
        sketch_reset();
        // Create enough points for all constraints
        let mut ids = Vec::new();
        for i in 0..5 {
            let r: serde_json::Value =
                serde_json::from_str(&sketch_add_point(i as f64 * 10.0, 0.0)).unwrap();
            ids.push(r["id"].as_u64().unwrap() as u32);
        }

        let two = format!("[{},{}]", ids[0], ids[1]);
        let three = format!("[{},{},{}]", ids[0], ids[1], ids[2]);
        let four = format!("[{},{},{},{}]", ids[0], ids[1], ids[2], ids[3]);

        assert!(sketch_add_constraint_impl("coincident", &two, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("horizontal", &two, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("vertical", &two, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("fixed", &format!("[{}]", ids[0]), 5.0, 10.0).is_ok());
        assert!(sketch_add_constraint_impl("angle", &two, 45.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("radius", &two, 5.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("perpendicular", &four, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("parallel", &four, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("midpoint", &three, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("equal_length", &four, 0.0, 0.0).is_ok());
        assert!(sketch_add_constraint_impl("symmetric", &four, 0.0, 0.0).is_ok());
    }

    // ── Error path tests ─────────────────────────────────────────────

    #[test]
    fn test_process_stl_invalid_config_json() {
        let result = process_stl_impl(minimal_ascii_stl(), "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_invalid_config_json() {
        let result = process_svg_impl(simple_svg(), "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_stl_invalid_config_json() {
        let result = preview_stl_impl(minimal_ascii_stl(), "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_stl_invalid_config_json() {
        let result = sim_moves_stl_impl(minimal_ascii_stl(), "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_sim_moves_svg_invalid_config_json() {
        let result = sim_moves_svg_impl(simple_svg(), "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_svg_invalid_svg() {
        let result = preview_svg_impl("<svg></svg>");
        assert!(result.is_err());
    }

    #[test]
    fn test_sketch_add_constraint_unknown_kind_detailed() {
        sketch_reset();
        let p1: serde_json::Value = serde_json::from_str(&sketch_add_point(0.0, 0.0)).unwrap();
        let id1 = p1["id"].as_u64().unwrap() as u32;
        let result =
            sketch_add_constraint_impl("unknown_constraint", &format!("[{id1}]"), 0.0, 0.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown constraint"));
    }

    #[test]
    fn test_sketch_add_constraint_invalid_ids_json() {
        sketch_reset();
        let result = sketch_add_constraint_impl("distance", "not json", 0.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_invalid_svg_content() {
        let result = process_svg_impl("<svg></svg>", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_svg_laser_rejects_invalid_strategy() {
        let config_json = r#"{"machine_type": "laser_cutter", "strategy": "zigzag"}"#;
        let result = process_svg_impl(simple_svg(), config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_stl_invalid_stl_data() {
        let result = process_stl_impl(b"not stl data", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_laser_params_from_config_defaults() {
        // Laser config with no optional fields
        let config = CamConfig {
            machine_type: "laser_cutter".into(),
            ..CamConfig::default()
        };
        let lp = laser_params_from_config(&config).unwrap();
        assert!((lp.power - 100.0).abs() < f64::EPSILON);
        assert_eq!(lp.passes, 1);
        assert!(!lp.air_assist);
    }
}
