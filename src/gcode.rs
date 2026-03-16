/// G-code emitter.
///
/// Swiss-cheese layer: **Output format**
/// Extension point: implement alternative output formats (HPGL, DXF toolpath,
/// Marlin flavour, GRBL flavour, etc.) by consuming `Vec<Toolpath>`.
use crate::gcode_parser::{parse_line, validate_command, ParseError, ValidationConfig};
use crate::geometry::Toolpath;
use crate::machine::{MachineProfile, MachineType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcodeParams {
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub spindle_speed: f64,
    pub safe_z: f64,
    pub unit_mm: bool,
}

impl Default for GcodeParams {
    fn default() -> Self {
        Self {
            feed_rate: 800.0,
            plunge_rate: 300.0,
            spindle_speed: 12000.0,
            safe_z: 5.0,
            unit_mm: true,
        }
    }
}

pub fn emit_gcode(toolpaths: &[Toolpath], params: &GcodeParams) -> String {
    let mut out = String::with_capacity(4096);

    // Header
    out.push_str("(RustCAM — generated G-code)\n");
    if params.unit_mm {
        out.push_str("G21 (metric)\n");
    } else {
        out.push_str("G20 (imperial)\n");
    }
    out.push_str("G90 (absolute positioning)\n");
    out.push_str(&format!("G0 Z{:.3}\n", params.safe_z));
    out.push_str(&format!("M3 S{:.0} (spindle on)\n", params.spindle_speed));
    out.push('\n');

    for (idx, tp) in toolpaths.iter().enumerate() {
        out.push_str(&format!("(Toolpath {})\n", idx + 1));
        let mut last_rapid = true; // track state to avoid redundant F words

        for mv in &tp.moves {
            if mv.rapid {
                out.push_str(&format!("G0 X{:.4} Y{:.4} Z{:.4}\n", mv.x, mv.y, mv.z));
                last_rapid = true;
            } else {
                let feed = if mv.z < params.safe_z - 0.01 && last_rapid {
                    params.plunge_rate
                } else {
                    params.feed_rate
                };
                out.push_str(&format!(
                    "G1 X{:.4} Y{:.4} Z{:.4} F{:.0}\n",
                    mv.x, mv.y, mv.z, feed
                ));
                last_rapid = false;
            }
        }
        out.push('\n');
    }

    // Footer
    out.push_str(&format!("G0 Z{:.3}\n", params.safe_z));
    out.push_str("M5 (spindle off)\n");
    out.push_str("G0 X0 Y0\n");
    out.push_str("M2 (program end)\n");

    out
}

/// Extended params for profile-aware emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaserParams {
    pub power: f64,
    pub passes: u32,
    #[serde(default)]
    pub air_assist: bool,
}

impl Default for LaserParams {
    fn default() -> Self {
        Self {
            power: 100.0,
            passes: 1,
            air_assist: false,
        }
    }
}

/// Emit G-code using a machine profile for CNC/laser-specific output.
pub fn emit_gcode_with_profile(
    toolpaths: &[Toolpath],
    params: &GcodeParams,
    profile: &MachineProfile,
    laser_params: Option<&LaserParams>,
) -> String {
    match profile.machine_type {
        MachineType::CncMill => emit_gcode_cnc(toolpaths, params, profile),
        MachineType::LaserCutter => emit_gcode_laser(
            toolpaths,
            params,
            profile,
            laser_params.unwrap_or(&LaserParams::default()),
        ),
    }
}

fn emit_gcode_cnc(
    toolpaths: &[Toolpath],
    params: &GcodeParams,
    profile: &MachineProfile,
) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("(RustCAM — generated G-code)\n");
    for line in &profile.output_config.preamble {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&format!("G0 Z{:.3}\n", params.safe_z));
    out.push_str(&format!("M3 S{:.0} (spindle on)\n", params.spindle_speed));
    out.push('\n');

    for (idx, tp) in toolpaths.iter().enumerate() {
        out.push_str(&format!("(Toolpath {})\n", idx + 1));
        let mut last_rapid = true;

        for mv in &tp.moves {
            if mv.rapid {
                out.push_str(&format!("G0 X{:.4} Y{:.4} Z{:.4}\n", mv.x, mv.y, mv.z));
                last_rapid = true;
            } else {
                let feed = if mv.z < params.safe_z - 0.01 && last_rapid {
                    params.plunge_rate
                } else {
                    params.feed_rate
                };
                out.push_str(&format!(
                    "G1 X{:.4} Y{:.4} Z{:.4} F{:.0}\n",
                    mv.x, mv.y, mv.z, feed
                ));
                last_rapid = false;
            }
        }
        out.push('\n');
    }

    out.push_str(&format!("G0 Z{:.3}\n", params.safe_z));
    for line in &profile.output_config.postamble {
        out.push_str(line);
        out.push('\n');
    }

    out
}

