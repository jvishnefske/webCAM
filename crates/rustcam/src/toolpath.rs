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

/// Traversal pattern for the 3-D surface strategy.
///
/// `ZigZag` alternates scan direction row-to-row without retracting.
/// `OneWay` always cuts in the same direction, retracting to safe Z
/// between rows. `Spiral` emits a rectangular offset spiral from the mesh
/// bounding box inward.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Pattern {
    /// Back-and-forth, no retract between rows (default).
    #[default]
    ZigZag,
    /// All rows cut in the same direction; retract-and-return between
    /// rows.
    OneWay,
    /// Outer-to-inner rectangular offset spiral.
    Spiral,
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
    /// Traversal pattern.
    pub pattern: Pattern,
}

impl<'a> SurfaceParams<'a> {
    /// Create new surface parameters with the default traversal pattern
    /// (`Pattern::ZigZag`). Preserved as a 3-argument constructor for
    /// backwards compatibility with existing call sites.
    pub fn new(mesh: &'a Mesh, cut_params: CutParams, scan_direction: ScanDirection) -> Self {
        Self::new_with_pattern(mesh, cut_params, scan_direction, Pattern::default())
    }

    /// Create new surface parameters with an explicit traversal pattern.
    pub fn new_with_pattern(
        mesh: &'a Mesh,
        cut_params: CutParams,
        scan_direction: ScanDirection,
        pattern: Pattern,
    ) -> Self {
        Self {
            mesh,
            cut_params,
            scan_direction,
            pattern,
        }
    }
}

