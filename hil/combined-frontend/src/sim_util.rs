//! Pure utility functions for simulation transport controls.
//!
//! This module is not gated on `wasm32` so that its tests can run on the host.

/// Convert a speed multiplier to interval in milliseconds.
///
/// Base interval is 100ms (10 Hz at 1x). Higher speeds shorten the interval.
pub fn speed_to_interval_ms(speed: f64) -> u32 {
    let ms = 100.0 / speed;
    ms.round() as u32
}

/// Format simulation time from tick count and dt.
pub fn format_sim_time(tick_count: u64, dt: f64) -> String {
    let t = tick_count as f64 * dt;
    if t < 1.0 {
        format!("{:.3}s", t)
    } else if t < 60.0 {
        format!("{:.2}s", t)
    } else {
        let mins = (t / 60.0).floor() as u64;
        let secs = t - (mins as f64 * 60.0);
        format!("{}m {:.1}s", mins, secs)
    }
}

/// Available speed multiplier presets.
pub const SPEED_PRESETS: &[(f64, &str)] = &[
    (0.25, "0.25x"),
    (0.5, "0.5x"),
    (1.0, "1x"),
    (2.0, "2x"),
    (4.0, "4x"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_to_interval_1x() {
        assert_eq!(speed_to_interval_ms(1.0), 100);
    }

    #[test]
    fn test_speed_to_interval_2x() {
        assert_eq!(speed_to_interval_ms(2.0), 50);
    }

    #[test]
    fn test_speed_to_interval_quarter() {
        assert_eq!(speed_to_interval_ms(0.25), 400);
    }

    #[test]
    fn test_speed_to_interval_half() {
        assert_eq!(speed_to_interval_ms(0.5), 200);
    }

    #[test]
    fn test_speed_to_interval_4x() {
        assert_eq!(speed_to_interval_ms(4.0), 25);
    }

    #[test]
    fn test_format_sim_time_subsecond() {
        assert_eq!(format_sim_time(50, 0.01), "0.500s");
    }

    #[test]
    fn test_format_sim_time_seconds() {
        assert_eq!(format_sim_time(100, 0.01), "1.00s");
    }

    #[test]
    fn test_format_sim_time_minutes() {
        // 6000 ticks * 0.01 = 60s = 1m 0.0s
        assert_eq!(format_sim_time(6000, 0.01), "1m 0.0s");
    }

    #[test]
    fn test_format_sim_time_zero() {
        assert_eq!(format_sim_time(0, 0.01), "0.000s");
    }

    #[test]
    fn test_format_sim_time_large_dt() {
        // 10 ticks * 1.0 = 10s
        assert_eq!(format_sim_time(10, 1.0), "10.00s");
    }
}
