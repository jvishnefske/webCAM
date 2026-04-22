//! I2C device blocks — starting with the TCA9548A bus multiplexer.

use dag_core::op::{Dag, DagError};
use dag_core::templates::BlockPorts;
use serde::{Deserialize, Serialize};

use crate::lower::{ConfigurableBlock, LowerResult};
use crate::schema::{
    BlockCategory, ChannelDirection, ChannelKind, ConfigField, DeclaredChannel, FieldKind,
};

/// I2C bus multiplexer block (TCA9548A-style).
///
/// Routes I2C transactions from a single upstream bus to one of N downstream
/// channel buses. The active channel is selected by writing the channel
/// bitmask to the mux's own I2C address.
///
/// Config:
/// - `address`: I2C address of the mux (0x70–0x77, default 0x70)
/// - `channels`: Number of downstream channels (2, 4, or 8; default 8)
/// - `bus_topic`: PubSub topic for the upstream I2C bus
///
/// Channels:
/// - Input: upstream I2C bus (hardware)
/// - Outputs: one per downstream channel (hardware)
/// - Config output: channel select register (pubsub, for codegen/runtime)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2cMuxBlock {
    /// I2C address (0x70–0x77).
    pub address: u8,
    /// Number of downstream channels (2, 4, or 8).
    pub num_channels: u8,
    /// PubSub topic for active channel selection (0-based index).
    pub select_topic: String,
    /// PubSub topic for upstream bus status.
    pub bus_topic: String,
}

impl Default for I2cMuxBlock {
    fn default() -> Self {
        Self {
            address: 0x70,
            num_channels: 8,
            select_topic: "i2c_mux/select".into(),
            bus_topic: "i2c_mux/bus".into(),
        }
    }
}

impl ConfigurableBlock for I2cMuxBlock {
    fn block_type(&self) -> &str {
        "i2c_mux"
    }

    fn display_name(&self) -> &str {
        "I2C Mux (TCA9548A)"
    }

