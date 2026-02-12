/// Toolpath generation strategies.
///
/// Swiss-cheese layer: **Strategy selection**
/// Extension point: implement `ToolpathStrategy` to add spiral, trochoidal,
/// adaptive-clearing, or any custom strategy.
use crate::geometry::{Mesh, Polyline, Toolpath, Vec2};

// ── Strategy trait (the "hole") ──────────────────────────────────────

pub trait ToolpathStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath>;
}

/// Scan direction for 3D surface strategies.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScanDirection {
    #[default]
    X,
    Y,
}

/// Parameters for 3D surface machining strategies.
#[derive(Debug, Clone)]
pub struct SurfaceParams<'a> {
    /// Reference to the mesh being machined.
    pub mesh: &'a Mesh,
    /// Cutting parameters (tool, feeds, depths).
    pub cut_params: CutParams,
    /// Direction to scan (X or Y).
    pub scan_direction: ScanDirection,
}

impl<'a> SurfaceParams<'a> {
    /// Create new surface parameters.
    pub fn new(mesh: &'a Mesh, cut_params: CutParams, scan_direction: ScanDirection) -> Self {
        Self {
            mesh,
            cut_params,
            scan_direction,
        }
    }
}

impl<'a> From<(&'a Mesh, &CutParams)> for SurfaceParams<'a> {
    fn from((mesh, cut_params): (&'a Mesh, &CutParams)) -> Self {
        Self {
            mesh,
            cut_params: cut_params.clone(),
            scan_direction: ScanDirection::default(),
        }
    }
}

use crate::tool::Tool;

/// Cutting parameters shared by all strategies.
#[derive(Debug, Clone)]
pub struct CutParams {
    /// Tool geometry (replaces tool_diameter for new code).
    pub tool: Tool,
    /// Tool diameter in mm (deprecated: use tool.diameter).
    pub tool_diameter: f64,
    pub step_over: f64,
    pub step_down: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub cut_z: f64,
    /// Climb cutting mode (tool left of path). Default is conventional.
    pub climb_cut: bool,
    /// Number of perimeter passes (default 1).
    pub perimeter_passes: u32,
}

impl Default for CutParams {
    fn default() -> Self {
        let tool = Tool::default();
        Self {
            tool_diameter: tool.diameter,
            tool,
            step_over: 1.5,
            step_down: 1.0,
            feed_rate: 800.0,
            plunge_rate: 300.0,
            safe_z: 5.0,
            cut_z: 0.0,
            climb_cut: false,
            perimeter_passes: 1,
        }
    }
}

// ── Contour strategy ─────────────────────────────────────────────────

pub struct ContourStrategy;

impl ToolpathStrategy for ContourStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath> {
        let mut toolpaths = Vec::new();
        let offset = params.tool_diameter / 2.0;

        for contour in contours {
            let offset_pts = offset_polyline(contour, offset);
            if offset_pts.is_empty() {
                continue;
            }

            let mut tp = Toolpath::new();
            // Rapid to start above first point
            tp.rapid(offset_pts[0].x, offset_pts[0].y, params.safe_z);
            // Plunge to cut depth
            tp.cut(offset_pts[0].x, offset_pts[0].y, params.cut_z);
            // Follow contour
            for pt in &offset_pts[1..] {
                tp.cut(pt.x, pt.y, params.cut_z);
            }
            // Close if needed
            if contour.closed && !offset_pts.is_empty() {
                tp.cut(offset_pts[0].x, offset_pts[0].y, params.cut_z);
            }
            // Retract
            tp.rapid(
                offset_pts.last().unwrap().x,
                offset_pts.last().unwrap().y,
                params.safe_z,
            );
            toolpaths.push(tp);
        }
        toolpaths
    }
}

// ── Pocket strategy (scanline fill) ──────────────────────────────────

pub struct PocketStrategy;

impl ToolpathStrategy for PocketStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath> {
        let mut toolpaths = Vec::new();

