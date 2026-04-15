//! Sketch data types and pure helper functions.
//!
//! This module is **not** gated on `wasm32` so that the helpers can be
//! unit-tested with `cargo test -p combined-frontend`.

use serde::{Deserialize, Serialize};

// ── Drawing tool ────────────────────────────────────────────────────

/// Which drawing tool the user has selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawingTool {
    Line,
    Rectangle,
    Circle,
    Polyline,
}

// ── Point ───────────────────────────────────────────────────────────

/// A 2D point in sketch world-space (mm).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// ── Sketch shapes ───────────────────────────────────────────────────

/// A committed shape in the sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SketchShape {
    Line {
        p1: Point,
        p2: Point,
    },
    Rectangle {
        origin: Point,
        width: f64,
        height: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Polyline {
        points: Vec<Point>,
    },
}

// ── Constraint kinds (for the UI selector) ─────────────────────────

/// Constraint types exposed in the editor UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    Coincident,
    Horizontal,
    Vertical,
    Distance,
    Fixed,
}

impl ConstraintKind {
    /// How many point picks are required for this constraint type.
    pub fn pick_count(&self) -> usize {
        match self {
            Self::Coincident | Self::Horizontal | Self::Vertical | Self::Distance => 2,
            Self::Fixed => 1,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Coincident => "Coincident",
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
            Self::Distance => "Distance",
            Self::Fixed => "Fixed",
        }
    }

    pub fn api_name(&self) -> &'static str {
        match self {
            Self::Coincident => "coincident",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Distance => "distance",
            Self::Fixed => "fixed",
        }
    }
}

// ── Pure helpers ────────────────────────────────────────────────────

/// Snap `(x, y)` to the nearest grid intersection.
///
/// If `grid_size` is zero or negative the coordinates are returned unchanged.
pub fn snap_to_grid(x: f64, y: f64, grid_size: f64) -> (f64, f64) {
    if grid_size <= 0.0 {
        return (x, y);
    }
    let sx = (x / grid_size).round() * grid_size;
    let sy = (y / grid_size).round() * grid_size;
    (sx, sy)
}

/// Convert a slice of shapes to a stand-alone SVG string.
pub fn shapes_to_svg(shapes: &[SketchShape]) -> String {
    let mut elements = String::new();
    for shape in shapes {
        match shape {
            SketchShape::Line { p1, p2 } => {
                elements.push_str(&format!(
                    "<path d=\"M {} {} L {} {}\"/>",
                    p1.x, p1.y, p2.x, p2.y,
                ));
            }
            SketchShape::Rectangle {
                origin,
                width,
                height,
            } => {
                elements.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    origin.x, origin.y, width, height,
                ));
            }
            SketchShape::Circle { center, radius } => {
                elements.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                    center.x, center.y, radius,
                ));
            }
            SketchShape::Polyline { points } => {
                if points.len() >= 2 {
                    let pts: Vec<String> =
                        points.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
                    elements.push_str(&format!("<polyline points=\"{}\"/>", pts.join(" "),));
                }
            }
        }
    }
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">{elements}</svg>")
}

/// Shape description for the shape-list panel.
pub fn shape_label(shape: &SketchShape) -> String {
    match shape {
        SketchShape::Line { p1, p2 } => {
            format!("Line ({},{}) -> ({},{})", p1.x, p1.y, p2.x, p2.y)
        }
        SketchShape::Rectangle {
            origin,
            width,
            height,
        } => {
            format!("Rect {}x{} at ({},{})", width, height, origin.x, origin.y,)
        }
        SketchShape::Circle { center, radius } => {
            format!("Circle r={} at ({},{})", radius, center.x, center.y)
        }
        SketchShape::Polyline { points } => {
            format!("Polyline {} pts", points.len())
        }
    }
}

/// Test whether the point `(px, py)` is within `tolerance` of any part of `shape`.
pub fn point_in_shape(px: f64, py: f64, shape: &SketchShape, tolerance: f64) -> bool {
    match shape {
        SketchShape::Line { p1, p2 } => {
            point_to_segment_distance(px, py, p1.x, p1.y, p2.x, p2.y) <= tolerance
        }
        SketchShape::Rectangle {
            origin,
            width,
            height,
        } => {
            // Test against the four edges of the rectangle.
            let x0 = origin.x;
            let y0 = origin.y;
            let x1 = origin.x + width;
            let y1 = origin.y + height;
            let edges = [
                (x0, y0, x1, y0), // top
                (x1, y0, x1, y1), // right
                (x1, y1, x0, y1), // bottom
                (x0, y1, x0, y0), // left
            ];
            edges.iter().any(|&(ax, ay, bx, by)| {
                point_to_segment_distance(px, py, ax, ay, bx, by) <= tolerance
            })
        }
        SketchShape::Circle { center, radius } => {
            let dist = ((px - center.x).powi(2) + (py - center.y).powi(2)).sqrt();
            (dist - radius).abs() <= tolerance
        }
        SketchShape::Polyline { points } => points.windows(2).any(|seg| {
            point_to_segment_distance(px, py, seg[0].x, seg[0].y, seg[1].x, seg[1].y) <= tolerance
        }),
    }
}

