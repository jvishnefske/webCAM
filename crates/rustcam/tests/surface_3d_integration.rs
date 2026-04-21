//! End-to-end integration test for the 3-D surface milling strategy.
//!
//! Assembles a binary-STL pyramid in memory, parses it via
//! `rustcam::stl::parse_stl`, drives [`rustcam::toolpath::Surface3dStrategy`]
//! across the three traversal patterns and two tool types, and checks both
//! the structural properties of the emitted toolpaths and the resulting
//! G-code produced by `rustcam::gcode::emit_gcode`.
//!
//! The goals of this file are:
//! - Confirm the full pipeline (STL → mesh → strategy → G-code) holds
//!   together for the new `surface3d` entry point.
//! - Guard against gouging: the disc-projected tool-center Z must reach
//!   the pyramid apex but stay above the mesh base.
//! - Confirm rapid-count contracts for each pattern.

use rustcam::geometry::Toolpath;
use rustcam::toolpath::{CutParams, Pattern, ScanDirection, Surface3dStrategy, SurfaceParams};
use rustcam::{
    gcode::{emit_gcode, GcodeParams},
    stl::parse_stl,
    tool::{Tool, ToolType},
};

const APEX_Z: f64 = 7.0;

/// Build a binary STL in memory for a 10×10 base pyramid rising from z=0
/// to an apex at (5, 5, 7) across four triangular walls.
fn pyramid_stl() -> Vec<u8> {
    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&4u32.to_le_bytes());
    let push_tri = |bytes: &mut Vec<u8>, n: [f32; 3], v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]| {
        for f in n.iter().chain(v0.iter()).chain(v1.iter()).chain(v2.iter()) {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        bytes.extend_from_slice(&[0u8, 0]); // attribute bytes
    };
    let apex = [5.0_f32, 5.0, 7.0];
    let c1 = [0.0_f32, 0.0, 0.0];
    let c2 = [10.0_f32, 0.0, 0.0];
    let c3 = [10.0_f32, 10.0, 0.0];
    let c4 = [0.0_f32, 10.0, 0.0];
    let n = [0.0_f32, 0.0, 1.0];
    push_tri(&mut bytes, n, c1, c2, apex);
    push_tri(&mut bytes, n, c2, c3, apex);
    push_tri(&mut bytes, n, c3, c4, apex);
    push_tri(&mut bytes, n, c4, c1, apex);
    bytes
}

fn run_strategy(tool: Tool, pattern: Pattern) -> (Vec<Toolpath>, String) {
    let bytes = pyramid_stl();
    let mesh = parse_stl(&bytes).expect("pyramid STL must parse");
    let cut_params = CutParams {
        tool,
        step_over: 1.0,
        safe_z: 10.0,
        ..CutParams::default()
    };
    let surface =
        SurfaceParams::new_with_pattern(&mesh, cut_params.clone(), ScanDirection::X, pattern);
    let toolpaths = Surface3dStrategy.generate_surface(&surface);
    let gcode_params = GcodeParams {
        feed_rate: cut_params.feed_rate,
        plunge_rate: cut_params.plunge_rate,
        spindle_speed: 12000.0,
        safe_z: cut_params.safe_z,
        unit_mm: true,
    };
    let gcode = emit_gcode(&toolpaths, &gcode_params);
    (toolpaths, gcode)
}

fn count_rapids(toolpaths: &[Toolpath]) -> usize {
    toolpaths
        .iter()
        .flat_map(|tp| tp.moves.iter())
        .filter(|m| m.rapid)
        .count()
}

fn row_count(toolpaths: &[Toolpath]) -> usize {
    toolpaths.len()
}

