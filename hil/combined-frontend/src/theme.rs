// Design tokens -- dark theme matching the TypeScript frontend.

// Colors
pub const BG: &str = "#0a0a0a";
pub const SURFACE: &str = "#1a1a1a";
pub const SURFACE_HOVER: &str = "#252525";
pub const BORDER: &str = "#2a2a2a";
pub const ACCENT: &str = "#3b82f6";
pub const ACCENT_HOVER: &str = "#2563eb";
pub const TEXT: &str = "#e5e7eb";
pub const TEXT_DIM: &str = "#9ca3af";
pub const TEXT_BRIGHT: &str = "#f9fafb";
pub const SUCCESS: &str = "#22c55e";
pub const WARNING: &str = "#f59e0b";
pub const DANGER: &str = "#ef4444";

// Port type colors
pub const PORT_FLOAT: &str = "#22d3ee";
pub const PORT_BYTES: &str = "#f59e0b";
pub const PORT_TEXT: &str = "#4ade80";
pub const PORT_SERIES: &str = "#a78bfa";
pub const PORT_ANY: &str = "#9ca3af";

// Wire colors
pub const WIRE_DEFAULT: &str = "#6b7280";
pub const WIRE_ACTIVE: &str = "#3b82f6";
pub const WIRE_SELECTED: &str = "#f59e0b";

// Spacing (px)
pub const SIDEBAR_WIDTH: f64 = 320.0;
pub const HEADER_HEIGHT: f64 = 49.0;
pub const PORT_RADIUS: f64 = 6.0;
pub const PORT_SPACING: f64 = 20.0;
pub const NODE_BASE_HEIGHT: f64 = 40.0;
pub const NODE_WIDTH: f64 = 190.0;

/// Returns the color for a port kind string.
pub fn port_color(kind: &str) -> &'static str {
    match kind {
        "Float" => PORT_FLOAT,
        "Bytes" => PORT_BYTES,
        "Text" => PORT_TEXT,
        "Series" => PORT_SERIES,
        _ => PORT_ANY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_color_float() {
        assert_eq!(port_color("Float"), PORT_FLOAT);
    }

    #[test]
    fn test_port_color_bytes() {
        assert_eq!(port_color("Bytes"), PORT_BYTES);
    }

    #[test]
    fn test_port_color_unknown_returns_any() {
        assert_eq!(port_color("Unknown"), PORT_ANY);
    }

    #[test]
    fn test_port_color_all_kinds() {
        assert_eq!(port_color("Text"), PORT_TEXT);
        assert_eq!(port_color("Series"), PORT_SERIES);
        assert_eq!(port_color("Any"), PORT_ANY);
    }
}