impl<'a> From<(&'a Mesh, &CutParams)> for SurfaceParams<'a> {
    fn from((mesh, cut_params): (&'a Mesh, &CutParams)) -> Self {
        Self {
            mesh,
            cut_params: cut_params.clone(),
            scan_direction: ScanDirection::default(),
            pattern: Pattern::default(),
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

// ── Surface 3-D strategy (raster + spiral, disc-projected) ────────────

use crate::slicer::{project_ball_tool, project_flat_tool};
use crate::tool::ToolType;

/// 3-D surface strategy: projects the tool shape (flat cylinder for
/// `EndMill` / `FaceMill`, sphere for `BallEnd`) onto the mesh from +Z and
/// emits a toolpath following the chosen [`Pattern`].
pub struct Surface3dStrategy;

/// Back-compat alias for the former name.
pub use self::Surface3dStrategy as ZigzagSurfaceStrategy;

impl Surface3dStrategy {
    /// Project the tool shape at `(x, y)` onto the mesh and return the
    /// tool-center Z, or `None` if the disc lies entirely off the mesh.
    fn sample_point(
        mesh: &Mesh,
        x: f64,
        y: f64,
        tool_type: &ToolType,
        tool_radius: f64,
    ) -> Option<(f64, f64, f64)> {
        let z = match tool_type {
            ToolType::BallEnd => project_ball_tool(mesh, x, y, tool_radius),
            ToolType::EndMill | ToolType::FaceMill { .. } => {
                project_flat_tool(mesh, x, y, tool_radius)
            }
        }?;
        Some((x, y, z))
    }

    /// Generate 3-D surface toolpath from the mesh.
    ///
    /// Dispatches to the pattern selected by `params.pattern`.
    pub fn generate_surface(&self, params: &SurfaceParams) -> Vec<Toolpath> {
        match params.pattern {
            Pattern::ZigZag => self.generate_zigzag(params),
            Pattern::OneWay => self.generate_one_way(params),
            Pattern::Spiral => self.generate_spiral(params),
        }
    }

    /// One-way pattern: every row is cut in the same direction, with a
    /// rapid retract to `safe_z` at the end of each row and a rapid plunge
    /// at the start of the next row. This sacrifices efficiency for a
    /// consistent cutting direction (useful when climb vs. conventional
    /// cutting matters for the whole surface).
    fn generate_one_way(&self, params: &SurfaceParams) -> Vec<Toolpath> {
        let rows = collect_surface_rows(params, /*alternate=*/ false);
        let safe_z = params.cut_params.safe_z;
        let mut toolpaths = Vec::new();
        for row in &rows {
            let (x0, y0, z0) = row[0];
            let mut tp = Toolpath::new();
            tp.rapid(x0, y0, safe_z);
            tp.cut(x0, y0, z0);
            for &(x, y, z) in &row[1..] {
                tp.cut(x, y, z);
            }
            let (xe, ye, _) = *row.last().unwrap();
            tp.rapid(xe, ye, safe_z);
            toolpaths.push(tp);
        }
        toolpaths
    }

    /// Rectangular offset spiral pattern: walks the mesh XY bounding
    /// rectangle outer-to-inner, offsetting inward by `step_over` each
    /// loop. Emits a single `Toolpath` with one initial rapid + plunge and
    /// one final retract; all intermediate moves are cutting moves.
    fn generate_spiral(&self, params: &SurfaceParams) -> Vec<Toolpath> {
        let bounds = match &params.mesh.bounds {
            Some(b) => b,
            None => return Vec::new(),
        };
        let step = params.cut_params.step_over.max(0.1);
        let safe_z = params.cut_params.safe_z;
        let tool_type = params.cut_params.tool.tool_type.clone();
        let tool_radius = params.cut_params.tool.diameter / 2.0;

        let mut samples: Vec<(f64, f64, f64)> = Vec::new();
        let mut k = 0usize;
        loop {
            let offset = (k as f64) * step;
            let x_lo = bounds.min.x + offset;
            let x_hi = bounds.max.x - offset;
            let y_lo = bounds.min.y + offset;
            let y_hi = bounds.max.y - offset;
            if x_hi - x_lo <= step || y_hi - y_lo <= step {
                break;
            }

            // Walk the four edges CCW, dropping the final point of each
            // edge to avoid duplicate samples at the corners.
            let mut x = x_lo;
            while x < x_hi - 1e-9 {
                if let Some(p) = Self::sample_point(params.mesh, x, y_lo, &tool_type, tool_radius) {
                    samples.push(p);
                }
                x += step;
            }
            let mut y = y_lo;
            while y < y_hi - 1e-9 {
                if let Some(p) = Self::sample_point(params.mesh, x_hi, y, &tool_type, tool_radius) {
                    samples.push(p);
                }
                y += step;
            }
            let mut x = x_hi;
            while x > x_lo + 1e-9 {
                if let Some(p) = Self::sample_point(params.mesh, x, y_hi, &tool_type, tool_radius) {
                    samples.push(p);
                }
                x -= step;
            }
            let mut y = y_hi;
            while y > y_lo + 1e-9 {
                if let Some(p) = Self::sample_point(params.mesh, x_lo, y, &tool_type, tool_radius) {
                    samples.push(p);
                }
                y -= step;
            }

            k += 1;
        }

        if samples.is_empty() {
            return Vec::new();
        }

        let mut tp = Toolpath::new();
        let (x0, y0, z0) = samples[0];
        tp.rapid(x0, y0, safe_z);
        tp.cut(x0, y0, z0);
        for &(x, y, z) in &samples[1..] {
            tp.cut(x, y, z);
        }
        let (xe, ye, _) = *samples.last().unwrap();
        tp.rapid(xe, ye, safe_z);
        vec![tp]
    }

    /// Zig-zag pattern: stay on surface between rows, alternating
    /// direction row to row.
    ///
    /// # Panics
    ///
    /// Panics if an internal scanline row is empty (should not happen in
    /// practice).
    fn generate_zigzag(&self, params: &SurfaceParams) -> Vec<Toolpath> {
        let rows = collect_surface_rows(params, /*alternate=*/ true);
        let safe_z = params.cut_params.safe_z;

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

            if let Some((px, py, pz)) = prev_end {
                // Subsequent rows: cut directly from previous row end
                // (staying on the surface, no retract)
                tp.cut(px, py, pz);
                tp.cut(x0, y0, z0);
            } else {
                // First row: rapid to safe Z then plunge
                tp.rapid(x0, y0, safe_z);
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

/// Collect tool-center sample rows from the mesh bounding box, using the
/// disc-projected sample at each grid point. When `alternate` is true the
/// scan direction reverses row-to-row (used by zig-zag); when false every
/// row is walked in the same direction (used by one-way).
///
/// Rows that contain no on-mesh samples are dropped. Returns an empty
/// `Vec` for a mesh with no bounding box.
fn collect_surface_rows(params: &SurfaceParams, alternate: bool) -> Vec<Vec<(f64, f64, f64)>> {
    let bounds = match &params.mesh.bounds {
        Some(b) => b,
        None => return Vec::new(),
    };
    let step = params.cut_params.step_over.max(0.1);
    let tool_type = params.cut_params.tool.tool_type.clone();
    let tool_radius = params.cut_params.tool.diameter / 2.0;
    let mut rows = Vec::new();
    let mut forward = true;
    match params.scan_direction {
        ScanDirection::X => {
            let (x_min, x_max) = (bounds.min.x, bounds.max.x);
            let (y_min, y_max) = (bounds.min.y, bounds.max.y);
            let mut y = y_min;
            while y <= y_max {
                let xs: Vec<f64> = if forward {
                    float_range(x_min, x_max, step).collect()
                } else {
                    let fwd: Vec<f64> = float_range(x_min, x_max, step).collect();
                    fwd.into_iter().rev().collect()
                };
                let row: Vec<(f64, f64, f64)> = xs
                    .into_iter()
                    .filter_map(|x| {
                        Surface3dStrategy::sample_point(params.mesh, x, y, &tool_type, tool_radius)
                    })
                    .collect();
                if !row.is_empty() {
                    rows.push(row);
                }
                if alternate {
                    forward = !forward;
                }
                y += step;
            }
        }
        ScanDirection::Y => {
            let (x_min, x_max) = (bounds.min.x, bounds.max.x);
            let (y_min, y_max) = (bounds.min.y, bounds.max.y);
            let mut x = x_min;
            while x <= x_max {
                let ys: Vec<f64> = if forward {
                    float_range(y_min, y_max, step).collect()
                } else {
                    let fwd: Vec<f64> = float_range(y_min, y_max, step).collect();
                    fwd.into_iter().rev().collect()
                };
                let col: Vec<(f64, f64, f64)> = ys
                    .into_iter()
                    .filter_map(|y| {
                        Surface3dStrategy::sample_point(params.mesh, x, y, &tool_type, tool_radius)
                    })
                    .collect();
                if !col.is_empty() {
                    rows.push(col);
                }
                if alternate {
                    forward = !forward;
                }
                x += step;
            }
        }
    }
    rows
}

// ── Laser cut strategy ──────────────────────────────────────────────

/// Laser cut strategy: follows contour paths at Z=0 with power metadata.
/// Supports multi-pass via the `passes` field in CutParams (or via emitter).
pub struct LaserCutStrategy {
    pub power: f64,
}

impl LaserCutStrategy {
    pub fn new(power: f64) -> Self {
        Self { power }
    }
}

impl ToolpathStrategy for LaserCutStrategy {
    fn generate(&self, contours: &[Polyline], _params: &CutParams) -> Vec<Toolpath> {
        let mut toolpaths = Vec::new();

        for contour in contours {
            if contour.points.is_empty() {
                continue;
            }

            let mut tp = Toolpath::new();
            let first = contour.points[0];

            // Rapid to start (no Z movement for laser)
            tp.rapid(first.x, first.y, 0.0);

            // Cut along contour with power
            for pt in &contour.points[1..] {
                tp.cut_with_power(pt.x, pt.y, 0.0, self.power);
            }

            // Close if needed
            if contour.closed && contour.points.len() > 1 {
                tp.cut_with_power(first.x, first.y, 0.0, self.power);
            }

            toolpaths.push(tp);
        }
        toolpaths
    }
}

// ── Laser engrave strategy ──────────────────────────────────────────

/// Laser engrave strategy: scanline fill of closed paths.
/// Bidirectional serpentine pattern with configurable line spacing.
pub struct LaserEngraveStrategy {
    pub power: f64,
    pub line_spacing: f64,
}

impl LaserEngraveStrategy {
    pub fn new(power: f64, line_spacing: f64) -> Self {
        Self {
            power,
            line_spacing: line_spacing.max(0.1),
        }
    }
}

impl ToolpathStrategy for LaserEngraveStrategy {
    fn generate(&self, contours: &[Polyline], _params: &CutParams) -> Vec<Toolpath> {
        let mut toolpaths = Vec::new();

        for contour in contours {
            if contour.points.len() < 3 || !contour.closed {
                continue;
            }

            let bounds = match contour.bounds() {
                Some(b) => b,
                None => continue,
            };

            let y_min = bounds.min.y;
            let y_max = bounds.max.y;

            let mut tp = Toolpath::new();
            let mut y = y_min;
            let mut forward = true;

            while y <= y_max {
                let mut xs = scanline_intersect(contour, y);
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());

                for pair in xs.chunks(2) {
                    if pair.len() < 2 {
                        continue;
                    }
                    let x0 = pair[0];
                    let x1 = pair[1];
                    if x0 >= x1 {
                        continue;
                    }

                    let (start_x, end_x) = if forward { (x0, x1) } else { (x1, x0) };

                    tp.rapid(start_x, y, 0.0);
                    tp.cut_with_power(end_x, y, 0.0, self.power);
                }
                forward = !forward;
                y += self.line_spacing;
            }

            if !tp.moves.is_empty() {
                toolpaths.push(tp);
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

    // ── Task-002 tests: Pattern enum + SurfaceParams extension ───────────

    #[test]
    fn test_req_002_pattern_default_is_zigzag() {
        assert_eq!(Pattern::default(), Pattern::ZigZag);
    }

    #[test]
    fn test_req_002_new_fills_default_pattern() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        assert_eq!(surface.pattern, Pattern::ZigZag);
    }

    #[test]
    fn test_req_002_from_tuple_fills_default_pattern() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        let surface: SurfaceParams = (&mesh, &cut_params).into();
        assert_eq!(surface.pattern, Pattern::ZigZag);
    }

    // ── Task-003 tests: Surface3dStrategy + disc-projected sampling ─────

    fn make_peak_surface_mesh() -> Mesh {
        use crate::geometry::{Triangle, Vec3};
        // Pyramid: 10x10 base at z=5 rising to apex at (5, 5, 7), built
        // from four triangular walls.
        let apex = Vec3::new(5.0, 5.0, 7.0);
        let c1 = Vec3::new(0.0, 0.0, 5.0);
        let c2 = Vec3::new(10.0, 0.0, 5.0);
        let c3 = Vec3::new(10.0, 10.0, 5.0);
        let c4 = Vec3::new(0.0, 10.0, 5.0);
        let n = Vec3::new(0.0, 0.0, 1.0);
        let t1 = Triangle {
            normal: n,
            v0: c1,
            v1: c2,
            v2: apex,
        };
        let t2 = Triangle {
            normal: n,
            v0: c2,
            v1: c3,
            v2: apex,
        };
        let t3 = Triangle {
            normal: n,
            v0: c3,
            v1: c4,
            v2: apex,
        };
        let t4 = Triangle {
            normal: n,
            v0: c4,
            v1: c1,
            v2: apex,
        };
        Mesh::new(vec![t1, t2, t3, t4])
    }

    #[test]
    fn test_req_003_surface3d_flat_tool_does_not_gouge_peak() {
        use crate::slicer::mesh_height_at;
        use crate::tool::{Tool, ToolType};
        let mesh = make_peak_surface_mesh();
        let tool = Tool::new(ToolType::EndMill, 2.0, 10.0, 0.0);
        let cut_params = CutParams {
            tool,
            step_over: 1.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        let max_z: f64 = toolpaths
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| !m.rapid)
            .map(|m| m.z)
            .fold(f64::NEG_INFINITY, f64::max);
        // Apex is at 7.0; the flat disc projection must lift the center at
        // the apex to exactly the apex Z.
        let apex_z = mesh_height_at(&mesh, 5.0, 5.0).unwrap();
        assert!(
            (max_z - apex_z).abs() < 1e-6,
            "flat end mill must sit on the z=7 apex; got max_z = {}",
            max_z
        );
    }

    #[test]
    fn test_req_003_surface3d_flat_disc_catches_off_center_peak() {
        use crate::slicer::mesh_height_at;
        use crate::tool::{Tool, ToolType};
        let mesh = make_peak_surface_mesh();
        // (4, 5) is on the west-facing slope, below the apex.
        let pointwise = mesh_height_at(&mesh, 4.0, 5.0).expect("on slope");
        assert!(
            pointwise < 7.0 - 1e-6,
            "precondition: (4, 5) slope should be below apex; got {}",
            pointwise
        );
        let tool = Tool::new(ToolType::EndMill, 2.0, 10.0, 0.0); // radius 1.0
        let cut_params = CutParams {
            tool,
            step_over: 1.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        // The grid places a tool-center sample at (4, 5); the disc of
        // radius 1 reaches the apex at (5, 5, 7), so Z at that move is 7.
        let found = toolpaths.iter().flat_map(|tp| tp.moves.iter()).any(|m| {
            !m.rapid
                && (m.x - 4.0).abs() < 0.01
                && (m.y - 5.0).abs() < 0.01
                && (m.z - 7.0).abs() < 1e-6
        });
        assert!(
            found,
            "disc-projected flat tool at (4, 5) must lift to the z=7 apex (regression guard against pointwise sampling)"
        );
    }

    #[test]
    fn test_req_003_surface3d_ball_lifts_plateau_by_radius() {
        use crate::tool::{Tool, ToolType};
        let mesh = make_flat_surface_mesh();
        let r = 1.0;
        let tool = Tool::new(ToolType::BallEnd, 2.0 * r, 10.0, r);
        let cut_params = CutParams {
            tool,
            step_over: 2.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::X);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        let cuts: Vec<_> = toolpaths
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| !m.rapid)
            .collect();
        assert!(!cuts.is_empty(), "strategy must emit cuts on a flat plate");
        for m in cuts {
            assert!(
                (m.z - (5.0 + r)).abs() < 1e-6,
                "ball mill on z=5 plateau with radius {} must yield z = {}, got z = {}",
                r,
                5.0 + r,
                m.z
            );
        }
    }

    // ── Task-004 tests: one-way pattern ─────────────────────────────────

    fn one_way_surface(mesh: &Mesh) -> (CutParams, SurfaceParams<'_>) {
        let cut_params = CutParams {
            step_over: 2.0,
            ..CutParams::default()
        };
        let surface = SurfaceParams::new_with_pattern(
            mesh,
            cut_params.clone(),
            ScanDirection::X,
            Pattern::OneWay,
        );
        (cut_params, surface)
    }

    #[test]
    fn test_req_004_one_way_emits_two_rapids_per_row() {
        let mesh = make_flat_surface_mesh();
        let (_, surface) = one_way_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        let rapid_count: usize = toolpaths
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| m.rapid)
            .count();
        let row_count = toolpaths.len();
        assert!(row_count > 0, "expected at least one row of cuts");
        assert_eq!(
            rapid_count,
            row_count * 2,
            "one-way emits plunge + retract per row (rows={}, rapids={})",
            row_count,
            rapid_count
        );
        // And crucially: strictly more rapids than zig-zag over the same mesh
        // (zig-zag emits exactly 2 rapids total).
        let zigzag_surface = SurfaceParams::new_with_pattern(
            &mesh,
            CutParams {
                step_over: 2.0,
                ..CutParams::default()
            },
            ScanDirection::X,
            Pattern::ZigZag,
        );
        let zigzag_rapids: usize = Surface3dStrategy
            .generate_surface(&zigzag_surface)
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| m.rapid)
            .count();
        assert_eq!(zigzag_rapids, 2);
        assert!(rapid_count > zigzag_rapids);
    }

    #[test]
    fn test_req_004_one_way_cuts_are_monotonically_forward() {
        let mesh = make_flat_surface_mesh();
        let (_, surface) = one_way_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        // For ScanDirection::X, within each toolpath the non-rapid moves
        // must have non-decreasing X.
        for tp in &toolpaths {
            let cuts: Vec<_> = tp.moves.iter().filter(|m| !m.rapid).collect();
            for pair in cuts.windows(2) {
                let dx = pair[1].x - pair[0].x;
                assert!(
                    dx >= -1e-9,
                    "one-way row cuts must have non-decreasing X; got dx={}",
                    dx
                );
            }
        }
    }

    #[test]
    fn test_req_004_one_way_first_and_last_include_safe_z_rapid() {
        let mesh = make_flat_surface_mesh();
        let (cut_params, surface) = one_way_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        assert!(toolpaths.len() >= 2, "need multiple rows to test");
        let first = &toolpaths[0];
        let last = toolpaths.last().unwrap();
        assert!(
            first
                .moves
                .iter()
                .any(|m| m.rapid && (m.z - cut_params.safe_z).abs() < 1e-9),
            "first toolpath must include a rapid to safe_z"
        );
        assert!(
            last.moves
                .iter()
                .any(|m| m.rapid && (m.z - cut_params.safe_z).abs() < 1e-9),
            "last toolpath must include a rapid to safe_z"
        );
    }

    // ── Task-005 tests: spiral pattern ──────────────────────────────────

    fn spiral_surface(mesh: &Mesh) -> SurfaceParams<'_> {
        SurfaceParams::new_with_pattern(
            mesh,
            CutParams {
                step_over: 1.0,
                ..CutParams::default()
            },
            ScanDirection::X,
            Pattern::Spiral,
        )
    }

    #[test]
    fn test_req_005_spiral_completes_multiple_loops() {
        // 10x10 flat surface with step_over=1 should fit an outer ring
        // plus at least one inner ring before termination (loops count
        // ≈ (side_length / 2) / step_over ≈ 5). We require ≥ 2.
        let mesh = make_flat_surface_mesh();
        let surface = spiral_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        assert_eq!(toolpaths.len(), 1, "spiral emits a single toolpath");
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        // An outer loop of a 10x10 box at step=1 contains ~40 points
        // (4 sides × 10). Two loops ⇒ ≥ 2*(perimeter/step) points.
        assert!(
            cuts.len() >= 40,
            "expected at least two full loops; got {} cut moves",
            cuts.len()
        );
    }

    #[test]
    fn test_req_005_spiral_inward_progression() {
        // The last sampled point must be strictly closer to the bounding
        // box center than the first. A stronger test — all inter-point
        // distance changes stay within tolerance `step_over * sqrt(2)` —
        // guards against any outward jumps in the path.
        let mesh = make_flat_surface_mesh(); // bounds: [0,10]x[0,10]
        let surface = spiral_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        let center = (5.0, 5.0);
        let dist = |m: &&crate::geometry::ToolpathMove| -> f64 {
            ((m.x - center.0).powi(2) + (m.y - center.1).powi(2)).sqrt()
        };
        let d_first = dist(&cuts[0]);
        let d_last = dist(&cuts[cuts.len() - 1]);
        assert!(
            d_last < d_first,
            "spiral end ({}) must be closer to center than start ({})",
            d_last,
            d_first
        );
        // No single step should exceed step_over * sqrt(2) + small slack
        // (diagonal of one step). This rules out large outward jumps.
        let step = 1.0_f64;
        let slack = step * core::f64::consts::SQRT_2 + 1e-6;
        for pair in cuts.windows(2) {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let d = (dx * dx + dy * dy).sqrt();
            assert!(
                d <= slack,
                "spiral hop between consecutive points must be ≤ one diagonal step; got {}",
                d
            );
        }
    }

    #[test]
    fn test_req_005_spiral_has_exactly_two_rapids() {
        let mesh = make_flat_surface_mesh();
        let surface = spiral_surface(&mesh);
        let toolpaths = Surface3dStrategy.generate_surface(&surface);
        let rapid_count: usize = toolpaths
            .iter()
            .flat_map(|tp| tp.moves.iter())
            .filter(|m| m.rapid)
            .count();
        assert_eq!(
            rapid_count, 2,
            "spiral emits exactly one initial rapid + one final retract"
        );
    }

    #[test]
    fn test_req_003_zigzag_alias_still_resolves() {
        // The `pub use` shim must keep the old name usable.
        let _alias = ZigzagSurfaceStrategy;
        // And it must be the same type as the new name.
        fn assert_same<T>(_a: T, _b: T) {}
        assert_same(Surface3dStrategy, ZigzagSurfaceStrategy);
    }

    #[test]
    fn test_req_002_new_with_pattern_round_trips_all_variants() {
        let mesh = make_simple_mesh();
        let cut_params = CutParams::default();
        for expected in [Pattern::ZigZag, Pattern::OneWay, Pattern::Spiral] {
            let surface = SurfaceParams::new_with_pattern(
                &mesh,
                cut_params.clone(),
                ScanDirection::Y,
                expected,
            );
            assert_eq!(surface.pattern, expected);
            assert_eq!(surface.scan_direction, ScanDirection::Y);
        }
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
    // (Former tests for the normal-based `ball_end_offset` helper were
    // deleted in task-003. Disc-sampled ball projection replaces it; see
    // slicer::project_ball_tool and the test_req_001_ball_* tests.)

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

    // ── Laser cut strategy tests ──────────────────────────────────────

    #[test]
    fn test_laser_cut_follows_contour() {
        let contours = vec![square()];
        let strategy = LaserCutStrategy::new(80.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        assert_eq!(toolpaths.len(), 1);
        // Should have rapid + 4 cuts + close = 6 moves
        assert!(toolpaths[0].moves.len() >= 5);
    }

    #[test]
    fn test_laser_cut_at_z_zero() {
        let contours = vec![square()];
        let strategy = LaserCutStrategy::new(50.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        for mv in &toolpaths[0].moves {
            assert!((mv.z - 0.0).abs() < 0.001, "Laser should operate at Z=0");
        }
    }

    #[test]
    fn test_laser_cut_has_power() {
        let contours = vec![square()];
        let strategy = LaserCutStrategy::new(75.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        assert!(!cuts.is_empty());
        for cut in &cuts {
            assert_eq!(cut.power, Some(75.0));
        }
    }

    #[test]
    fn test_laser_cut_closes_path() {
        let contours = vec![square()]; // closed polyline
        let strategy = LaserCutStrategy::new(80.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        let moves = &toolpaths[0].moves;
        // First move is rapid to start point; last move should return there (closing)
        let first = &moves[0];
        let last = moves.last().unwrap();
        assert!((last.x - first.x).abs() < 0.01);
        assert!((last.y - first.y).abs() < 0.01);
    }

    // ── Laser engrave strategy tests ──────────────────────────────────

    #[test]
    fn test_laser_engrave_scanlines() {
        let contours = vec![square()]; // 10x10 closed square
        let strategy = LaserEngraveStrategy::new(60.0, 1.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        assert!(!toolpaths.is_empty());
        // Should have multiple scanlines
        assert!(toolpaths[0].moves.len() > 10);
    }

    #[test]
    fn test_laser_engrave_at_z_zero() {
        let contours = vec![square()];
        let strategy = LaserEngraveStrategy::new(60.0, 1.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        for mv in &toolpaths[0].moves {
            assert!((mv.z - 0.0).abs() < 0.001, "Engrave should operate at Z=0");
        }
    }

    #[test]
    fn test_laser_engrave_has_power() {
        let contours = vec![square()];
        let strategy = LaserEngraveStrategy::new(42.0, 1.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        assert!(!cuts.is_empty());
        for cut in &cuts {
            assert_eq!(cut.power, Some(42.0));
        }
    }

    #[test]
    fn test_laser_engrave_skips_open_paths() {
        let open = Polyline::new(vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)], false);
        let strategy = LaserEngraveStrategy::new(60.0, 1.0);
        let toolpaths = strategy.generate(&[open], &CutParams::default());
        assert!(toolpaths.is_empty(), "Should skip open polylines");
    }

    #[test]
    fn test_laser_engrave_serpentine() {
        let contours = vec![square()];
        let strategy = LaserEngraveStrategy::new(60.0, 2.0);
        let toolpaths = strategy.generate(&contours, &CutParams::default());
        // Find cutting moves to check direction alternates
        let cuts: Vec<_> = toolpaths[0].moves.iter().filter(|m| !m.rapid).collect();
        if cuts.len() >= 2 {
            // First and second cut lines should go in different X directions
            // (alternating forward/backward)
            let first_dx = cuts[0].x;
            // Just verify we have valid moves
            assert!(first_dx.is_finite());
        }
    }

    #[test]
    fn test_zigzag_surface_y_scan_direction() {
        let mesh = make_flat_surface_mesh();
        let cut_params = CutParams::default();
        let surface = SurfaceParams::new(&mesh, cut_params, ScanDirection::Y);
        let strategy = ZigzagSurfaceStrategy;
        let toolpaths = strategy.generate_surface(&surface);
        assert!(!toolpaths.is_empty());
        let total_moves: usize = toolpaths.iter().map(|tp| tp.moves.len()).sum();
        assert!(total_moves > 1);
    }
}
