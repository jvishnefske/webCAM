//! Configuration schema for user-editable block parameters.

use serde::{Deserialize, Serialize};

/// A single configurable field in a block's parameter form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    /// Machine-readable key (e.g. "kp", "setpoint_topic").
    pub key: String,
    /// Human-readable label (e.g. "Proportional Gain").
    pub label: String,
    /// Field type determines the UI widget.
    pub kind: FieldKind,
    /// Default value as a JSON value.
    pub default: serde_json::Value,
}

/// Widget type for a configuration field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// Floating-point number input.
    Float,
    /// Integer number input.
    Int,
    /// Text/string input (used for topic names, channel names).
    Text,
    /// Boolean toggle.
    Bool,
    /// Select from a list of string options.
    Select(Vec<String>),
}

/// Block category for sub-menu grouping in the palette.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockCategory {
    /// Math and signal processing (gain, filter, integrator).
    Math,
    /// Control systems (PID, state machine, scheduler).
    Control,
    /// I/O and hardware (ADC, PWM, GPIO, encoder).
    Io,
    /// Pub/Sub messaging (subscribe, publish, bridge).
    PubSub,
    /// Monitoring and visualization (plot, logger).
    Monitor,
}

impl BlockCategory {
    pub fn label(&self) -> &'static str {
        match self {
            BlockCategory::Math => "Math",
            BlockCategory::Control => "Control",
            BlockCategory::Io => "I/O",
            BlockCategory::PubSub => "Pub/Sub",
            BlockCategory::Monitor => "Monitor",
        }
    }

    pub fn all() -> &'static [BlockCategory] {
        &[
            BlockCategory::Math,
            BlockCategory::Control,
            BlockCategory::Io,
            BlockCategory::PubSub,
            BlockCategory::Monitor,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_category_label() {
        assert_eq!(BlockCategory::Math.label(), "Math");
        assert_eq!(BlockCategory::Control.label(), "Control");
        assert_eq!(BlockCategory::Io.label(), "I/O");
        assert_eq!(BlockCategory::PubSub.label(), "Pub/Sub");
        assert_eq!(BlockCategory::Monitor.label(), "Monitor");
    }

    #[test]
    fn test_block_category_all() {
        let all = BlockCategory::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&BlockCategory::Math));
        assert!(all.contains(&BlockCategory::Monitor));
    }
}

/// Pubsub or hardware channel declared by a configurable block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclaredChannel {
    /// Channel name (e.g. "motor/setpoint", "adc0").
    pub name: String,
    /// Whether this is an input (subscribe/read) or output (publish/write).
    pub direction: ChannelDirection,
    /// Channel kind — pubsub topic or hardware I/O.
    pub kind: ChannelKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    /// Pub/Sub topic (maps to DAG Subscribe/Publish ops).
    PubSub,
    /// Hardware I/O channel (maps to DAG Input/Output ops).
    Hardware,
}
