//! SimMove type and testable geometry helpers for CAM preview/simulation.
//!
//! This module is NOT gated behind `target_arch = "wasm32"` so that tests
//! run under the native host target.

use serde::{Deserialize, Serialize};

/// A single tool-position sample in a CAM simulation.
///
/// Mirrors the JSON objects produced by `sim_moves_stl_impl` /
/// `sim_moves_svg_impl` in the `rustcam` crate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimMove {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rapid: bool,
}

/// Axis-aligned bounding box: `(min_x, min_y, max_x, max_y)`.
///
/// For an empty slice the box degenerates to `(0, 0, 0, 0)`.
pub fn compute_bounds(moves: &[SimMove]) -> (f64, f64, f64, f64) {
    if moves.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for m in moves {
        if m.x < min_x {
            min_x = m.x;
        }
        if m.y < min_y {
            min_y = m.y;
        }
        if m.x > max_x {
            max_x = m.x;
        }
        if m.y > max_y {
            max_y = m.y;
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Map a world coordinate `(wx, wy)` to canvas pixel space.
///
/// The mapping fits the bounding box into the canvas with uniform scaling
/// and `padding` pixels of margin on each side.  The Y axis is flipped so
/// that positive Y points upward in world space but downward on the canvas.
///
/// When the bounding box has zero width or height the coordinate is placed
/// at the centre of the available axis.
pub fn world_to_canvas(
    wx: f64,
    wy: f64,
    bounds: (f64, f64, f64, f64),
    canvas_width: f64,
    canvas_height: f64,
    padding: f64,
) -> (f64, f64) {
    let (min_x, min_y, max_x, max_y) = bounds;
    let bw = max_x - min_x;
    let bh = max_y - min_y;
    let avail_w = (canvas_width - 2.0 * padding).max(1.0);
    let avail_h = (canvas_height - 2.0 * padding).max(1.0);

    if bw <= 0.0 && bh <= 0.0 {
        // Degenerate: single point or empty — place at centre.
        return (canvas_width / 2.0, canvas_height / 2.0);
    }

    let scale = if bw <= 0.0 {
        avail_h / bh
    } else if bh <= 0.0 {
        avail_w / bw
    } else {
        (avail_w / bw).min(avail_h / bh)
    };

    // Centre the fitted rectangle inside the canvas.
    let off_x = padding + (avail_w - bw * scale) / 2.0;
    let off_y = padding + (avail_h - bh * scale) / 2.0;

    let cx = off_x + (wx - min_x) * scale;
    // Flip Y: world-up → canvas-down.
    let cy = off_y + (max_y - wy) * scale;

    (cx, cy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_empty() {
        let b = compute_bounds(&[]);
        assert_eq!(b, (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_bounds_single() {
        let moves = vec![SimMove {
            x: 5.0,
            y: 7.0,
            z: 0.0,
            rapid: false,
        }];
        let b = compute_bounds(&moves);
        assert_eq!(b, (5.0, 7.0, 5.0, 7.0));
    }

    #[test]
    fn test_bounds_multiple() {
        let moves = vec![
            SimMove {
                x: -1.0,
                y: 2.0,
                z: 0.0,
                rapid: true,
            },
            SimMove {
                x: 3.0,
                y: -4.0,
                z: -1.0,
                rapid: false,
            },
            SimMove {
                x: 10.0,
                y: 6.0,
                z: 0.0,
                rapid: false,
            },
        ];
        let b = compute_bounds(&moves);
        assert_eq!(b, (-1.0, -4.0, 10.0, 6.0));
    }

    #[test]
    fn test_world_to_canvas_basic() {
        // Bounding box 0..100 x 0..100, canvas 200x200, padding 10%.
        // Available = 200 - 2*20 = 160. Scale = 160/100 = 1.6.
        // Off_x = 20 + (160 - 100*1.6)/2 = 20. Off_y = 20.
        let bounds = (0.0, 0.0, 100.0, 100.0);
        let padding = 20.0; // 10% of 200

        // Bottom-left corner in world → top of canvas (Y flipped)
        let (cx, cy) = world_to_canvas(0.0, 0.0, bounds, 200.0, 200.0, padding);
        assert!((cx - 20.0).abs() < 1e-9, "cx={cx}");
        assert!((cy - 180.0).abs() < 1e-9, "cy={cy}");

        // Top-right corner in world
        let (cx2, cy2) = world_to_canvas(100.0, 100.0, bounds, 200.0, 200.0, padding);
        assert!((cx2 - 180.0).abs() < 1e-9, "cx2={cx2}");
        assert!((cy2 - 20.0).abs() < 1e-9, "cy2={cy2}");

        // Centre
        let (cx3, cy3) = world_to_canvas(50.0, 50.0, bounds, 200.0, 200.0, padding);
        assert!((cx3 - 100.0).abs() < 1e-9, "cx3={cx3}");
        assert!((cy3 - 100.0).abs() < 1e-9, "cy3={cy3}");
    }

    #[test]
    fn test_world_to_canvas_degenerate_single_point() {
        // Single point → placed at canvas centre.
        let bounds = (5.0, 5.0, 5.0, 5.0);
        let (cx, cy) = world_to_canvas(5.0, 5.0, bounds, 300.0, 200.0, 10.0);
        assert!((cx - 150.0).abs() < 1e-9, "cx={cx}");
        assert!((cy - 100.0).abs() < 1e-9, "cy={cy}");
    }

    #[test]
    fn test_world_to_canvas_non_square() {
        // Bounding box wider than tall; scale determined by width.
        // bounds: 0..200 x 0..100 on a 400x400 canvas, padding=0.
        let bounds = (0.0, 0.0, 200.0, 100.0);
        // avail = 400, scale = min(400/200, 400/100) = 2.0
        let (cx, cy) = world_to_canvas(200.0, 100.0, bounds, 400.0, 400.0, 0.0);
        // off_x = 0 + (400 - 200*2)/2 = 0. off_y = 0 + (400 - 100*2)/2 = 100.
        assert!((cx - 400.0).abs() < 1e-9, "cx={cx}");
        assert!((cy - 100.0).abs() < 1e-9, "cy={cy}");
    }

    #[test]
    fn test_world_to_canvas_zero_height() {
        // All points on a horizontal line (bh == 0).
        let bounds = (0.0, 5.0, 10.0, 5.0);
        let (cx, cy) = world_to_canvas(5.0, 5.0, bounds, 200.0, 200.0, 0.0);
        // scale = avail_w / bw = 200 / 10 = 20.
        // off_x = 0 + (200 - 10*20)/2 = 0
        // off_y = 0 + (200 - 0*20)/2 = 100
        assert!((cx - 100.0).abs() < 1e-9, "cx={cx}");
        assert!((cy - 100.0).abs() < 1e-9, "cy={cy}");
    }
}
