/// G-code emitter.
///
/// Swiss-cheese layer: **Output format**
/// Extension point: implement alternative output formats (HPGL, DXF toolpath,
/// Marlin flavour, GRBL flavour, etc.) by consuming `Vec<Toolpath>`.
use crate::geometry::Toolpath;
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
}
