//! CAM configuration builder.
//!
//! Extracts the JSON config construction logic into pure, testable functions
//! that do not depend on any WASM or DOM APIs.

use serde::{Deserialize, Serialize};

/// Numeric/boolean parameters collected from the CAM form UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamParams {
    pub tool_type: String,
    pub tool_diameter: f64,
    pub corner_radius: f64,
    pub effective_diameter: f64,
    pub step_over: f64,
    pub step_down: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub spindle_speed: f64,
    pub safe_z: f64,
    pub cut_depth: f64,
    pub scan_direction: String,
    /// Surface-3D traversal pattern: "zigzag", "one_way", or "spiral".
    pub pattern: String,
    pub climb_cut: bool,
    pub perimeter_passes: u32,
    pub laser_power: f64,
    pub laser_passes: u32,
    pub air_assist: bool,
}

impl Default for CamParams {
    fn default() -> Self {
        Self {
            tool_type: "end_mill".into(),
            tool_diameter: 3.175,
            corner_radius: 0.0,
            effective_diameter: 3.175,
            step_over: 1.5,
            step_down: 1.0,
            feed_rate: 800.0,
            plunge_rate: 300.0,
            spindle_speed: 12000.0,
            safe_z: 5.0,
            cut_depth: -1.0,
            scan_direction: "x".into(),
            pattern: "zigzag".into(),
            climb_cut: false,
            perimeter_passes: 1,
            laser_power: 100.0,
            laser_passes: 1,
            air_assist: false,
        }
    }
}

/// Default parameters for a CNC mill configuration.
pub fn default_cnc_params() -> CamParams {
    CamParams::default()
}

/// Default parameters for a laser cutter configuration.
pub fn default_laser_params() -> CamParams {
    CamParams {
        tool_type: "end_mill".into(),
        tool_diameter: 0.1,
        step_over: 0.1,
        feed_rate: 1000.0,
        plunge_rate: 1000.0,
        spindle_speed: 0.0,
        safe_z: 0.0,
        cut_depth: 0.0,
        step_down: 1.0,
        laser_power: 100.0,
        laser_passes: 1,
        air_assist: false,
        ..CamParams::default()
    }
}