fn emit_gcode_laser(
    toolpaths: &[Toolpath],
    params: &GcodeParams,
    profile: &MachineProfile,
    laser_params: &LaserParams,
) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("(RustCAM — generated G-code)\n");
    for line in &profile.output_config.preamble {
        out.push_str(line);
        out.push('\n');
    }
    if laser_params.air_assist {
        out.push_str("M8 (air assist on)\n");
    }
    out.push('\n');

    for pass in 0..laser_params.passes {
        if laser_params.passes > 1 {
            out.push_str(&format!("(Pass {} of {})\n", pass + 1, laser_params.passes));
        }

        for (idx, tp) in toolpaths.iter().enumerate() {
            out.push_str(&format!("(Toolpath {})\n", idx + 1));

            for mv in &tp.moves {
                if mv.rapid {
                    // Laser off during rapids
                    out.push_str(&format!("G0 X{:.4} Y{:.4} S0\n", mv.x, mv.y));
                } else {
                    // Use move-specific power if available, otherwise default
                    let power = mv.power.unwrap_or(laser_params.power);
                    out.push_str(&format!(
                        "G1 X{:.4} Y{:.4} F{:.0} S{:.0}\n",
                        mv.x, mv.y, params.feed_rate, power
                    ));
                }
            }
            out.push('\n');
        }
    }

    if laser_params.air_assist {
        out.push_str("M9 (air assist off)\n");
    }
    for line in &profile.output_config.postamble {
        out.push_str(line);
        out.push('\n');
    }

    out
}

/// A warning produced during G-code validation.
#[derive(Debug, Clone)]
pub struct GcodeWarning {
    pub line_number: usize,
    pub message: String,
}

