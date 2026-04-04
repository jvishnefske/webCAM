/// Tool definitions for CAM operations.
///
/// Swiss-cheese layer: **Tool geometry**
/// Extension point: add new tool types (V-bit, drill, etc.) by extending ToolType.
use serde::{Deserialize, Serialize};

/// Type of cutting tool with type-specific parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolType {
    /// Standard end mill with flat or corner-radiused bottom.
    #[default]
    EndMill,
    /// Ball-end mill for 3D surface finishing.
    BallEnd,
    /// Face mill with effective cutting diameter.
    FaceMill {
        /// Effective cutting width (may differ from body diameter).
        effective_diameter: f64,
    },
}

/// Cutting tool definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Type of tool (end mill, ball end, face mill).
    pub tool_type: ToolType,
    /// Tool diameter in mm.
    pub diameter: f64,
    /// Flute length in mm.
    pub flute_length: f64,
    /// Corner radius in mm (0 for sharp corners, equals radius for ball end).
    pub corner_radius: f64,
}

impl Default for Tool {
    fn default() -> Self {
        Self {
            tool_type: ToolType::EndMill,
            diameter: 3.175, // 1/8" end mill
            flute_length: 10.0,
            corner_radius: 0.0,
        }
    }
}

impl Tool {
    /// Create a new tool with specified parameters.
    pub fn new(tool_type: ToolType, diameter: f64, flute_length: f64, corner_radius: f64) -> Self {
        Self {
            tool_type,
            diameter,
            flute_length,
            corner_radius,
        }
    }

    /// Create a ball-end mill (corner radius equals half diameter).
    pub fn ball_end(diameter: f64, flute_length: f64) -> Self {
        Self {
            tool_type: ToolType::BallEnd,
            diameter,
            flute_length,
            corner_radius: diameter / 2.0,
        }
    }

    /// Create a face mill with effective cutting diameter.
    pub fn face_mill(diameter: f64, effective_diameter: f64, flute_length: f64) -> Self {
        Self {
            tool_type: ToolType::FaceMill { effective_diameter },
            diameter,
            flute_length,
            corner_radius: 0.0,
        }
    }

    /// Get the effective cutting diameter (for face mills, this may differ from body diameter).
    pub fn effective_diameter(&self) -> f64 {
        match &self.tool_type {
            ToolType::FaceMill { effective_diameter } => *effective_diameter,
            _ => self.diameter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_default() {
        let tool = Tool::default();
        assert_eq!(tool.tool_type, ToolType::EndMill);
        assert!((tool.diameter - 3.175).abs() < 0.001);
        assert!((tool.flute_length - 10.0).abs() < 0.001);
        assert!((tool.corner_radius - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_tool_new() {
        let tool = Tool::new(ToolType::EndMill, 6.0, 20.0, 0.5);
        assert_eq!(tool.tool_type, ToolType::EndMill);
        assert!((tool.diameter - 6.0).abs() < 0.001);
        assert!((tool.flute_length - 20.0).abs() < 0.001);
        assert!((tool.corner_radius - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_ball_end() {
        let tool = Tool::ball_end(6.0, 15.0);
        assert_eq!(tool.tool_type, ToolType::BallEnd);
        assert!((tool.diameter - 6.0).abs() < 0.001);
        assert!((tool.corner_radius - 3.0).abs() < 0.001); // radius = diameter / 2
    }

    #[test]
    fn test_face_mill() {
        let tool = Tool::face_mill(50.0, 40.0, 10.0);
        assert!(
            matches!(tool.tool_type, ToolType::FaceMill { effective_diameter } if (effective_diameter - 40.0).abs() < 0.001)
        );
        assert!((tool.diameter - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_effective_diameter() {
        let end_mill = Tool::default();
        assert!((end_mill.effective_diameter() - 3.175).abs() < 0.001);

        let face_mill = Tool::face_mill(50.0, 40.0, 10.0);
        assert!((face_mill.effective_diameter() - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_tool_type_default() {
        let tt = ToolType::default();
        assert_eq!(tt, ToolType::EndMill);
    }
}