    fn category(&self) -> BlockCategory {
        BlockCategory::Io
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "address".into(),
                label: "I2C Address (hex)".into(),
                kind: FieldKind::Int,
                default: serde_json::Value::Number(self.address.into()),
            },
            ConfigField {
                key: "num_channels".into(),
                label: "Channels".into(),
                kind: FieldKind::Select(vec!["2".into(), "4".into(), "8".into()]),
                default: serde_json::Value::Number(self.num_channels.into()),
            },
            ConfigField {
                key: "select_topic".into(),
                label: "Select Topic".into(),
                kind: FieldKind::Text,
                default: self.select_topic.clone().into(),
            },
            ConfigField {
                key: "bus_topic".into(),
                label: "Bus Topic".into(),
                kind: FieldKind::Text,
                default: self.bus_topic.clone().into(),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("address").and_then(|v| v.as_u64()) {
            self.address = v as u8;
        }
        if let Some(v) = config.get("num_channels").and_then(|v| v.as_u64()) {
            let n = v as u8;
            if n == 2 || n == 4 || n == 8 {
                self.num_channels = n;
            }
        }
        if let Some(s) = config.get("select_topic").and_then(|v| v.as_str()) {
            self.select_topic = s.into();
        }
        if let Some(s) = config.get("bus_topic").and_then(|v| v.as_str()) {
            self.bus_topic = s.into();
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        let mut channels = vec![
            // Upstream bus — hardware input
            DeclaredChannel {
                name: self.bus_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::Hardware,
                channel_type: None,
            },
            // Channel select — pubsub input (runtime selects active channel)
            DeclaredChannel {
                name: self.select_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::PubSub,
                channel_type: None,
            },
        ];

        // One hardware output per downstream channel
        for i in 0..self.num_channels {
            channels.push(DeclaredChannel {
                name: format!("{}/ch{}", self.bus_topic, i),
                direction: ChannelDirection::Output,
                kind: ChannelKind::Hardware,
                channel_type: None,
            });
        }

        channels
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();

        // Subscribe to the channel-select topic (runtime writes 0–7)
        let select_node = dag.subscribe(&self.select_topic)?;

        // Upstream bus as a DAG Input
        let bus_input = dag.input(&self.bus_topic)?;

        // For each downstream channel, publish a "channel active" indicator:
        // channel_active_i = (select == i) ? 1.0 : 0.0
        // Approximated as: relu(1 - abs(select - i))
        // which is 1.0 when select == i and 0.0 otherwise (for integer select)
        //
        // Simpler: just publish the select value — the runtime/codegen uses it
        // to set the mux register. Each channel output gets the bus input passed
        // through, gated by the mux hardware.

        let mut outputs = Vec::new();
        for i in 0..self.num_channels {
            let ch_name = format!("{}/ch{}", self.bus_topic, i);
            let ch_const = dag.constant(i as f64)?;
            // select == i check: (select - i) == 0 → use sub, then negate via
            // comparison. Since DAG has no compare op, we publish (select, bus)
            // and let the codegen/runtime handle the mux logic.
            //
            // For DAG-level simulation: publish the select value on each channel
            // topic so downstream blocks can read which channel is active.
            dag.publish(&ch_name, select_node)?;
            outputs.push((ch_name, ch_const));
        }

        Ok(LowerResult {
            dag,
            ports: BlockPorts {
                inputs: vec![("bus".into(), bus_input), ("select".into(), select_node)],
                outputs,
            },
        })
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dag_core::op::Op;

    // ── Default construction ────────────────────────────────────────────

    #[test]
    fn test_default_address_is_0x70() {
        let mux = I2cMuxBlock::default();
        assert_eq!(mux.address, 0x70);
    }

    #[test]
    fn test_default_num_channels_is_8() {
        let mux = I2cMuxBlock::default();
        assert_eq!(mux.num_channels, 8);
    }

    #[test]
    fn test_block_type() {
        let mux = I2cMuxBlock::default();
        assert_eq!(mux.block_type(), "i2c_mux");
    }

    #[test]
    fn test_display_name() {
        let mux = I2cMuxBlock::default();
        assert_eq!(mux.display_name(), "I2C Mux (TCA9548A)");
    }

    #[test]
    fn test_category_is_io() {
        let mux = I2cMuxBlock::default();
        assert_eq!(mux.category(), BlockCategory::Io);
    }

    // ── Config schema ───────────────────────────────────────────────────

    #[test]
    fn test_config_schema_has_four_fields() {
        let mux = I2cMuxBlock::default();
        let schema = mux.config_schema();
        assert_eq!(schema.len(), 4);
        assert_eq!(schema[0].key, "address");
        assert_eq!(schema[1].key, "num_channels");
        assert_eq!(schema[2].key, "select_topic");
        assert_eq!(schema[3].key, "bus_topic");
    }

    #[test]
    fn test_num_channels_field_is_select_with_options() {
        let mux = I2cMuxBlock::default();
        let schema = mux.config_schema();
        match &schema[1].kind {
            FieldKind::Select(opts) => {
                assert_eq!(
                    opts,
                    &vec!["2".to_string(), "4".to_string(), "8".to_string()]
                );
            }
            other => panic!("expected Select, got {:?}", other),
        }
    }

    // ── Config JSON round-trip ──────────────────────────────────────────

    #[test]
    fn test_config_json_roundtrip() {
        let mux = I2cMuxBlock::default();
        let json = mux.config_json();
        let mut mux2 = I2cMuxBlock::default();
        mux2.apply_config(&json);
        assert_eq!(mux.address, mux2.address);
        assert_eq!(mux.num_channels, mux2.num_channels);
        assert_eq!(mux.select_topic, mux2.select_topic);
        assert_eq!(mux.bus_topic, mux2.bus_topic);
    }

    #[test]
    fn test_apply_config_changes_address() {
        let mut mux = I2cMuxBlock::default();
        mux.apply_config(&serde_json::json!({"address": 0x73}));
        assert_eq!(mux.address, 0x73);
    }

    #[test]
    fn test_apply_config_changes_num_channels() {
        let mut mux = I2cMuxBlock::default();
        mux.apply_config(&serde_json::json!({"num_channels": 4}));
        assert_eq!(mux.num_channels, 4);
    }

    #[test]
    fn test_apply_config_rejects_invalid_num_channels() {
        let mut mux = I2cMuxBlock::default();
        mux.apply_config(&serde_json::json!({"num_channels": 3}));
        assert_eq!(
            mux.num_channels, 8,
            "invalid channel count should be rejected"
        );
    }

    #[test]
    fn test_apply_config_changes_topics() {
        let mut mux = I2cMuxBlock::default();
        mux.apply_config(&serde_json::json!({
            "select_topic": "mux0/sel",
            "bus_topic": "mux0/bus"
        }));
        assert_eq!(mux.select_topic, "mux0/sel");
        assert_eq!(mux.bus_topic, "mux0/bus");
    }

    // ── Declared channels ───────────────────────────────────────────────

    #[test]
    fn test_declared_channels_default_has_10_channels() {
        // 8 downstream + 1 upstream bus + 1 select = 10
        let mux = I2cMuxBlock::default();
        let channels = mux.declared_channels();
        assert_eq!(channels.len(), 10);
    }

    #[test]
    fn test_declared_channels_upstream_is_hardware_input() {
        let mux = I2cMuxBlock::default();
        let channels = mux.declared_channels();
        assert_eq!(channels[0].name, "i2c_mux/bus");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::Hardware);
    }

    #[test]
    fn test_declared_channels_select_is_pubsub_input() {
        let mux = I2cMuxBlock::default();
        let channels = mux.declared_channels();
        assert_eq!(channels[1].name, "i2c_mux/select");
        assert_eq!(channels[1].direction, ChannelDirection::Input);
        assert_eq!(channels[1].kind, ChannelKind::PubSub);
    }

    #[test]
    fn test_declared_channels_downstream_are_hardware_outputs() {
        let mux = I2cMuxBlock::default();
        let channels = mux.declared_channels();
        for i in 0..8 {
            let ch = &channels[i + 2]; // skip upstream + select
            assert_eq!(ch.name, format!("i2c_mux/bus/ch{}", i));
            assert_eq!(ch.direction, ChannelDirection::Output);
            assert_eq!(ch.kind, ChannelKind::Hardware);
        }
    }

    #[test]
    fn test_declared_channels_respects_num_channels() {
        let mux = I2cMuxBlock {
            num_channels: 2,
            ..I2cMuxBlock::default()
        };
        let channels = mux.declared_channels();
        // 2 downstream + 1 upstream + 1 select = 4
        assert_eq!(channels.len(), 4);
    }

    #[test]
    fn test_declared_channels_respects_bus_topic() {
        let mux = I2cMuxBlock {
            bus_topic: "my_bus".into(),
            num_channels: 2,
            ..I2cMuxBlock::default()
        };
        let channels = mux.declared_channels();
        assert_eq!(channels[0].name, "my_bus");
        assert_eq!(channels[2].name, "my_bus/ch0");
        assert_eq!(channels[3].name, "my_bus/ch1");
    }

    // ── Lower to DAG ────────────────────────────────────────────────────

    #[test]
    fn test_lower_produces_valid_dag() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        assert!(!result.dag.is_empty(), "DAG should have nodes");
    }