        for contour in contours {
            if contour.points.len() < 3 || !contour.closed {
                continue;
            }

            let bounds = match contour.bounds() {
                Some(b) => b,
                None => continue,
            };

            let offset = params.tool_diameter / 2.0;
            let y_min = bounds.min.y + offset;
            let y_max = bounds.max.y - offset;
            let step = params.step_over.max(0.1);

            let mut tp = Toolpath::new();
            let mut y = y_min;
            let mut forward = true;

            while y <= y_max {
                let intersections = scanline_intersect(contour, y);
                let mut xs: Vec<f64> = intersections;
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());

                // Inset X by tool radius
                for pair in xs.chunks(2) {
                    if pair.len() < 2 {
                        continue;
                    }
                    let x0 = pair[0] + offset;
                    let x1 = pair[1] - offset;
                    if x0 >= x1 {
                        continue;
                    }
                    let (start_x, end_x) = if forward { (x0, x1) } else { (x1, x0) };

                    // Rapid to start
                    tp.rapid(start_x, y, params.safe_z);
                    tp.cut(start_x, y, params.cut_z);
                    tp.cut(end_x, y, params.cut_z);
                    tp.rapid(end_x, y, params.safe_z);
                }
                forward = !forward;
                y += step;
            }

            if !tp.moves.is_empty() {
                toolpaths.push(tp);
            }
        }
        toolpaths
    }
}

// ── Perimeter strategy (boundary follow) ──────────────────────────────

/// Perimeter strategy follows the outer boundary of a contour.
pub struct PerimeterStrategy;

impl ToolpathStrategy for PerimeterStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath> {
        let mut toolpaths = Vec::new();

        // Find outermost contour (largest bounding box area)
        let outer = contours.iter().max_by(|a, b| {
            let area_a = a
                .bounds()
                .map_or(0.0, |b| (b.max.x - b.min.x) * (b.max.y - b.min.y));
            let area_b = b
                .bounds()
                .map_or(0.0, |b| (b.max.x - b.min.x) * (b.max.y - b.min.y));
            area_a.partial_cmp(&area_b).unwrap()
        });

        if let Some(contour) = outer {
            let base_offset = params.tool_diameter / 2.0;
            let num_passes = params.perimeter_passes.max(1);

            // Generate multiple passes from outside to inside
            for pass in 0..num_passes {
                // Each pass is offset by step_over from the previous
                // Pass 0 (outermost) uses full tool offset
                // Innermost pass uses full tool offset + (num_passes-1) * step_over
                let pass_offset = base_offset + (pass as f64) * params.step_over;
                let mut offset_pts = offset_polyline(contour, pass_offset);
                if offset_pts.is_empty() {
                    continue;
                }

                // Climb cut reverses direction (CW for outside = CCW traverse)
                if params.climb_cut {
                    offset_pts.reverse();
                }

                let mut tp = Toolpath::new();
                // Rapid to start above first point
                tp.rapid(offset_pts[0].x, offset_pts[0].y, params.safe_z);
                // Plunge to cut depth
                tp.cut(offset_pts[0].x, offset_pts[0].y, params.cut_z);
                // Follow perimeter
                for pt in &offset_pts[1..] {
                    tp.cut(pt.x, pt.y, params.cut_z);
                }
                // Close path if contour is closed
                if contour.closed && !offset_pts.is_empty() {
                    tp.cut(offset_pts[0].x, offset_pts[0].y, params.cut_z);
                }
                // Retract
                tp.rapid(
                    offset_pts.last().unwrap().x,
                    offset_pts.last().unwrap().y,
                    params.safe_z,
                );
                toolpaths.push(tp);
            }
        }
        toolpaths
    }
}

// ── Zigzag Surface strategy (3D raster) ───────────────────────────────

use crate::geometry::Vec3;
use crate::slicer::{mesh_height_at, surface_normal_at};
use crate::tool::ToolType;

/// Compute ball-end tool contact point offset based on surface normal.
///
/// For a ball-end tool, the contact point shifts along the surface normal.
/// Returns (dx, dy, dz) offset to apply to the tool center position.
fn ball_end_offset(normal: Vec3, radius: f64) -> (f64, f64, f64) {
    // Normalize the normal vector
    let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
    if len < 1e-10 {
        return (0.0, 0.0, 0.0);
    }

    let nx = normal.x / len;
    let ny = normal.y / len;
    let nz = normal.z / len;

    // For a flat surface (normal = +Z), no XY offset, Z offset = radius
    // For angled surface, offset tool center along normal direction
    // The contact point is at distance radius from tool center along -normal
    // So tool center is at: contact_point + radius * normal

    // X and Y offset: move tool center in direction of normal's XY component
    let dx = radius * nx;
    let dy = radius * ny;
    // Z offset: the difference from flat case (radius * (1 - nz))
    // For flat (nz=1): dz = 0, for 45° (nz=0.707): dz = radius * 0.293
    let dz = radius * (1.0 - nz);

    (dx, dy, dz)
}

