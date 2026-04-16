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
    /// Type selector widget — renders a dropdown of DAG scalar types
    /// (e.g. "f32", "i32", "bool"). The selected value is stored as a
    /// type name string that maps to a [`dag_core::types::DagType`] at
    /// lowering time.
    TypeSelector,
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

    // --- DeclaredChannel tests ---

    #[test]
    fn test_declared_channel_with_channel_type_serde_roundtrip() {
        let ch = DeclaredChannel {
            name: "motor/speed".into(),
            direction: ChannelDirection::Output,
            kind: ChannelKind::PubSub,
            channel_type: Some("i32".into()),
        };
        let json = serde_json::to_string(&ch).expect("serialize");
        let restored: DeclaredChannel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "motor/speed");
        assert_eq!(restored.direction, ChannelDirection::Output);
        assert_eq!(restored.kind, ChannelKind::PubSub);
        assert_eq!(restored.channel_type, Some("i32".into()));
    }

    #[test]
    fn test_declared_channel_with_channel_type_none_serde_roundtrip() {
        let ch = DeclaredChannel {
            name: "adc0".into(),
            direction: ChannelDirection::Input,
            kind: ChannelKind::Hardware,
            channel_type: None,
        };
        let json = serde_json::to_string(&ch).expect("serialize");
        let restored: DeclaredChannel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "adc0");
        assert_eq!(restored.direction, ChannelDirection::Input);
        assert_eq!(restored.kind, ChannelKind::Hardware);
        assert_eq!(restored.channel_type, None);
    }

    #[test]
    fn test_declared_channel_none_skipped_in_json() {
        // When channel_type is None, it should not appear in serialized JSON
        let ch = DeclaredChannel {
            name: "x".into(),
            direction: ChannelDirection::Input,
            kind: ChannelKind::PubSub,
            channel_type: None,
        };
        let json = serde_json::to_string(&ch).expect("serialize");
        assert!(
            !json.contains("channel_type"),
            "None channel_type should be skipped: {json}"
        );
    }

    #[test]
    fn test_declared_channel_backward_compat_missing_channel_type() {
        // Old JSON without channel_type should deserialize with channel_type = None
        let json = r#"{"name":"x","direction":"input","kind":"pub_sub"}"#;
        let ch: DeclaredChannel = serde_json::from_str(json).expect("deserialize old format");
        assert_eq!(ch.name, "x");
        assert_eq!(ch.channel_type, None);
    }

    #[test]
    fn test_declared_channel_channel_type_f32() {
        let ch = DeclaredChannel {
            name: "sensor/temp".into(),
            direction: ChannelDirection::Output,
            kind: ChannelKind::PubSub,
            channel_type: Some("f32".into()),
        };
        // Verify type name can be resolved via DagType::from_name
        let dag_type = dag_core::types::DagType::from_name(ch.channel_type.as_deref().unwrap());
        assert_eq!(dag_type, Some(dag_core::types::DagType::F32));
    }

    // --- FieldKind tests ---

    #[test]
    fn test_field_kind_type_selector_serde_roundtrip() {
        let kind = FieldKind::TypeSelector;
        let json = serde_json::to_string(&kind).expect("serialize TypeSelector");
        let restored: FieldKind = serde_json::from_str(&json).expect("deserialize TypeSelector");
        assert_eq!(kind, restored);
        assert_eq!(json, "\"type_selector\""); // snake_case rename
    }

    #[test]
    fn test_field_kind_existing_variants_unchanged() {
        // Verify existing FieldKind variants still serialize/deserialize correctly
        let cases = vec![
            (FieldKind::Float, "\"float\""),
            (FieldKind::Int, "\"int\""),
            (FieldKind::Text, "\"text\""),
            (FieldKind::Bool, "\"bool\""),
        ];
        for (kind, expected_json) in &cases {
            let json = serde_json::to_string(kind).expect("serialize");
            assert_eq!(
                &json, expected_json,
                "FieldKind::{kind:?} serialization mismatch"
            );
            let restored: FieldKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(kind, &restored);
        }
    }

    #[test]
    fn test_field_kind_select_unchanged() {
        let kind = FieldKind::Select(vec!["a".into(), "b".into()]);
        let json = serde_json::to_string(&kind).expect("serialize Select");
        let restored: FieldKind = serde_json::from_str(&json).expect("deserialize Select");
        assert_eq!(kind, restored);
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
    /// Optional DAG type name (e.g. "i32", "f32", "bool").
    ///
    /// When `None`, the channel defaults to `f64` (the standard DAG scalar).
    /// The string maps to a [`dag_core::types::DagType`] at lowering time via
    /// [`DagType::from_name`](dag_core::types::DagType::from_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<String>,
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
