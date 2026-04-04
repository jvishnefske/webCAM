//! Machine profile system for CNC mill and laser cutter support.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MachineType {
    #[default]
    CncMill,
    LaserCutter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineCapabilities {
    pub available_strategies: Vec<String>,
    pub has_spindle: bool,
    pub has_laser_power: bool,
    pub has_z_axis: bool,
    pub max_feed_rate: f64,
    pub max_spindle_rpm: Option<f64>,
    pub max_laser_power: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub preamble: Vec<String>,
    pub postamble: Vec<String>,
    pub unit_mode: String,
    pub distance_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineProfile {
    pub name: String,
    pub machine_type: MachineType,
    pub capabilities: MachineCapabilities,
    pub output_config: OutputConfig,
}

impl MachineProfile {
    /// Default CNC mill profile with all current strategies.
    pub fn cnc_mill() -> Self {
        Self {
            name: "CNC Mill".into(),
            machine_type: MachineType::CncMill,
            capabilities: MachineCapabilities {
                available_strategies: vec![
                    "contour".into(),
                    "pocket".into(),
                    "slice".into(),
                    "zigzag".into(),
                    "perimeter".into(),
                ],
                has_spindle: true,
                has_laser_power: false,
                has_z_axis: true,
                max_feed_rate: 10000.0,
                max_spindle_rpm: Some(30000.0),
                max_laser_power: None,
            },
            output_config: OutputConfig {
                preamble: vec!["G21 (metric)".into(), "G90 (absolute positioning)".into()],
                postamble: vec![
                    "M5 (spindle off)".into(),
                    "M9 (coolant off)".into(),
                    "G0 X0 Y0".into(),
                    "M2 (program end)".into(),
                ],
                unit_mode: "G21".into(),
                distance_mode: "G90".into(),
            },
        }
    }

    /// Default laser cutter profile with 2D-only strategies.
    pub fn laser_cutter() -> Self {
        Self {
            name: "Laser Cutter".into(),
            machine_type: MachineType::LaserCutter,
            capabilities: MachineCapabilities {
                available_strategies: vec![
                    "contour".into(),
                    "pocket".into(),
                    "perimeter".into(),
                    "laser_cut".into(),
                    "laser_engrave".into(),
                ],
                has_spindle: false,
                has_laser_power: true,
                has_z_axis: false,
                max_feed_rate: 20000.0,
                max_spindle_rpm: None,
                max_laser_power: Some(100.0),
            },
            output_config: OutputConfig {
                preamble: vec![
                    "G21 (metric)".into(),
                    "G90 (absolute positioning)".into(),
                    "M4 S0 (dynamic laser mode)".into(),
                ],
                postamble: vec![
                    "M5 (laser off)".into(),
                    "G0 X0 Y0".into(),
                    "M2 (program end)".into(),
                ],
                unit_mode: "G21".into(),
                distance_mode: "G90".into(),
            },
        }
    }

    /// Returns true if the given strategy is supported by this profile.
    pub fn supports_strategy(&self, strategy: &str) -> bool {
        self.capabilities
            .available_strategies
            .iter()
            .any(|s| s == strategy)
    }

    /// Validate that a strategy is allowed for this machine type.
    /// Returns an error message if the strategy is rejected.
    pub fn validate_strategy(&self, strategy: &str) -> Result<(), String> {
        // 3D strategies are not valid for laser cutters
        if self.machine_type == MachineType::LaserCutter && matches!(strategy, "zigzag" | "slice") {
            return Err(format!(
                "Strategy '{}' requires Z-axis which laser cutter does not have",
                strategy
            ));
        }
        if !self.supports_strategy(strategy) {
            return Err(format!(
                "Strategy '{}' is not available for {}",
                strategy, self.name
            ));
        }
        Ok(())
    }
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self::cnc_mill()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cnc_mill_has_all_strategies() {
        let profile = MachineProfile::cnc_mill();
        assert!(profile.supports_strategy("contour"));
        assert!(profile.supports_strategy("pocket"));
        assert!(profile.supports_strategy("slice"));
        assert!(profile.supports_strategy("zigzag"));
        assert!(profile.supports_strategy("perimeter"));
    }

    #[test]
    fn laser_rejects_3d_strategies() {
        let profile = MachineProfile::laser_cutter();
        assert!(profile.validate_strategy("zigzag").is_err());
        assert!(profile.validate_strategy("slice").is_err());
    }

    #[test]
    fn laser_accepts_2d_strategies() {
        let profile = MachineProfile::laser_cutter();
        assert!(profile.validate_strategy("contour").is_ok());
        assert!(profile.validate_strategy("pocket").is_ok());
        assert!(profile.validate_strategy("laser_cut").is_ok());
        assert!(profile.validate_strategy("laser_engrave").is_ok());
    }

    #[test]
    fn cnc_mill_has_spindle() {
        let profile = MachineProfile::cnc_mill();
        assert!(profile.capabilities.has_spindle);
        assert!(!profile.capabilities.has_laser_power);
        assert!(profile.capabilities.has_z_axis);
    }

    #[test]
    fn laser_has_laser_power() {
        let profile = MachineProfile::laser_cutter();
        assert!(!profile.capabilities.has_spindle);
        assert!(profile.capabilities.has_laser_power);
        assert!(!profile.capabilities.has_z_axis);
    }

    #[test]
    fn machine_type_serde() {
        let json = serde_json::to_string(&MachineType::CncMill).unwrap();
        assert_eq!(json, "\"cnc_mill\"");
        let json = serde_json::to_string(&MachineType::LaserCutter).unwrap();
        assert_eq!(json, "\"laser_cutter\"");
    }

    #[test]
    fn profile_serde_roundtrip() {
        let profile = MachineProfile::cnc_mill();
        let json = serde_json::to_string(&profile).unwrap();
        let p2: MachineProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(p2.name, "CNC Mill");
        assert_eq!(p2.machine_type, MachineType::CncMill);
    }

    #[test]
    fn machine_profile_default() {
        let p = MachineProfile::default();
        assert!(!p.name.is_empty());
    }
}