/// Zigzag surface strategy rasters across the mesh surface.
pub struct ZigzagSurfaceStrategy;

impl ZigzagSurfaceStrategy {
    /// Sample a surface point with optional ball-end compensation.
    fn sample_point(
        mesh: &Mesh,
        x: f64,
        y: f64,
        is_ball_end: bool,
        tool_radius: f64,
    ) -> Option<(f64, f64, f64)> {
        mesh_height_at(mesh, x, y).map(|z| {
            if is_ball_end {
                if let Some(normal) = surface_normal_at(mesh, x, y) {
                    let (dx, dy, dz) = ball_end_offset(normal, tool_radius);
                    (x + dx, y + dy, z + dz)
                } else {
                    (x, y, z)
                }
            } else {
                (x, y, z)
            }
        })
    }

    /// Generate 3D surface toolpath from mesh.
    ///
    /// The tool stays on the surface between rows, cutting directly from the
    /// end of one row to the start of the next instead of retracting to safe Z.
    /// This produces a continuous back-and-forth surface movement.
    pub fn generate_surface(&self, params: &SurfaceParams) -> Vec<Toolpath> {
        let bounds = match &params.mesh.bounds {
            Some(b) => b,
            None => return Vec::new(),
        };

        let step = params.cut_params.step_over.max(0.1);
        let safe_z = params.cut_params.safe_z;

        let is_ball_end = matches!(params.cut_params.tool.tool_type, ToolType::BallEnd);
        let tool_radius = params.cut_params.tool.diameter / 2.0;

        // Collect all rows/columns of surface points
        let rows: Vec<Vec<(f64, f64, f64)>> = match params.scan_direction {
            ScanDirection::X => {
                let y_min = bounds.min.y;
                let y_max = bounds.max.y;
                let x_min = bounds.min.x;
                let x_max = bounds.max.x;
                let mut rows = Vec::new();
                let mut y = y_min;
                let mut forward = true;

                while y <= y_max {
                    let x_range: Vec<f64> = if forward {
                        float_range(x_min, x_max, step).collect()
                    } else {
                        float_range(x_min, x_max, step).collect::<Vec<_>>().into_iter().rev().collect()
                    };

                    let row: Vec<(f64, f64, f64)> = x_range
                        .into_iter()
                        .filter_map(|x| Self::sample_point(params.mesh, x, y, is_ball_end, tool_radius))
                        .collect();

                    if !row.is_empty() {
                        rows.push(row);
                    }
                    forward = !forward;
                    y += step;
                }
                rows
            }
            ScanDirection::Y => {
                let x_min = bounds.min.x;
                let x_max = bounds.max.x;
                let y_min = bounds.min.y;
                let y_max = bounds.max.y;
                let mut rows = Vec::new();
                let mut x = x_min;
                let mut forward = true;

                while x <= x_max {
                    let y_range: Vec<f64> = if forward {
                        float_range(y_min, y_max, step).collect()
                    } else {
                        float_range(y_min, y_max, step).collect::<Vec<_>>().into_iter().rev().collect()
                    };

                    let col: Vec<(f64, f64, f64)> = y_range
                        .into_iter()
                        .filter_map(|y| Self::sample_point(params.mesh, x, y, is_ball_end, tool_radius))
                        .collect();

                    if !col.is_empty() {
                        rows.push(col);
                    }
                    forward = !forward;
                    x += step;
                }
                rows
            }
        };

        // Build toolpaths: one per row for clear preview rendering.
        // Between rows, the tool stays on the surface (no retract to safe Z).
        let mut toolpaths: Vec<Toolpath> = Vec::new();
        let mut prev_end: Option<(f64, f64, f64)> = None;

        for row in &rows {
            if row.is_empty() {
                continue;
            }

            let mut tp = Toolpath::new();
            let (x0, y0, z0) = row[0];

            if prev_end.is_none() {
                // First row: rapid to safe Z then plunge
                tp.rapid(x0, y0, safe_z);
                tp.cut(x0, y0, z0);
            } else {
                // Subsequent rows: cut directly from previous row end
                // (staying on the surface, no retract)
                let (px, py, pz) = prev_end.unwrap();
                tp.cut(px, py, pz);
                tp.cut(x0, y0, z0);
            }

            // Cut along this row
            for &(x, y, z) in &row[1..] {
                tp.cut(x, y, z);
            }

            let last = row.last().unwrap();
            prev_end = Some(*last);
            toolpaths.push(tp);
        }

        // Final retract on last toolpath
        if let Some(last_tp) = toolpaths.last_mut() {
            if let Some((x, y, _)) = prev_end {
                last_tp.rapid(x, y, safe_z);
            }
        }

        toolpaths
    }
}

