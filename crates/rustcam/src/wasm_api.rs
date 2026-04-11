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
    super::process_stl_impl(data, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn process_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    super::process_svg_impl(svg_text, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn process_stl_progress(
    data: &[u8],
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    use super::*;

    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mesh = stl::parse_stl(data).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = toolpath::CutParams {
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

    let gcode_params = gcode::GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let toolpaths: Vec<geometry::Toolpath> = match config.strategy.as_str() {
        "zigzag" => {
            report_progress(on_progress, 0, 1);
            let strategy = toolpath::ZigzagSurfaceStrategy;
            let surface_params =
                toolpath::SurfaceParams::new(&mesh, cut_params, scan_direction_from_config(&config));
            let result = strategy.generate_surface(&surface_params);
            report_progress(on_progress, 1, 1);
            result
        }
        other => {
            let layers = slicer::slice_mesh(&mesh, config.step_down);
            let total = layers.len() as u32;
            report_progress(on_progress, 0, total);
            let strategy: Box<dyn toolpath::ToolpathStrategy> = match other {
                "pocket" => Box::new(toolpath::PocketStrategy),
                "perimeter" => Box::new(toolpath::PerimeterStrategy),
                "slice" => Box::new(toolpath::ContourStrategy),
                _ => Box::new(toolpath::ContourStrategy),
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

    Ok(gcode::emit_gcode(&toolpaths, &gcode_params))
}

#[wasm_bindgen]
pub fn process_svg_progress(
    svg_text: &str,
    config_json: &str,
    on_progress: &Function,
) -> Result<String, JsValue> {
    use super::*;

    let config: CamConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let polylines = svg::parse_svg(svg_text).map_err(|e| JsValue::from_str(&e))?;

    let cut_params = toolpath::CutParams {
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

    let gcode_params = gcode::GcodeParams {
        feed_rate: config.feed_rate,
        plunge_rate: config.plunge_rate,
        spindle_speed: config.spindle_speed,
        safe_z: config.safe_z,
        unit_mm: true,
    };

    let strategy: Box<dyn toolpath::ToolpathStrategy> = match config.strategy.as_str() {
        "pocket" => Box::new(toolpath::PocketStrategy),
        "perimeter" => Box::new(toolpath::PerimeterStrategy),
        _ => Box::new(toolpath::ContourStrategy),
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

    Ok(gcode::emit_gcode(&all_toolpaths, &gcode_params))
}

/// Helper: call a JS progress callback with (completed, total).
fn report_progress(cb: &Function, completed: u32, total: u32) {
    let _ = cb.call2(
        &JsValue::NULL,
        &JsValue::from(completed),
        &JsValue::from(total),
    );
}

// ── Preview ────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn preview_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    super::preview_stl_impl(data, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn preview_svg(svg_text: &str) -> Result<String, JsValue> {
    super::preview_svg_impl(svg_text).map_err(|e| JsValue::from_str(&e))
}

// ── Simulation data ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn sim_moves_stl(data: &[u8], config_json: &str) -> Result<String, JsValue> {
    super::sim_moves_stl_impl(data, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn sim_moves_svg(svg_text: &str, config_json: &str) -> Result<String, JsValue> {
    super::sim_moves_svg_impl(svg_text, config_json).map_err(|e| JsValue::from_str(&e))
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
    super::sketch_add_constraint_impl(kind, ids_json, value, value2)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn sketch_remove_constraint(id: u32) {
    super::sketch_remove_constraint(id)
}

#[wasm_bindgen]
pub fn sketch_solve() -> Result<String, JsValue> {
    super::sketch_solve_impl().map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn sketch_pump() -> Result<String, JsValue> {
    super::sketch_pump_impl().map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn sketch_snapshot() -> Result<String, JsValue> {
    super::sketch_snapshot_impl().map_err(|e| JsValue::from_str(&e))
}
