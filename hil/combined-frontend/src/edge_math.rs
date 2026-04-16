//! Pure geometry helpers for DAG editor edge paths.
//!
//! These functions are not wasm32-gated so they can be tested on host.

/// Compute the SVG path data for a cubic Bezier curve between two points.
///
/// Uses the TypeScript editor's algorithm:
/// `cpX = max(dx * 0.5, min(|dy|, 50))`
pub fn edge_path_d(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let cp_x = f64::max(dx * 0.5, f64::min(dy, 50.0));
    format!(
        "M {},{} C {},{} {},{} {},{}",
        x1,
        y1,
        x1 + cp_x,
        y1,
        x2 - cp_x,
        y2,
        x2,
        y2,
    )
}

/// Port Y position within a node.
///
/// Returns the vertical center of the given port circle, accounting for
/// the node header (30px), port spacing (20px per port), and a 6px offset
/// to center on a 12px port circle.
pub fn port_y(node_y: f64, port_index: usize) -> f64 {
    node_y + 30.0 + port_index as f64 * 20.0 + 6.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_path_d_straight_horizontal() {
        let d = edge_path_d(0.0, 100.0, 200.0, 100.0);
        assert!(d.starts_with("M 0,100"));
        assert!(d.contains("200,100"));
    }

    #[test]
    fn test_edge_path_d_diagonal() {
        let d = edge_path_d(0.0, 0.0, 200.0, 100.0);
        // cpX = max(100, min(100, 50)) = max(100, 50) = 100
        assert!(d.contains("C 100,0")); // control point 1
    }

    #[test]
    fn test_edge_path_d_short_distance() {
        let d = edge_path_d(0.0, 0.0, 20.0, 200.0);
        // dx=20, dy=200, cpX = max(10, min(200, 50)) = max(10, 50) = 50
        assert!(d.contains("C 50,0"));
    }

    #[test]
    fn test_port_y_first_port() {
        assert_eq!(port_y(0.0, 0), 36.0); // 30 + 0*20 + 6
    }

    #[test]
    fn test_port_y_second_port() {
        assert_eq!(port_y(0.0, 1), 56.0); // 30 + 1*20 + 6
    }

    #[test]
    fn test_port_y_with_offset() {
        assert_eq!(port_y(100.0, 2), 176.0); // 100 + 30 + 2*20 + 6
    }
}