/// Build a `serde_json::Value` matching the `CamConfig` structure expected by
/// `rustcam::process_stl_impl` / `rustcam::process_svg_impl`.
///
/// The `machine_type` is `"cnc_mill"` or `"laser_cutter"`, and `strategy` is
/// one of the strategy strings accepted by rustcam (e.g. `"contour"`,
/// `"pocket"`, `"laser_cut"`).
pub fn build_cam_config(
    machine_type: &str,
    strategy: &str,
    params: &CamParams,
) -> serde_json::Value {
    let is_laser = machine_type == "laser_cutter";

    let mut config = serde_json::json!({
        "machine_type": machine_type,
        "strategy": strategy,
        "tool_diameter": params.tool_diameter,
        "tool_type": params.tool_type,
        "step_over": params.step_over,
    });

    let obj = config.as_object_mut().expect("json object");

    if is_laser {
        obj.insert("feed_rate".into(), serde_json::json!(params.feed_rate));
        obj.insert("plunge_rate".into(), serde_json::json!(params.feed_rate));
        obj.insert("spindle_speed".into(), serde_json::json!(0));
        obj.insert("safe_z".into(), serde_json::json!(0));
        obj.insert("cut_depth".into(), serde_json::json!(0));
        obj.insert("step_down".into(), serde_json::json!(1));
        obj.insert("laser_power".into(), serde_json::json!(params.laser_power));
        obj.insert("passes".into(), serde_json::json!(params.laser_passes));
        obj.insert("air_assist".into(), serde_json::json!(params.air_assist));
    } else {
        obj.insert("step_down".into(), serde_json::json!(params.step_down));
        obj.insert("feed_rate".into(), serde_json::json!(params.feed_rate));
        obj.insert("plunge_rate".into(), serde_json::json!(params.plunge_rate));
        obj.insert(
            "spindle_speed".into(),
            serde_json::json!(params.spindle_speed),
        );
        obj.insert("safe_z".into(), serde_json::json!(params.safe_z));
        obj.insert("cut_depth".into(), serde_json::json!(params.cut_depth));
    }

    // Tool-type-specific fields.
    if params.tool_type == "ball_end" {
        obj.insert(
            "corner_radius".into(),
            serde_json::json!(params.corner_radius),
        );
    } else if params.tool_type == "face_mill" {
        obj.insert(
            "effective_diameter".into(),
            serde_json::json!(params.effective_diameter),
        );
    }

    // Strategy-specific fields.
    if strategy == "zigzag" || strategy == "surface3d" {
        obj.insert(
            "scan_direction".into(),
            serde_json::json!(params.scan_direction),
        );
        obj.insert("pattern".into(), serde_json::json!(params.pattern));
    }
    if strategy == "perimeter" {
        obj.insert("climb_cut".into(), serde_json::json!(params.climb_cut));
        obj.insert(
            "perimeter_passes".into(),
            serde_json::json!(params.perimeter_passes),
        );
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cnc_config() {
        let params = default_cnc_params();
        let config = build_cam_config("cnc_mill", "contour", &params);

        assert_eq!(config["machine_type"], "cnc_mill");
        assert_eq!(config["strategy"], "contour");
        assert_eq!(config["tool_diameter"], 3.175);
        assert_eq!(config["feed_rate"], 800.0);
        assert_eq!(config["plunge_rate"], 300.0);
        assert_eq!(config["spindle_speed"], 12000.0);
        assert_eq!(config["safe_z"], 5.0);
        assert_eq!(config["cut_depth"], -1.0);
        assert_eq!(config["step_down"], 1.0);
        // Laser fields should not be present.
        assert!(config.get("laser_power").is_none());
        assert!(config.get("passes").is_none());
        assert!(config.get("air_assist").is_none());
    }

    #[test]
    fn test_build_laser_config() {
        let params = CamParams {
            laser_power: 80.0,
            laser_passes: 3,
            air_assist: true,
            feed_rate: 1200.0,
            ..default_laser_params()
        };
        let config = build_cam_config("laser_cutter", "laser_cut", &params);

        assert_eq!(config["machine_type"], "laser_cutter");
        assert_eq!(config["strategy"], "laser_cut");
        assert_eq!(config["laser_power"], 80.0);
        assert_eq!(config["passes"], 3);
        assert_eq!(config["air_assist"], true);
        assert_eq!(config["feed_rate"], 1200.0);
        // Laser overrides: plunge_rate == feed_rate, spindle 0, safe_z 0, cut_depth 0.
        assert_eq!(config["plunge_rate"], 1200.0);
        assert_eq!(config["spindle_speed"], 0);
        assert_eq!(config["safe_z"], 0);
        assert_eq!(config["cut_depth"], 0);
    }

    #[test]
    fn test_default_cnc_params() {
        let params = default_cnc_params();
        assert!((params.tool_diameter - 3.175).abs() < f64::EPSILON);
        assert_eq!(params.tool_type, "end_mill");
        assert!((params.feed_rate - 800.0).abs() < f64::EPSILON);
        assert!((params.plunge_rate - 300.0).abs() < f64::EPSILON);
        assert!((params.spindle_speed - 12000.0).abs() < f64::EPSILON);
        assert!((params.safe_z - 5.0).abs() < f64::EPSILON);
        assert!((params.cut_depth - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_laser_params() {
        let params = default_laser_params();
        assert!((params.laser_power - 100.0).abs() < f64::EPSILON);
        assert_eq!(params.laser_passes, 1);
        assert!(!params.air_assist);
        assert!((params.spindle_speed).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ball_end_corner_radius() {
        let params = CamParams {
            tool_type: "ball_end".into(),
            corner_radius: 1.5875,
            ..default_cnc_params()
        };
        let config = build_cam_config("cnc_mill", "contour", &params);
        assert_eq!(config["corner_radius"], 1.5875);
        assert!(config.get("effective_diameter").is_none());
    }

    #[test]
    fn test_face_mill_effective_diameter() {
        let params = CamParams {
            tool_type: "face_mill".into(),
            effective_diameter: 25.0,
            ..default_cnc_params()
        };
        let config = build_cam_config("cnc_mill", "pocket", &params);
        assert_eq!(config["effective_diameter"], 25.0);
        assert!(config.get("corner_radius").is_none());
    }

    #[test]
    fn test_zigzag_scan_direction() {
        let params = CamParams {
            scan_direction: "y".into(),
            ..default_cnc_params()
        };
        let config = build_cam_config("cnc_mill", "zigzag", &params);
        assert_eq!(config["scan_direction"], "y");
    }

    #[test]
    fn test_req_006_surface3d_pattern_round_trip() {
        // Each of the three supported pattern values must survive the
        // round trip from CamParams → JSON → rustcam::CamConfig.
        for pattern in ["zigzag", "one_way", "spiral"] {
            let params = CamParams {
                pattern: pattern.into(),
                ..default_cnc_params()
            };
            let json = build_cam_config("cnc_mill", "surface3d", &params);
            assert_eq!(
                json["pattern"], pattern,
                "pattern {} must appear in JSON",
                pattern
            );
            let parsed: rustcam::CamConfig =
                serde_json::from_value(json).expect("JSON must deserialize as CamConfig");
            assert_eq!(parsed.pattern, pattern);
            assert_eq!(parsed.strategy, "surface3d");
        }
    }

    #[test]
    fn test_req_006_default_pattern_is_zigzag() {
        let params = default_cnc_params();
        assert_eq!(params.pattern, "zigzag");
    }

    #[test]
    fn test_perimeter_options() {
        let params = CamParams {
            climb_cut: true,
            perimeter_passes: 3,
            ..default_cnc_params()
        };
        let config = build_cam_config("cnc_mill", "perimeter", &params);
        assert_eq!(config["climb_cut"], true);
        assert_eq!(config["perimeter_passes"], 3);
    }

    #[test]
    fn test_config_deserializes_as_cam_config() {
        // Verify the JSON we produce can be deserialized by rustcam::CamConfig.
        let params = default_cnc_params();
        let json_val = build_cam_config("cnc_mill", "contour", &params);
        let json_str = serde_json::to_string(&json_val).unwrap();
        let parsed: rustcam::CamConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.machine_type, "cnc_mill");
        assert_eq!(parsed.strategy, "contour");
        assert!((parsed.tool_diameter - 3.175).abs() < f64::EPSILON);
    }

    #[test]
    fn test_laser_config_deserializes_as_cam_config() {
        let params = default_laser_params();
        let json_val = build_cam_config("laser_cutter", "laser_cut", &params);
        let json_str = serde_json::to_string(&json_val).unwrap();
        let parsed: rustcam::CamConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.machine_type, "laser_cutter");
        assert_eq!(parsed.strategy, "laser_cut");
        assert_eq!(parsed.laser_power, Some(100.0));
        assert_eq!(parsed.passes, Some(1));
        assert_eq!(parsed.air_assist, Some(false));
    }
}
