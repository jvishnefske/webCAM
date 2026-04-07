//! Pub/Sub source and sink blocks for cross-graph or external messaging.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default)]
pub struct PubSubConfig {
    pub topic: String,
    #[ts(type = "\"Float\" | \"Bytes\" | \"Text\" | \"Series\" | \"Any\"")]
    pub port_kind: PortKind,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            topic: "default".into(),
            port_kind: PortKind::Float,
        }
    }
}

// ---------------------------------------------------------------------------
// PubSubSinkBlock
// ---------------------------------------------------------------------------

pub struct PubSubSinkBlock {
    topic: String,
    port_kind: PortKind,
    last_value: Option<Value>,
    display_name: String,
}

impl PubSubSinkBlock {
    pub fn new(topic: String, port_kind: PortKind) -> Self {
        let display_name = format!("PubSub Sink ({})", topic);
        Self {
            topic,
            port_kind,
            last_value: None,
            display_name,
        }
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn last_value(&self) -> Option<&Value> {
        self.last_value.as_ref()
    }

    pub fn from_config(cfg: PubSubConfig) -> Self {
        Self::new(cfg.topic, cfg.port_kind)
    }
}

impl Module for PubSubSinkBlock {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn block_type(&self) -> &str {
        "pubsub_sink"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("in", self.port_kind.clone())]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&PubSubConfig {
            topic: self.topic.clone(),
            port_kind: self.port_kind.clone(),
        })
        .unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for PubSubSinkBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        self.last_value = inputs.first().and_then(|v| v.cloned());
        vec![]
    }
}

// ---------------------------------------------------------------------------
// PubSubSourceBlock
// ---------------------------------------------------------------------------

pub struct PubSubSourceBlock {
    topic: String,
    port_kind: PortKind,
    current_value: Option<Value>,
    display_name: String,
}

impl PubSubSourceBlock {
    pub fn new(topic: String, port_kind: PortKind) -> Self {
        let display_name = format!("PubSub Source ({})", topic);
        Self {
            topic,
            port_kind,
            current_value: None,
            display_name,
        }
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn set_value(&mut self, value: Value) {
        self.current_value = Some(value);
    }

    pub fn clear(&mut self) {
        self.current_value = None;
    }

    pub fn from_config(cfg: PubSubConfig) -> Self {
        Self::new(cfg.topic, cfg.port_kind)
    }
}

impl Module for PubSubSourceBlock {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn block_type(&self) -> &str {
        "pubsub_source"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("out", self.port_kind.clone())]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&PubSubConfig {
            topic: self.topic.clone(),
            port_kind: self.port_kind.clone(),
        })
        .unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for PubSubSourceBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![self.current_value.clone()]
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // PubSubSinkBlock tests
    // -----------------------------------------------------------------------

    #[test]
    fn sink_block_type() {
        let block = PubSubSinkBlock::new("temperature".into(), PortKind::Float);
        assert_eq!(block.block_type(), "pubsub_sink");
    }

    #[test]
    fn sink_name() {
        let block = PubSubSinkBlock::new("temperature".into(), PortKind::Float);
        let name = block.name();
        assert!(
            name.contains("temperature"),
            "name should contain the topic, got: {name}"
        );
    }

