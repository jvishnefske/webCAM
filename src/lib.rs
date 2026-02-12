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
pub mod tool;
pub mod toolpath;

use gcode::{emit_gcode, GcodeParams};
use geometry::Toolpath;
use serde::{Deserialize, Serialize};
use tool::Tool;
use toolpath::{
    ContourStrategy, CutParams, PerimeterStrategy, PocketStrategy, ScanDirection, SurfaceParams,
    ToolpathStrategy, ZigzagSurfaceStrategy,
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
fn default_strategy() -> String {
    "contour".into()
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
        }
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

// ── WASM entry points ────────────────────────────────────────────────

/// Process an STL file (binary bytes) and return G-code.
#[wasm_bindgen]
pub fn process_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
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
            let surface_params = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
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

    Ok(emit_gcode(&toolpaths, &gcode_params))
}

/// Process an SVG string and return G-code.
#[wasm_bindgen]
pub fn process_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
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
        let surface_params = SurfaceParams::new(mesh, cut_params, ScanDirection::X);
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
    let strategy: Box<dyn ToolpathStrategy> = match config.strategy.as_str() {
        "pocket" => Box::new(PocketStrategy),
        "perimeter" => Box::new(PerimeterStrategy),
        _ => Box::new(ContourStrategy),
    };
    let mut all = Vec::new();
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
    all
}

fn flatten_moves(toolpaths: &[Toolpath]) -> Result<String, JsValue> {
    let moves: Vec<&geometry::ToolpathMove> =
        toolpaths.iter().flat_map(|tp| tp.moves.iter()).collect();
    serde_json::to_string(&moves).map_err(|e| JsValue::from_str(&e.to_string()))
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
}
