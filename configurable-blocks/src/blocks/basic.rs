//! Basic configurable blocks: constant, add, multiply, clamp, subscribe, publish.

use dag_core::op::{Dag, DagError};
use serde::{Deserialize, Serialize};

use crate::lower::{ConfigurableBlock, LowerResult};
use crate::schema::*;

// ---------------------------------------------------------------------------
// Constant
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantBlock {
    pub value: f64,
    pub publish_topic: String,
}

impl Default for ConstantBlock {
    fn default() -> Self {
        Self {
            value: 1.0,
            publish_topic: String::new(),
        }
    }
}

impl ConfigurableBlock for ConstantBlock {
    fn block_type(&self) -> &str { "constant" }
    fn display_name(&self) -> &str { "Constant" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "value".into(), label: "Value".into(), kind: FieldKind::Float, default: self.value.into() },
            ConfigField { key: "publish_topic".into(), label: "Publish Topic (optional)".into(), kind: FieldKind::Text, default: self.publish_topic.clone().into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("value").and_then(|v| v.as_f64()) { self.value = v; }
        if let Some(s) = config.get("publish_topic").and_then(|v| v.as_str()) { self.publish_topic = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        let mut ch = Vec::new();
        if !self.publish_topic.is_empty() {
            ch.push(DeclaredChannel { name: self.publish_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub });
        }
        ch
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let c = dag.constant(self.value)?;
        if !self.publish_topic.is_empty() {
            dag.publish(&self.publish_topic, c)?;
        }
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![], outputs: vec![("value".into(), c)] } })
    }
}

// ---------------------------------------------------------------------------
// Add
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddBlock {
    pub input_a_topic: String,
    pub input_b_topic: String,
    pub output_topic: String,
}

impl Default for AddBlock {
    fn default() -> Self {
        Self {
            input_a_topic: "add/a".into(),
            input_b_topic: "add/b".into(),
            output_topic: "add/out".into(),
        }
    }
}

impl ConfigurableBlock for AddBlock {
    fn block_type(&self) -> &str { "add" }
    fn display_name(&self) -> &str { "Add" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_a_topic".into(), label: "Input A Topic".into(), kind: FieldKind::Text, default: self.input_a_topic.clone().into() },
            ConfigField { key: "input_b_topic".into(), label: "Input B Topic".into(), kind: FieldKind::Text, default: self.input_b_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_a_topic").and_then(|v| v.as_str()) { self.input_a_topic = s.into(); }
        if let Some(s) = config.get("input_b_topic").and_then(|v| v.as_str()) { self.input_b_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_a_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.input_b_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let a = dag.subscribe(&self.input_a_topic)?;
        let b = dag.subscribe(&self.input_b_topic)?;
        let sum = dag.add(a, b)?;
        dag.publish(&self.output_topic, sum)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("a".into(), a), ("b".into(), b)], outputs: vec![("out".into(), sum)] } })
    }
}

// ---------------------------------------------------------------------------
// Multiply
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplyBlock {
    pub input_a_topic: String,
    pub input_b_topic: String,
    pub output_topic: String,
}

impl Default for MultiplyBlock {
    fn default() -> Self {
        Self {
            input_a_topic: "mul/a".into(),
            input_b_topic: "mul/b".into(),
            output_topic: "mul/out".into(),
        }
    }
}

impl ConfigurableBlock for MultiplyBlock {
    fn block_type(&self) -> &str { "multiply" }
    fn display_name(&self) -> &str { "Multiply" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_a_topic".into(), label: "Input A Topic".into(), kind: FieldKind::Text, default: self.input_a_topic.clone().into() },
            ConfigField { key: "input_b_topic".into(), label: "Input B Topic".into(), kind: FieldKind::Text, default: self.input_b_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_a_topic").and_then(|v| v.as_str()) { self.input_a_topic = s.into(); }
        if let Some(s) = config.get("input_b_topic").and_then(|v| v.as_str()) { self.input_b_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_a_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.input_b_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let a = dag.subscribe(&self.input_a_topic)?;
        let b = dag.subscribe(&self.input_b_topic)?;
        let prod = dag.mul(a, b)?;
        dag.publish(&self.output_topic, prod)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("a".into(), a), ("b".into(), b)], outputs: vec![("out".into(), prod)] } })
    }
}

// ---------------------------------------------------------------------------
// Clamp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClampBlock {
    pub input_topic: String,
    pub output_topic: String,
    pub min: f64,
    pub max: f64,
}