    #[test]
    fn sink_input_ports() {
        let block = PubSubSinkBlock::new("sensor".into(), PortKind::Float);
        let ports = block.input_ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "in");
        assert_eq!(ports[0].kind, PortKind::Float);
    }

    #[test]
    fn sink_output_ports() {
        let block = PubSubSinkBlock::new("sensor".into(), PortKind::Float);
        let ports = block.output_ports();
        assert!(ports.is_empty());
    }

    #[test]
    fn sink_config_json() {
        let block = PubSubSinkBlock::new("sensor".into(), PortKind::Float);
        let json = block.config_json();
        let cfg: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(cfg["topic"], "sensor");
        assert_eq!(cfg["port_kind"], "Float");
    }

    #[test]
    fn sink_tick_stores_value() {
        let mut block = PubSubSinkBlock::new("temp".into(), PortKind::Float);
        let val = Value::Float(42.0);
        block.tick(&[Some(&val)], 0.01);
        assert_eq!(block.last_value(), Some(&Value::Float(42.0)));
    }

    #[test]
    fn sink_tick_no_input() {
        let mut block = PubSubSinkBlock::new("temp".into(), PortKind::Float);
        block.tick(&[None], 0.01);
        assert_eq!(block.last_value(), None);
    }

    #[test]
    fn sink_tick_updates_value() {
        let mut block = PubSubSinkBlock::new("temp".into(), PortKind::Float);
        let v1 = Value::Float(1.0);
        let v2 = Value::Float(2.0);
        block.tick(&[Some(&v1)], 0.01);
        assert_eq!(block.last_value(), Some(&Value::Float(1.0)));
        block.tick(&[Some(&v2)], 0.01);
        assert_eq!(block.last_value(), Some(&Value::Float(2.0)));
    }

    #[test]
    fn sink_returns_empty_outputs() {
        let mut block = PubSubSinkBlock::new("temp".into(), PortKind::Float);
        let val = Value::Float(1.0);
        let outputs = block.tick(&[Some(&val)], 0.01);
        assert!(outputs.is_empty());
    }

    // -----------------------------------------------------------------------
    // PubSubSourceBlock tests
    // -----------------------------------------------------------------------

    #[test]
    fn source_block_type() {
        let block = PubSubSourceBlock::new("temperature".into(), PortKind::Float);
        assert_eq!(block.block_type(), "pubsub_source");
    }

    #[test]
    fn source_name() {
        let block = PubSubSourceBlock::new("temperature".into(), PortKind::Float);
        let name = block.name();
        assert!(
            name.contains("temperature"),
            "name should contain the topic, got: {name}"
        );
    }

    #[test]
    fn source_input_ports() {
        let block = PubSubSourceBlock::new("sensor".into(), PortKind::Float);
        let ports = block.input_ports();
        assert!(ports.is_empty());
    }

    #[test]
    fn source_output_ports() {
        let block = PubSubSourceBlock::new("sensor".into(), PortKind::Float);
        let ports = block.output_ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "out");
        assert_eq!(ports[0].kind, PortKind::Float);
    }

    #[test]
    fn source_config_json() {
        let block = PubSubSourceBlock::new("sensor".into(), PortKind::Float);
        let json = block.config_json();
        let cfg: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(cfg["topic"], "sensor");
        assert_eq!(cfg["port_kind"], "Float");
    }

    #[test]
    fn source_tick_no_value() {
        let mut block = PubSubSourceBlock::new("temp".into(), PortKind::Float);
        let outputs = block.tick(&[], 0.01);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], None);
    }

    #[test]
    fn source_tick_with_value() {
        let mut block = PubSubSourceBlock::new("temp".into(), PortKind::Float);
        block.set_value(Value::Float(99.0));
        let outputs = block.tick(&[], 0.01);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], Some(Value::Float(99.0)));
    }

    #[test]
    fn source_tick_persists_value() {
        let mut block = PubSubSourceBlock::new("temp".into(), PortKind::Float);
        block.set_value(Value::Float(5.0));
        let o1 = block.tick(&[], 0.01);
        let o2 = block.tick(&[], 0.01);
        assert_eq!(o1[0], Some(Value::Float(5.0)));
        assert_eq!(o2[0], Some(Value::Float(5.0)));
    }

    #[test]
    fn source_clear() {
        let mut block = PubSubSourceBlock::new("temp".into(), PortKind::Float);
        block.set_value(Value::Float(5.0));
        block.clear();
        let outputs = block.tick(&[], 0.01);
        assert_eq!(outputs[0], None);
    }

    #[test]
    fn source_set_value_replaces() {
        let mut block = PubSubSourceBlock::new("temp".into(), PortKind::Float);
        block.set_value(Value::Float(1.0));
        block.set_value(Value::Float(2.0));
        let outputs = block.tick(&[], 0.01);
        assert_eq!(outputs[0], Some(Value::Float(2.0)));
    }

    // -----------------------------------------------------------------------
    // Integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn sink_and_source_same_topic() {
        let sink = PubSubSinkBlock::new("shared_topic".into(), PortKind::Float);
        let source = PubSubSourceBlock::new("shared_topic".into(), PortKind::Float);
        assert_eq!(sink.topic(), source.topic());
        assert_eq!(sink.topic(), "shared_topic");
    }

    #[test]
    fn sink_various_port_kinds() {
        for kind in &[
            PortKind::Float,
            PortKind::Bytes,
            PortKind::Text,
            PortKind::Series,
        ] {
            let block = PubSubSinkBlock::new("t".into(), kind.clone());
            let ports = block.input_ports();
            assert_eq!(ports[0].kind, *kind);
        }
    }

    #[test]
    fn sink_from_config() {
        let cfg = PubSubConfig {
            topic: "t".into(),
            port_kind: PortKind::Float,
        };
        let block = PubSubSinkBlock::from_config(cfg);
        assert_eq!(block.topic(), "t");
        assert_eq!(block.block_type(), "pubsub_sink");
    }

    #[test]
    fn sink_module_trait_defaults() {
        let mut block = PubSubSinkBlock::new("x".into(), PortKind::Float);
        assert!(block.as_tick().is_some());
        assert!(block.as_analysis().is_none());
        assert!(block.as_codegen().is_none());
        assert!(block.as_sim_model().is_none());
    }

    #[test]
    fn source_from_config() {
        let cfg = PubSubConfig {
            topic: "s".into(),
            port_kind: PortKind::Text,
        };
        let block = PubSubSourceBlock::from_config(cfg);
        assert_eq!(block.topic(), "s");
        assert_eq!(block.block_type(), "pubsub_source");
    }

    #[test]
    fn source_module_trait_defaults() {
        let mut block = PubSubSourceBlock::new("x".into(), PortKind::Float);
        assert!(block.as_tick().is_some());
        assert!(block.as_analysis().is_none());
        assert!(block.as_codegen().is_none());
        assert!(block.as_sim_model().is_none());
    }

    #[test]
    fn source_various_port_kinds() {
        for kind in &[
            PortKind::Float,
            PortKind::Bytes,
            PortKind::Text,
            PortKind::Series,
        ] {
            let block = PubSubSourceBlock::new("t".into(), kind.clone());
            let ports = block.output_ports();
            assert_eq!(ports[0].kind, *kind);
        }
    }
}