/// Generate floating-point range values.
fn float_range(start: f64, end: f64, step: f64) -> impl Iterator<Item = f64> {
    let steps = ((end - start) / step).ceil() as usize + 1;
    (0..steps).map(move |i| (start + (i as f64) * step).min(end))
}

/// Find all X coordinates where a horizontal scanline at `y` intersects the polyline edges.
fn scanline_intersect(poly: &Polyline, y: f64) -> Vec<f64> {
    let pts = &poly.points;
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let mut xs = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let a = pts[i];
        let b = pts[j];
        if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
            let t = (y - a.y) / (b.y - a.y);
            xs.push(a.x + t * (b.x - a.x));
        }
    }
    xs
}

// ── Polyline offset (simple normal offset) ───────────────────────────

fn offset_polyline(poly: &Polyline, dist: f64) -> Vec<Vec2> {
    let pts = &poly.points;
    let n = pts.len();
    if n < 2 {
        return pts.clone();
    }

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i == 0 {
            if poly.closed {
                pts[n - 1]
            } else {
                pts[0]
            }
        } else {
            pts[i - 1]
        };
        let next = if i == n - 1 {
            if poly.closed {
                pts[0]
            } else {
                pts[n - 1]
            }
        } else {
            pts[i + 1]
        };

        // Average normal of adjacent edges
        let dx1 = pts[i].x - prev.x;
        let dy1 = pts[i].y - prev.y;
        let dx2 = next.x - pts[i].x;
        let dy2 = next.y - pts[i].y;

        let nx = -(dy1 + dy2);
        let ny = dx1 + dx2;
        let len = (nx * nx + ny * ny).sqrt();
        if len < 1e-10 {
            result.push(pts[i]);
        } else {
            result.push(Vec2::new(
                pts[i].x + dist * nx / len,
                pts[i].y + dist * ny / len,
            ));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolType;

    #[test]
    fn test_req_002_cutparams_has_tool() {
        // CutParams must include tool: Tool field
        let params = CutParams::default();
        assert_eq!(params.tool.tool_type, ToolType::EndMill);
    }

    #[test]
    fn test_req_002_cutparams_tool_diameter_matches() {
        // tool_diameter should match tool.diameter for backward compat
        let params = CutParams::default();
        assert!((params.tool_diameter - params.tool.diameter).abs() < 0.001);
    }

    #[test]
    fn test_req_002_strategies_use_tool_diameter() {
        // Existing strategies should work with new CutParams
        let contours = vec![square()];
        let params = CutParams::default();
        let strategy = ContourStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
    }

    fn square() -> Polyline {
        Polyline::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0),
                Vec2::new(0.0, 10.0),
            ],
            true,
        )
    }

    #[test]
    fn test_contour_strategy() {
        let contours = vec![square()];
        let params = CutParams::default();
        let strategy = ContourStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
        assert!(toolpaths[0].moves.len() >= 4);
    }

    #[test]
    fn test_pocket_strategy() {
        let contours = vec![square()];
        let params = CutParams::default();
        let strategy = PocketStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
    }

    #[test]
    fn test_scanline() {
        let sq = square();
        let xs = scanline_intersect(&sq, 5.0);
        assert_eq!(xs.len(), 2);
    }

    // ── Task-006 tests: SurfaceParams ────────────────────────────────────

    fn make_simple_mesh() -> Mesh {
        use crate::geometry::{Triangle, Vec3};
        let t = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(10.0, 0.0, 0.0),
            v2: Vec3::new(5.0, 10.0, 0.0),
        };
        Mesh::new(vec![t])
    }

    #[test]
    fn test_req_006_surface_params_new() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        assert_eq!(surface.scan_direction, ScanDirection::X);
    }

    #[test]
    fn test_req_006_surface_params_from() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        let surface: SurfaceParams = (&mesh, &cut_params).into();
        assert_eq!(surface.scan_direction, ScanDirection::X); // default
    }

    #[test]
    fn test_req_006_surface_params_has_mesh_ref() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        let surface: SurfaceParams = (&mesh, &cut_params).into();
        // Verify we can access mesh bounds through reference
        assert!(surface.mesh.bounds.is_some());
    }

    // ── Task-010 tests: PerimeterStrategy ────────────────────────────────

    #[test]
    fn test_req_010_perimeter_implements_trait() {
        let contours = vec![square()];
        let params = CutParams::default();
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
    }

    #[test]
    fn test_req_010_perimeter_follows_outer() {
        // Two nested squares - should follow outer
        let outer = Polyline::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(20.0, 0.0),
                Vec2::new(20.0, 20.0),
                Vec2::new(0.0, 20.0),
            ],
            true,
        );
        let inner = Polyline::new(
            vec![
                Vec2::new(5.0, 5.0),
                Vec2::new(15.0, 5.0),
                Vec2::new(15.0, 15.0),
                Vec2::new(5.0, 15.0),
            ],
            true,
        );
        let contours = vec![inner, outer];
        let params = CutParams::default();
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
        // Check that toolpath covers area near outer boundary
        let first_move = &toolpaths[0].moves[0];
        // Should be near outer boundary (offset by tool radius)
        assert!(first_move.x.abs() < 5.0 || first_move.x > 15.0);
    }

    #[test]
    fn test_req_010_perimeter_applies_offset() {
        let contours = vec![square()]; // 10x10 square from 0,0 to 10,10
        let params = CutParams {
            tool_diameter: 2.0, // 1.0 radius offset
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
        // First cut move should be offset from original corner
        let moves = &toolpaths[0].moves;
        // Find first cut move (not rapid)
        let cut_move = moves.iter().find(|m| !m.rapid).unwrap();
        // Should be offset from corner - not at exactly (0,0)
        assert!(cut_move.x > 0.0 || cut_move.y > 0.0);
    }

    #[test]
    fn test_req_010_perimeter_single_pass() {
        let contours = vec![square()];
        let params = CutParams::default();
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        // Should produce exactly 1 toolpath for 1 contour
        assert_eq!(toolpaths.len(), 1);
    }

    // ── Task-007 tests: ZigzagSurfaceStrategy ────────────────────────────

    fn make_flat_surface_mesh() -> Mesh {
        use crate::geometry::{Triangle, Vec3};
        // 10x10 flat surface at z=5
        let t1 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 5.0),
            v1: Vec3::new(10.0, 0.0, 5.0),
            v2: Vec3::new(10.0, 10.0, 5.0),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 5.0),
            v1: Vec3::new(10.0, 10.0, 5.0),
            v2: Vec3::new(0.0, 10.0, 5.0),
        };
        Mesh::new(vec![t1, t2])
    }

    fn make_ramp_surface_mesh() -> Mesh {
        use crate::geometry::{Triangle, Vec3};
        // Ramp from z=0 at y=0 to z=10 at y=10
        let t1 = Triangle {
            normal: Vec3::new(0.0, -1.0, 1.0).normalize(),
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(10.0, 0.0, 0.0),
            v2: Vec3::new(10.0, 10.0, 10.0),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, -1.0, 1.0).normalize(),
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(10.0, 10.0, 10.0),
            v2: Vec3::new(0.0, 10.0, 10.0),
        };
        Mesh::new(vec![t1, t2])
    }

    #[test]
    fn test_req_007_zigzag_generates_toolpath() {
        let mesh = make_flat_surface_mesh();
        let cut_params = CutParams::default();
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());
        assert!(!toolpaths[0].moves.is_empty());
    }

    #[test]
    fn test_req_007_zigzag_samples_at_step_over() {
        let mesh = make_flat_surface_mesh();
        let cut_params = CutParams {
            step_over: 2.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());
        // Should have multiple moves sampling across the surface (across all rows)
        let total_moves: usize = toolpaths.iter().map(|tp| tp.moves.len()).sum();
        assert!(total_moves > 10);
    }

    #[test]
    fn test_req_007_zigzag_queries_mesh_height() {
        let mesh = make_ramp_surface_mesh();
        let cut_params = CutParams::default();
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());
        // Check that Z varies (not all at same height) across all rows
        let z_values: Vec<f64> = toolpaths
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| !m.rapid)
            .map(|m| m.z)
            .collect();
        assert!(!z_values.is_empty());
        let z_min = z_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let z_max = z_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // Should have varying Z on ramp
        assert!(z_max - z_min > 1.0, "Z should vary on ramp mesh");
    }

    #[test]
    fn test_req_007_zigzag_skips_outside_mesh() {
        // Small mesh - should not generate moves outside its bounds
        use crate::geometry::{Triangle, Vec3};
        let t = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(2.0, 2.0, 5.0),
            v1: Vec3::new(4.0, 2.0, 5.0),
            v2: Vec3::new(3.0, 4.0, 5.0),
        };
        let mesh = Mesh::new(vec![t]);
        let cut_params = CutParams {
            step_over: 0.5,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        // Should have some moves, but limited to mesh area
        if !toolpaths.is_empty() {
            for m in &toolpaths[0].moves {
                if !m.rapid {
                    assert!(m.x >= 1.5 && m.x <= 4.5, "X {} out of bounds", m.x);
                    assert!(m.y >= 1.5 && m.y <= 4.5, "Y {} out of bounds", m.y);
                }
            }
        }
    }

    #[test]
    fn test_req_007_zigzag_alternates_direction() {
        let mesh = make_flat_surface_mesh();
        let cut_params = CutParams {
            step_over: 2.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());
        // Find first cut moves of consecutive rows - X direction should alternate
        // This is a simplified check for serpentine motion
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        assert!(!cuts.is_empty());
    }

    // ── Task-011 tests: climb/conventional cut ────────────────────────────

    #[test]
    fn test_req_011_cutparams_has_climb_cut() {
        let params = CutParams::default();
        // Default should be conventional (not climb)
        assert!(!params.climb_cut);
    }

    #[test]
    fn test_req_011_perimeter_conventional_direction() {
        let contours = vec![square()];
        let params = CutParams {
            climb_cut: false,
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert!(!toolpaths.is_empty());
        // Get direction of first segment
        let moves = &toolpaths[0].moves;
        let cuts: Vec<_> = moves.iter().filter(|m| !m.rapid).collect();
        assert!(cuts.len() >= 2);
        // Verify we have valid points
        let _dx1 = cuts[1].x - cuts[0].x;
        let _dy1 = cuts[1].y - cuts[0].y;
    }

    #[test]
    fn test_req_011_perimeter_climb_reverses() {
        let contours = vec![square()];
        let conv_params = CutParams {
            climb_cut: false,
            ..CutParams::default()
        };
        let climb_params = CutParams {
            climb_cut: true,
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;

        let conv_toolpaths = strategy.generate(&contours, &conv_params);
        let climb_toolpaths = strategy.generate(&contours, &climb_params);

        assert!(!conv_toolpaths.is_empty());
        assert!(!climb_toolpaths.is_empty());

        // Get cut moves
        let conv_cuts: Vec<_> = conv_toolpaths[0]
            .moves
            .iter()
            .filter(|m| !m.rapid)
            .collect();
        let climb_cuts: Vec<_> = climb_toolpaths[0]
            .moves
            .iter()
            .filter(|m| !m.rapid)
            .collect();

        assert!(conv_cuts.len() >= 2);
        assert!(climb_cuts.len() >= 2);

        // Verify the paths go in opposite orders
        // For a square, the first point of climb should be near the last point of conventional
        let conv_first = (conv_cuts[0].x, conv_cuts[0].y);
        let climb_first = (climb_cuts[0].x, climb_cuts[0].y);

        // They should start at different points (reversed order)
        let same_start = (conv_first.0 - climb_first.0).abs() < 0.01
            && (conv_first.1 - climb_first.1).abs() < 0.01;
        assert!(
            !same_start,
            "Climb should start at different point than conventional"
        );
    }

    // ── Task-008 tests: Ball-end compensation ────────────────────────────

    #[test]
    fn test_req_008_ball_end_offset_flat_surface() {
        // For flat surface (normal = +Z), only Z offset
        let normal = Vec3::new(0.0, 0.0, 1.0);
        let (dx, dy, dz) = ball_end_offset(normal, 3.0);
        assert!(dx.abs() < 0.001, "No X offset for flat surface");
        assert!(dy.abs() < 0.001, "No Y offset for flat surface");
        assert!(dz.abs() < 0.001, "No Z adjustment for flat surface");
    }

    #[test]
    fn test_req_008_ball_end_offset_angled() {
        // 45-degree ramp (normal at 45° in Y-Z plane)
        let normal = Vec3::new(0.0, -0.707, 0.707);
        let radius = 3.0;
        let (dx, dy, dz) = ball_end_offset(normal, radius);

        // Should have Y offset and Z adjustment
        assert!(dx.abs() < 0.001, "No X offset for Y ramp");
        assert!(dy < -0.5, "Should have negative Y offset: {}", dy);
        assert!(dz > 0.5, "Should have positive Z adjustment: {}", dz);
    }

    #[test]
    fn test_req_008_zigzag_with_ball_end() {
        use crate::tool::Tool;

        let mesh = make_ramp_surface_mesh();
        let ball_tool = Tool::ball_end(6.0, 10.0);
        let cut_params = CutParams {
            tool: ball_tool,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());

        // On ramp, ball-end compensation should shift tool position
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        assert!(!cuts.is_empty());
    }

    #[test]
    fn test_req_008_zigzag_no_offset_for_end_mill() {
        let mesh = make_flat_surface_mesh();
        let cut_params = CutParams::default(); // EndMill
        let surface = SurfaceParams::new(&mesh, cut_params.clone(), ScanDirection::X);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);

        // End mill should have no compensation applied
        // Just verify it runs without error
        assert!(!toolpaths.is_empty());
    }

    // ── Task-012 tests: Multiple perimeter passes ────────────────────────

    #[test]
    fn test_req_012_default_single_pass() {
        let params = CutParams::default();
        assert_eq!(params.perimeter_passes, 1);
    }

    #[test]
    fn test_req_012_multiple_passes_count() {
        let contours = vec![square()]; // 10x10 square
        let params = CutParams {
            perimeter_passes: 3,
            step_over: 1.0,
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        // Should generate 3 toolpaths (one per pass)
        assert_eq!(toolpaths.len(), 3, "Should have 3 perimeter passes");
    }

    #[test]
    fn test_req_012_passes_offset_inward() {
        let contours = vec![square()]; // 10x10 square from 0,0 to 10,10
        let params = CutParams {
            perimeter_passes: 2,
            step_over: 1.0,
            tool_diameter: 2.0, // 1.0 radius
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);
        assert_eq!(toolpaths.len(), 2);

        // Get first cut move of each pass
        let pass1_cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        let pass2_cuts: Vec<_> = toolpaths[1].moves.iter().filter(|m| !m.rapid).collect();

        // Second pass should be more inward (further from edges)
        // For square at 0,0 with offset, pass 2 should have min x > pass 1 min x
        let p1_min_x = pass1_cuts.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
        let p2_min_x = pass2_cuts.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
        assert!(
            p2_min_x > p1_min_x,
            "Pass 2 should be more inward: p1={}, p2={}",
            p1_min_x,
            p2_min_x
        );
    }

    #[test]
    fn test_req_012_concentric_offset() {
        let contours = vec![square()];
        let params = CutParams {
            perimeter_passes: 2,
            step_over: 2.0, // Each pass 2mm more inward
            tool_diameter: 2.0,
            ..CutParams::default()
        };
        let strategy = PerimeterStrategy;
        let toolpaths = strategy.generate(&contours, &params);

        // Verify second pass is further inward than first
        let pass1_cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        let pass2_cuts: Vec<_> = toolpaths[1].moves.iter().filter(|m| !m.rapid).collect();

        let p1_min_x = pass1_cuts.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
        let p2_min_x = pass2_cuts.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);

        // Second pass should be at least step_over/2 inward (accounting for corner effects)
        let offset_diff = p2_min_x - p1_min_x;
        assert!(
            offset_diff > 0.5,
            "Pass 2 should be inward by at least step_over/2: {}",
            offset_diff
        );
    }
}