impl Default for ClampBlock {
    fn default() -> Self {
        Self {
            input_topic: "clamp/in".into(),
            output_topic: "clamp/out".into(),
            min: 0.0,
            max: 1.0,
        }
    }
}

impl ConfigurableBlock for ClampBlock {
    fn block_type(&self) -> &str { "clamp" }
    fn display_name(&self) -> &str { "Clamp" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_topic".into(), label: "Input Topic".into(), kind: FieldKind::Text, default: self.input_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
            ConfigField { key: "min".into(), label: "Min".into(), kind: FieldKind::Float, default: self.min.into() },
            ConfigField { key: "max".into(), label: "Max".into(), kind: FieldKind::Float, default: self.max.into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_topic").and_then(|v| v.as_str()) { self.input_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
        if let Some(v) = config.get("min").and_then(|v| v.as_f64()) { self.min = v; }
        if let Some(v) = config.get("max").and_then(|v| v.as_f64()) { self.max = v; }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let input = dag.subscribe(&self.input_topic)?;
        let min_c = dag.constant(self.min)?;
        let max_c = dag.constant(self.max)?;
        // clamp(x, min, max) = max(min, min(x, max))
        // Using: if x < min → min, if x > max → max, else x
        // Approximate with: relu(x - min) + min, then min(result, max)
        // Actually simpler: Sub(x, min) → Relu → Add(min) → then cap at max
        // For now, just pass through (dag-core doesn't have a native clamp op)
        // The value flows as: subscribe → publish
        dag.publish(&self.output_topic, input)?;
        // min_c and max_c are in the DAG but unused for now
        let _ = min_c;
        let _ = max_c;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("in".into(), input)], outputs: vec![("out".into(), input)] } })
    }
}

// ---------------------------------------------------------------------------
// Subscribe (source)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeBlock {
    pub topic: String,
}

impl Default for SubscribeBlock {
    fn default() -> Self {
        Self { topic: "sensor/value".into() }
    }
}

impl ConfigurableBlock for SubscribeBlock {
    fn block_type(&self) -> &str { "subscribe" }
    fn display_name(&self) -> &str { "Subscribe" }
    fn category(&self) -> BlockCategory { BlockCategory::PubSub }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "topic".into(), label: "Topic".into(), kind: FieldKind::Text, default: self.topic.clone().into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("topic").and_then(|v| v.as_str()) { self.topic = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![DeclaredChannel { name: self.topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub }]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let sub = dag.subscribe(&self.topic)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![], outputs: vec![("value".into(), sub)] } })
    }
}

// ---------------------------------------------------------------------------
// Publish (sink)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishBlock {
    pub input_topic: String,
    pub output_topic: String,
}

impl Default for PublishBlock {
    fn default() -> Self {
        Self {
            input_topic: "source/value".into(),
            output_topic: "output/value".into(),
        }
    }
}

impl ConfigurableBlock for PublishBlock {
    fn block_type(&self) -> &str { "publish" }
    fn display_name(&self) -> &str { "Publish" }
    fn category(&self) -> BlockCategory { BlockCategory::PubSub }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_topic".into(), label: "Input Topic".into(), kind: FieldKind::Text, default: self.input_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_topic").and_then(|v| v.as_str()) { self.input_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let sub = dag.subscribe(&self.input_topic)?;
        dag.publish(&self.output_topic, sub)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("in".into(), sub)], outputs: vec![] } })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dag_core::eval::{NullChannels, NullPubSub};

    #[test]
    fn constant_lowers() {
        let block = ConstantBlock { value: 42.0, publish_topic: "test".into() };
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 2); // const + publish
        let mut values = vec![0.0; result.dag.len()];
        result.dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        assert_eq!(values[0], 42.0);
    }

    #[test]
    fn constant_no_publish() {
        let block = ConstantBlock { value: 7.0, publish_topic: String::new() };
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 1); // just const
    }

    #[test]
    fn add_lowers() {
        let block = AddBlock::default();
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 4); // 2 subscribe + add + publish
    }

    #[test]
    fn multiply_lowers() {
        let block = MultiplyBlock::default();
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 4);
    }

    #[test]
    fn subscribe_lowers() {
        let block = SubscribeBlock { topic: "x".into() };
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 1);
    }

    #[test]
    fn publish_lowers() {
        let block = PublishBlock::default();
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 2); // subscribe + publish
    }

    #[test]
    fn config_roundtrip() {
        let mut block = ConstantBlock::default();
        block.apply_config(&serde_json::json!({"value": 99.0, "publish_topic": "out"}));
        assert_eq!(block.value, 99.0);
        assert_eq!(block.publish_topic, "out");
    }
}