/// Validate generated G-code by parsing and validating each line.
/// Returns any warnings found alongside the original G-code.
pub fn validate_gcode(gcode: &str, profile: &MachineProfile) -> Vec<GcodeWarning> {
    let mut warnings = Vec::new();

    let config = ValidationConfig {
        max_feed_rate: profile.capabilities.max_feed_rate,
        max_spindle_speed: profile.capabilities.max_spindle_rpm.unwrap_or(30000.0) as u32,
        ..Default::default()
    };

    for (i, line) in gcode.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_line(trimmed) {
            Ok(cmd) => {
                if let Err(e) = validate_command(&cmd, &config) {
                    warnings.push(GcodeWarning {
                        line_number: line_num,
                        message: e.to_string(),
                    });
                }

                // Profile-specific checks: Z movement in laser mode
                if profile.machine_type == MachineType::LaserCutter && cmd.is_motion() {
                    if let crate::gcode_parser::GCodeCommand::RapidMove { z: Some(z), .. }
                    | crate::gcode_parser::GCodeCommand::LinearMove { z: Some(z), .. } = &cmd
                    {
                        if z.abs() > 0.001 {
                            warnings.push(GcodeWarning {
                                line_number: line_num,
                                message: format!(
                                    "Z movement ({z:.3}) in laser mode - laser has no Z axis"
                                ),
                            });
                        }
                    }
                }
            }
            Err(ParseError::EmptyLine) => {}
            Err(e) => {
                warnings.push(GcodeWarning {
                    line_number: line_num,
                    message: format!("Parse error: {e}"),
                });
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_toolpaths() {
        let code = emit_gcode(&[], &GcodeParams::default());
        assert!(code.contains("G21"));
        assert!(code.contains("M2"));
    }

    #[test]
    fn test_single_move() {
        let mut tp = Toolpath::new();
        tp.rapid(10.0, 20.0, 5.0);
        tp.cut(10.0, 20.0, -1.0);
        tp.cut(30.0, 20.0, -1.0);
        tp.rapid(30.0, 20.0, 5.0);

        let code = emit_gcode(&[tp], &GcodeParams::default());
        assert!(code.contains("G0 X10.0000 Y20.0000 Z5.0000"));
        assert!(code.contains("G1 X30.0000 Y20.0000 Z-1.0000"));
    }

    #[test]
    fn test_cnc_profile_emitter() {
        let profile = MachineProfile::cnc_mill();
        let mut tp = Toolpath::new();
        tp.rapid(10.0, 0.0, 5.0);
        tp.cut(10.0, 0.0, -1.0);
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, None);
        assert!(code.contains("M3 S12000"));
        assert!(code.contains("M5 (spindle off)"));
        assert!(code.contains("M2 (program end)"));
    }

    #[test]
    fn test_laser_profile_emitter() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.rapid(10.0, 0.0, 0.0);
        tp.cut(20.0, 0.0, 0.0);
        let laser = LaserParams {
            power: 80.0,
            passes: 1,
            ..Default::default()
        };
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        assert!(code.contains("M4 S0"), "Should have dynamic laser mode");
        assert!(code.contains("S0\n"), "Rapids should have S0");
        assert!(code.contains("S80"), "Cuts should have power");
        assert!(!code.contains("Z"), "Laser should not have Z moves");
    }

    #[test]
    fn test_laser_multi_pass() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.cut(10.0, 0.0, 0.0);
        let laser = LaserParams {
            power: 50.0,
            passes: 3,
            ..Default::default()
        };
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        assert!(code.contains("Pass 1 of 3"));
        assert!(code.contains("Pass 3 of 3"));
    }

    #[test]
    fn test_laser_move_power() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.cut_with_power(10.0, 0.0, 0.0, 42.0);
        let laser = LaserParams {
            power: 80.0,
            passes: 1,
            ..Default::default()
        };
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        assert!(
            code.contains("S42"),
            "Should use move-specific power, not default"
        );
    }

    // ── Validation tests ──────────────────────────────────────────────

    #[test]
    fn test_validate_cnc_gcode_clean() {
        let profile = MachineProfile::cnc_mill();
        let mut tp = Toolpath::new();
        tp.rapid(10.0, 0.0, 5.0);
        tp.cut(10.0, 0.0, -1.0);
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, None);
        let warnings = validate_gcode(&code, &profile);
        assert!(
            warnings.is_empty(),
            "Clean CNC G-code should have no warnings: {:?}",
            warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_validate_laser_gcode_clean() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.rapid(10.0, 0.0, 0.0);
        tp.cut(20.0, 0.0, 0.0);
        let laser = LaserParams {
            power: 80.0,
            passes: 1,
            ..Default::default()
        };
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        let warnings = validate_gcode(&code, &profile);
        assert!(
            warnings.is_empty(),
            "Clean laser G-code should have no warnings: {:?}",
            warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_validate_detects_z_in_laser_mode() {
        let profile = MachineProfile::laser_cutter();
        // Manually crafted G-code with Z movement
        let gcode = "G21\nG90\nG0 X10 Y0 Z5\nG1 X20 Y0 Z-1 F800\nM2\n";
        let warnings = validate_gcode(gcode, &profile);
        let z_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.message.contains("Z movement"))
            .collect();
        assert!(!z_warnings.is_empty(), "Should warn about Z in laser mode");
    }

    #[test]
    fn test_laser_air_assist() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.cut(10.0, 0.0, 0.0);
        let laser = LaserParams {
            power: 80.0,
            passes: 1,
            air_assist: true,
        };
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        assert!(code.contains("M8 (air assist on)"), "Should enable air assist");
        assert!(code.contains("M9 (air assist off)"), "Should disable air assist");
    }

    #[test]
    fn test_laser_no_air_assist_by_default() {
        let profile = MachineProfile::laser_cutter();
        let mut tp = Toolpath::new();
        tp.cut(10.0, 0.0, 0.0);
        let laser = LaserParams::default();
        let code = emit_gcode_with_profile(&[tp], &GcodeParams::default(), &profile, Some(&laser));
        assert!(!code.contains("M8"), "Should not have air assist by default");
        assert!(!code.contains("M9"), "Should not have M9 by default");
    }
}
