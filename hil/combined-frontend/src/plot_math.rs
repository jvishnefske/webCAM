//! Pure-logic helpers for the series plot component.
//!
//! Extracted into a standalone module so they compile and are testable on
//! native (non-wasm) targets.  The canvas-drawing component in
//! `components::dag::plot` imports these at runtime.

use std::collections::VecDeque;

/// Maximum number of samples retained per series.
pub const MAX_HISTORY: usize = 200;

/// Colours cycled through for successive series.
pub const SERIES_COLORS: &[&str] = &["#22d3ee", "#f59e0b", "#4ade80", "#a78bfa", "#ef4444"];

/// Padding (in CSS pixels) around the plot area.
pub const PAD: f64 = 40.0;

/// Compute Y-axis bounds from data, returning `(min, max)`.
///
/// * If all deques are empty, returns `(0.0, 1.0)`.
/// * If min == max (flat line), adds symmetric padding of 1.0.
/// * Otherwise adds 5 % padding on each side.
pub fn compute_y_bounds(data: &[&VecDeque<f64>]) -> (f64, f64) {
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;

    for dq in data {
        for &v in dq.iter() {
            if v < global_min {
                global_min = v;
            }
            if v > global_max {
                global_max = v;
            }
        }
    }

    if global_min > global_max {
        // No data at all.
        return (0.0, 1.0);
    }

    if (global_max - global_min).abs() < f64::EPSILON {
        return (global_min - 1.0, global_max + 1.0);
    }

    let margin = (global_max - global_min) * 0.05;
    (global_min - margin, global_max + margin)
}

/// Generate `count` evenly-spaced grid-line Y values between `min` and `max`
/// (inclusive of both endpoints).
///
/// Returns an empty vec when `count < 2`.
pub fn grid_lines(min: f64, max: f64, count: usize) -> Vec<f64> {
    if count < 2 {
        return Vec::new();
    }
    let step = (max - min) / (count - 1) as f64;
    (0..count).map(|i| min + step * i as f64).collect()
}

/// Map a data value to a canvas Y coordinate.
///
/// `canvas_height` is the usable plot height (excluding padding).
/// Y = 0 is top of canvas, so higher values map to lower Y.
pub fn value_to_canvas_y(value: f64, min: f64, max: f64, canvas_height: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return canvas_height / 2.0;
    }
    canvas_height - ((value - min) / (max - min)) * canvas_height
}

/// Format a numeric axis label to a reasonable precision.
///
/// Uses two decimal places, which matches the TypeScript plot.
pub fn format_axis_label(value: f64) -> String {
    format!("{value:.2}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn test_y_bounds_empty() {
        let data: Vec<&VecDeque<f64>> = vec![];
        let (min, max) = compute_y_bounds(&data);
        assert!((min - 0.0).abs() < f64::EPSILON);
        assert!((max - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_y_bounds_empty_deques() {
        let dq = VecDeque::new();
        let (min, max) = compute_y_bounds(&[&dq]);
        assert!((min - 0.0).abs() < f64::EPSILON);
        assert!((max - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_y_bounds_single_value() {
        let dq: VecDeque<f64> = [5.0].into_iter().collect();
        let (min, max) = compute_y_bounds(&[&dq]);
        // Flat line: symmetric padding of 1.0
        assert!((min - 4.0).abs() < f64::EPSILON);
        assert!((max - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_y_bounds_range() {
        let dq: VecDeque<f64> = [0.0, 10.0].into_iter().collect();
        let (min, max) = compute_y_bounds(&[&dq]);
        // 5% margin on range of 10 => 0.5
        assert!((min - (-0.5)).abs() < f64::EPSILON);
        assert!((max - 10.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_y_bounds_multiple_series() {
        let dq1: VecDeque<f64> = [1.0, 3.0].into_iter().collect();
        let dq2: VecDeque<f64> = [-2.0, 5.0].into_iter().collect();
        let (min, max) = compute_y_bounds(&[&dq1, &dq2]);
        // Global range: -2..5, range=7, margin=0.35
        assert!((min - (-2.35)).abs() < 1e-10);
        assert!((max - 5.35).abs() < 1e-10);
    }

    #[test]
    fn test_value_to_canvas_y_at_min() {
        // Value at min maps to canvas_height (bottom).
        let y = value_to_canvas_y(0.0, 0.0, 10.0, 100.0);
        assert!((y - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_value_to_canvas_y_at_max() {
        // Value at max maps to 0 (top).
        let y = value_to_canvas_y(10.0, 0.0, 10.0, 100.0);
        assert!(y.abs() < f64::EPSILON);
    }

    #[test]
    fn test_value_to_canvas_y_midpoint() {
        // Midpoint maps to half.
        let y = value_to_canvas_y(5.0, 0.0, 10.0, 100.0);
        assert!((y - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_value_to_canvas_y_flat() {
        // When min == max, should return midpoint of canvas.
        let y = value_to_canvas_y(5.0, 5.0, 5.0, 200.0);
        assert!((y - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_grid_lines_five() {
        let lines = grid_lines(0.0, 10.0, 5);
        assert_eq!(lines.len(), 5);
        assert!((lines[0] - 0.0).abs() < f64::EPSILON);
        assert!((lines[1] - 2.5).abs() < f64::EPSILON);
        assert!((lines[2] - 5.0).abs() < f64::EPSILON);
        assert!((lines[3] - 7.5).abs() < f64::EPSILON);
        assert!((lines[4] - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_grid_lines_count_zero() {
        let lines = grid_lines(0.0, 10.0, 0);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_grid_lines_count_one() {
        let lines = grid_lines(0.0, 10.0, 1);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_format_axis_label_pi() {
        assert_eq!(format_axis_label(std::f64::consts::PI), "3.14");
    }

    #[test]
    fn test_format_axis_label_zero() {
        assert_eq!(format_axis_label(0.0), "0.00");
    }

    #[test]
    fn test_format_axis_label_negative() {
        assert_eq!(format_axis_label(-42.5), "-42.50");
    }

    #[test]
    fn test_format_axis_label_large() {
        assert_eq!(format_axis_label(1000.0), "1000.00");
    }
}