/// Distance from point `(px, py)` to the line segment `(ax, ay) -- (bx, by)`.
fn point_to_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        // Degenerate segment (a == b): distance to the point.
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- snap_to_grid ------------------------------------------------

    #[test]
    fn test_snap_to_grid_basic() {
        assert_eq!(snap_to_grid(7.3, 4.8, 5.0), (5.0, 5.0));
        assert_eq!(snap_to_grid(12.6, 17.4, 5.0), (15.0, 15.0));
    }

    #[test]
    fn test_snap_to_grid_exact() {
        assert_eq!(snap_to_grid(10.0, 20.0, 10.0), (10.0, 20.0));
    }

    #[test]
    fn test_snap_to_grid_zero_size() {
        assert_eq!(snap_to_grid(3.7, 8.2, 0.0), (3.7, 8.2));
    }

    #[test]
    fn test_snap_to_grid_negative_size() {
        assert_eq!(snap_to_grid(3.7, 8.2, -1.0), (3.7, 8.2));
    }

    #[test]
    fn test_snap_to_grid_small() {
        let (sx, sy) = snap_to_grid(0.13, 0.27, 0.1);
        assert!((sx - 0.1).abs() < 1e-9);
        assert!((sy - 0.3).abs() < 1e-9);
    }

    // -- shapes_to_svg -----------------------------------------------

    #[test]
    fn test_shapes_to_svg_line() {
        let shapes = vec![SketchShape::Line {
            p1: Point::new(0.0, 0.0),
            p2: Point::new(10.0, 20.0),
        }];
        let svg = shapes_to_svg(&shapes);
        assert!(svg.contains("<path d=\"M 0 0 L 10 20\"/>"));
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_shapes_to_svg_rect() {
        let shapes = vec![SketchShape::Rectangle {
            origin: Point::new(5.0, 10.0),
            width: 20.0,
            height: 15.0,
        }];
        let svg = shapes_to_svg(&shapes);
        assert!(svg.contains("<rect x=\"5\" y=\"10\" width=\"20\" height=\"15\"/>"));
    }

    #[test]
    fn test_shapes_to_svg_circle() {
        let shapes = vec![SketchShape::Circle {
            center: Point::new(50.0, 50.0),
            radius: 25.0,
        }];
        let svg = shapes_to_svg(&shapes);
        assert!(svg.contains("<circle cx=\"50\" cy=\"50\" r=\"25\"/>"));
    }

    #[test]
    fn test_shapes_to_svg_polyline() {
        let shapes = vec![SketchShape::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
            ],
        }];
        let svg = shapes_to_svg(&shapes);
        assert!(svg.contains("<polyline points=\"0,0 10,0 10,10\"/>"));
    }

    #[test]
    fn test_shapes_to_svg_empty() {
        let svg = shapes_to_svg(&[]);
        assert!(svg.contains("viewBox"));
        assert!(!svg.contains("<path"));
    }

    // -- point_in_shape (line) ----------------------------------------

    #[test]
    fn test_point_in_line_on_segment() {
        let line = SketchShape::Line {
            p1: Point::new(0.0, 0.0),
            p2: Point::new(10.0, 0.0),
        };
        assert!(point_in_shape(5.0, 0.0, &line, 1.0));
    }

    #[test]
    fn test_point_in_line_near() {
        let line = SketchShape::Line {
            p1: Point::new(0.0, 0.0),
            p2: Point::new(10.0, 0.0),
        };
        assert!(point_in_shape(5.0, 0.5, &line, 1.0));
    }

    #[test]
    fn test_point_in_line_far() {
        let line = SketchShape::Line {
            p1: Point::new(0.0, 0.0),
            p2: Point::new(10.0, 0.0),
        };
        assert!(!point_in_shape(5.0, 5.0, &line, 1.0));
    }

    // -- point_in_shape (rectangle) -----------------------------------

    #[test]
    fn test_point_in_rect_edge() {
        let rect = SketchShape::Rectangle {
            origin: Point::new(0.0, 0.0),
            width: 10.0,
            height: 10.0,
        };
        // On the top edge
        assert!(point_in_shape(5.0, 0.0, &rect, 1.0));
        // Inside but far from edges
        assert!(!point_in_shape(5.0, 5.0, &rect, 0.5));
    }

    // -- point_in_shape (circle) --------------------------------------

    #[test]
    fn test_point_in_circle_on_circumference() {
        let circle = SketchShape::Circle {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
        };
        assert!(point_in_shape(10.0, 0.0, &circle, 1.0));
        assert!(point_in_shape(0.0, 10.0, &circle, 1.0));
    }

    #[test]
    fn test_point_in_circle_center() {
        let circle = SketchShape::Circle {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
        };
        // Center is 10 units from circumference
        assert!(!point_in_shape(0.0, 0.0, &circle, 1.0));
    }

    // -- point_in_shape (polyline) ------------------------------------

    #[test]
    fn test_point_in_polyline() {
        let poly = SketchShape::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
            ],
        };
        assert!(point_in_shape(5.0, 0.0, &poly, 1.0));
        assert!(point_in_shape(10.0, 5.0, &poly, 1.0));
        assert!(!point_in_shape(5.0, 5.0, &poly, 1.0));
    }

    // -- shape_label --------------------------------------------------

    #[test]
    fn test_shape_label() {
        let line = SketchShape::Line {
            p1: Point::new(0.0, 0.0),
            p2: Point::new(10.0, 20.0),
        };
        assert!(shape_label(&line).contains("Line"));
    }
}