    #[test]
    fn test_lower_has_subscribe_for_select_topic() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        let has_sub = result
            .dag
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Subscribe(t) if t == "i2c_mux/select"));
        assert!(has_sub, "DAG should subscribe to select topic");
    }

    #[test]
    fn test_lower_has_input_for_bus() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        let has_input = result
            .dag
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Input(name) if name == "i2c_mux/bus"));
        assert!(has_input, "DAG should have Input for upstream bus");
    }

    #[test]
    fn test_lower_publishes_per_channel() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        for i in 0..8 {
            let topic = format!("i2c_mux/bus/ch{}", i);
            let has_pub = result
                .dag
                .nodes()
                .iter()
                .any(|op| matches!(op, Op::Publish(t, _) if *t == topic));
            assert!(has_pub, "DAG should publish to {}", topic);
        }
    }

    #[test]
    fn test_lower_2_channels_has_fewer_nodes() {
        let mux2 = I2cMuxBlock {
            num_channels: 2,
            ..I2cMuxBlock::default()
        };
        let result2 = mux2.lower().expect("lower should succeed");

        let mux8 = I2cMuxBlock::default();
        let result8 = mux8.lower().expect("lower should succeed");

        assert!(
            result2.dag.len() < result8.dag.len(),
            "2-channel DAG ({}) should be smaller than 8-channel ({})",
            result2.dag.len(),
            result8.dag.len()
        );
    }

    #[test]
    fn test_lower_ports_has_bus_and_select_inputs() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        let input_names: Vec<&str> = result
            .ports
            .inputs
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert!(
            input_names.contains(&"bus"),
            "ports should include bus input"
        );
        assert!(
            input_names.contains(&"select"),
            "ports should include select input"
        );
    }

    #[test]
    fn test_lower_ports_has_channel_outputs() {
        let mux = I2cMuxBlock::default();
        let result = mux.lower().expect("lower should succeed");
        assert_eq!(result.ports.outputs.len(), 8);
    }

    // ── Integration: lower_and_encode ───────────────────────────────────

    #[test]
    fn test_lower_and_encode_produces_valid_cbor() {
        let mux = I2cMuxBlock::default();
        let bytes = crate::lower::lower_and_encode(&mux).expect("encode should succeed");
        let decoded = dag_core::cbor::decode_dag(&bytes).expect("CBOR should be valid");
        assert!(!decoded.is_empty());
    }

    // ── Integration: lower_to_il_text ──────────────────────────────────

    #[test]
    fn test_il_text_contains_block_type() {
        let mux = I2cMuxBlock::default();
        let text = crate::lower::lower_to_il_text(&mux).expect("il text should succeed");
        assert!(text.contains("block @i2c_mux"));
    }

    #[test]
    fn test_il_text_shows_hardware_channels() {
        let mux = I2cMuxBlock::default();
        let text = crate::lower::lower_to_il_text(&mux).expect("il text should succeed");
        assert!(text.contains("hw"), "should show hardware channel kind");
        assert!(text.contains("i2c_mux/bus"), "should show bus channel name");
    }
}
