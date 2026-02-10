/// Toolpath generation strategies.
///
/// Swiss-cheese layer: **Strategy selection**
/// Extension point: implement `ToolpathStrategy` to add spiral, trochoidal,
/// adaptive-clearing, or any custom strategy.
use crate::geometry::{Polyline, Toolpath, Vec2};

// ── Strategy trait (the "hole") ──────────────────────────────────────

pub trait ToolpathStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath>;
}

/// Cutting parameters shared by all strategies.
#[derive(Debug, Clone)]
pub struct CutParams {
    pub tool_diameter: f64,
    pub step_over: f64,
    pub step_down: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub cut_z: f64,
}

impl Default for CutParams {
    fn default() -> Self {
        Self {
            tool_diameter: 3.175,
            step_over: 1.5,
            step_down: 1.0,
            feed_rate: 800.0,
            plunge_rate: 300.0,
            safe_z: 5.0,
            cut_z: 0.0,
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
}
