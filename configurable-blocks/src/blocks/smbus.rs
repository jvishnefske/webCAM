//! SMBus read word configurable block.

use dag_core::op::{Dag, DagError};
use dag_core::templates::BlockPorts;
use serde::{Deserialize, Serialize};

use crate::lower::{ConfigurableBlock, LowerResult};
use crate::schema::{
    BlockCategory, ChannelDirection, ChannelKind, ConfigField, DeclaredChannel, FieldKind,
};

/// SMBus read word block.
///
/// Reads a 16-bit word from an I2C device using SMBus protocol
/// (write command byte, read 2 bytes). Publishes the result as an
/// integer (f64) to a topic.
///
/// When `periodic` is true, the block re-executes on a tick interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmBusReadBlock {
    pub bus: u8,
    pub addr: u8,
    pub cmd: u8,
    pub topic: String,
    pub periodic: bool,
    pub interval_ms: u32,
}

impl Default for SmBusReadBlock {
    fn default() -> Self {
        Self {
            bus: 0,
            addr: 0x48,
            cmd: 0x00,
            topic: "smbus/result".into(),
            periodic: false,
            interval_ms: 1000,
        }
    }
}

impl ConfigurableBlock for SmBusReadBlock {
    fn block_type(&self) -> &str {
        "smbus_read"
    }

    fn display_name(&self) -> &str {
        "SMBus Read Word"
    }

    fn category(&self) -> BlockCategory {
        BlockCategory::Io
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "bus".into(),
                label: "I2C Bus".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.bus),
            },
            ConfigField {
                key: "addr".into(),
                label: "Device Address".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.addr),
            },
            ConfigField {
                key: "cmd".into(),
                label: "Command Byte".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.cmd),
            },
            ConfigField {
                key: "topic".into(),
                label: "Publish Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!(self.topic),
            },
            ConfigField {
                key: "periodic".into(),
                label: "Periodic".into(),
                kind: FieldKind::Bool,
                default: serde_json::json!(self.periodic),
            },
            ConfigField {
                key: "interval_ms".into(),
                label: "Interval (ms)".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.interval_ms),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("bus").and_then(|v| v.as_u64()) {
            self.bus = v as u8;
        }
        if let Some(v) = config.get("addr").and_then(|v| v.as_u64()) {
            self.addr = v as u8;
        }
        if let Some(v) = config.get("cmd").and_then(|v| v.as_u64()) {
            self.cmd = v as u8;
        }
        if let Some(s) = config.get("topic").and_then(|v| v.as_str()) {
            self.topic = s.into();
        }
        if let Some(v) = config.get("periodic").and_then(|v| v.as_bool()) {
            self.periodic = v;
        }
        if let Some(v) = config.get("interval_ms").and_then(|v| v.as_u64()) {
            self.interval_ms = v as u32;
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![DeclaredChannel {
            name: self.topic.clone(),
            direction: ChannelDirection::Output,
            kind: ChannelKind::PubSub,
        }]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let channel_name = format!("smbus/{}_{:#04x}_{:#04x}", self.bus, self.addr, self.cmd);
        let read_val = dag.input(&channel_name)?;
        dag.publish(&self.topic, read_val)?;

        Ok(LowerResult {
            dag,
            ports: BlockPorts {
                inputs: vec![],
                outputs: vec![("value".into(), read_val)],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_to_il_text;
    use dag_core::op::Op;

    #[test]
    fn default_config() {
        let block = SmBusReadBlock::default();
        assert_eq!(block.block_type(), "smbus_read");
        assert_eq!(block.category(), BlockCategory::Io);
        assert_eq!(block.bus, 0);
        assert_eq!(block.addr, 0x48);
    }

    #[test]
    fn apply_config_updates_fields() {
        let mut block = SmBusReadBlock::default();
        block.apply_config(&serde_json::json!({
            "bus": 2,
            "addr": 0x50,
            "cmd": 0x05,
            "topic": "temp/sensor1",
            "periodic": true,
            "interval_ms": 500
        }));
        assert_eq!(block.bus, 2);
        assert_eq!(block.addr, 0x50);
        assert_eq!(block.cmd, 0x05);
        assert_eq!(block.topic, "temp/sensor1");
        assert!(block.periodic);
        assert_eq!(block.interval_ms, 500);
    }

    #[test]
    fn declared_channels_has_output_topic() {
        let block = SmBusReadBlock::default();
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "smbus/result");
        assert_eq!(channels[0].direction, ChannelDirection::Output);
    }

    #[test]
    fn lower_produces_input_and_publish() {
        let block = SmBusReadBlock::default();
        let result = block.lower().unwrap();
        let ops = result.dag.nodes();

        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Op::Input(name) if name.starts_with("smbus/")));
        assert!(matches!(&ops[1], Op::Publish(topic, 0) if topic == "smbus/result"));
    }

    #[test]
    fn lower_to_il_text_contains_block_name() {
        let block = SmBusReadBlock::default();
        let text = lower_to_il_text(&block).unwrap();
        assert!(text.contains("block @smbus_read"));
        assert!(text.contains("Publish"));
    }

    #[test]
    fn config_schema_has_six_fields() {
        let block = SmBusReadBlock::default();
        assert_eq!(block.config_schema().len(), 6);
    }
}