/// Maximum Z across all non-rapid (cutting) moves.
fn max_cut_z(toolpaths: &[Toolpath]) -> f64 {
    toolpaths
        .iter()
        .flat_map(|tp| tp.moves.iter())
        .filter(|m| !m.rapid)
        .map(|m| m.z)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Minimum Z across all non-rapid (cutting) moves.
fn min_cut_z(toolpaths: &[Toolpath]) -> f64 {
    toolpaths
        .iter()
        .flat_map(|tp| tp.moves.iter())
        .filter(|m| !m.rapid)
        .map(|m| m.z)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn test_req_007_pyramid_stl_parses() {
    let bytes = pyramid_stl();
    let mesh = parse_stl(&bytes).expect("STL must parse");
    assert_eq!(mesh.triangles.len(), 4);
    let bounds = mesh.bounds.expect("pyramid must have bounds");
    assert!((bounds.min.x - 0.0).abs() < 1e-6);
    assert!((bounds.max.x - 10.0).abs() < 1e-6);
    assert!((bounds.max.z - APEX_Z).abs() < 1e-6);
}

#[test]
fn test_req_007_endmill_zigzag_reaches_apex_without_gouging() {
    let tool = Tool::new(ToolType::EndMill, 2.0, 10.0, 0.0); // radius 1.0
    let (toolpaths, gcode) = run_strategy(tool, Pattern::ZigZag);
    assert!(!gcode.is_empty());
    assert!(gcode.contains("G1"));
    // Flat disc projection reaches the apex exactly.
    assert!((max_cut_z(&toolpaths) - APEX_Z).abs() < 1e-6);
    // And never drops below the pyramid base (z = 0).
    assert!(
        min_cut_z(&toolpaths) >= 0.0 - 1e-6,
        "flat mill must not cut below the mesh base; got {}",
        min_cut_z(&toolpaths)
    );
    // Zig-zag emits exactly 2 rapids across all toolpaths (first plunge
    // + final retract).
    assert_eq!(
        count_rapids(&toolpaths),
        2,
        "zig-zag should emit exactly two rapids"
    );
}

#[test]
fn test_req_007_ballend_zigzag_lifts_apex_by_radius() {
    let r = 1.0;
    let tool = Tool::new(ToolType::BallEnd, 2.0 * r, 10.0, r);
    let (toolpaths, gcode) = run_strategy(tool, Pattern::ZigZag);
    assert!(!gcode.is_empty());
    // Sphere sits tangent on the apex: tool center Z = apex + r.
    assert!(
        (max_cut_z(&toolpaths) - (APEX_Z + r)).abs() < 1e-6,
        "ball-end max Z should be apex + radius ({}), got {}",
        APEX_Z + r,
        max_cut_z(&toolpaths)
    );
    // Ball on the base plane (z=0) sits at z=r, so min Z >= r (tolerance
    // for floating point).
    assert!(min_cut_z(&toolpaths) >= r - 1e-6);
}

#[test]
fn test_req_007_one_way_rapid_contract() {
    // One-way emits plunge + retract per row: 2 rapids per row.
    let tool = Tool::new(ToolType::EndMill, 2.0, 10.0, 0.0);
    let (toolpaths, gcode) = run_strategy(tool, Pattern::OneWay);
    assert!(!gcode.is_empty());
    let rows = row_count(&toolpaths);
    assert!(rows >= 3, "pyramid at step_over=1 should yield >=3 rows");
    assert_eq!(count_rapids(&toolpaths), rows * 2);
    // No gouging: min Z still above base.
    assert!(min_cut_z(&toolpaths) >= 0.0 - 1e-6);
}

#[test]
fn test_req_007_spiral_single_plunge_single_retract() {
    let tool = Tool::new(ToolType::EndMill, 2.0, 10.0, 0.0);
    let (toolpaths, gcode) = run_strategy(tool, Pattern::Spiral);
    assert!(!gcode.is_empty());
    assert_eq!(toolpaths.len(), 1, "spiral emits a single toolpath");
    assert_eq!(
        count_rapids(&toolpaths),
        2,
        "spiral emits exactly 1 initial rapid + 1 final retract"
    );
    // First rapid is the initial plunge; last rapid is the final retract.
    let moves = &toolpaths[0].moves;
    assert!(moves.first().unwrap().rapid);
    assert!(moves.last().unwrap().rapid);
}

#[test]
fn test_req_007_ballend_spiral_non_empty_gcode() {
    let r = 1.0;
    let tool = Tool::new(ToolType::BallEnd, 2.0 * r, 10.0, r);
    let (toolpaths, gcode) = run_strategy(tool, Pattern::Spiral);
    assert!(!gcode.is_empty());
    assert!(gcode.contains("G1"));
    assert!(!toolpaths.is_empty());
}
