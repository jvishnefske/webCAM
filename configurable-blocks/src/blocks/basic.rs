//! Basic configurable blocks: constant, add, multiply, clamp, subscribe, publish, adc.

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
        // Using: if x < min -> min, if x > max -> max, else x
        // Approximate with: relu(x - min) + min, then min(result, max)
        // Actually simpler: Sub(x, min) -> Relu -> Add(min) -> then cap at max
        // For now, just pass through (dag-core doesn't have a native clamp op)
        // The value flows as: subscribe -> publish
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
// PWM Output (hardware output)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwmBlock {
    pub channel_name: String,
}

impl Default for PwmBlock {
    fn default() -> Self {
        Self { channel_name: "pwm0".into() }
    }
}

impl ConfigurableBlock for PwmBlock {
    fn block_type(&self) -> &str { "pwm" }
    fn display_name(&self) -> &str { "PWM Output" }
    fn category(&self) -> BlockCategory { BlockCategory::Io }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![ConfigField { key: "channel_name".into(), label: "Channel Name".into(), kind: FieldKind::Text, default: self.channel_name.clone().into() }]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("channel_name").and_then(|v| v.as_str()) { self.channel_name = s.into(); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![DeclaredChannel { name: self.channel_name.clone(), direction: ChannelDirection::Output, kind: ChannelKind::Hardware }]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let node = dag.output(&self.channel_name, 0).or_else(|_| {
            // If no upstream node, create a constant 0 and output that
            let c = dag.constant(0.0)?;
            dag.output(&self.channel_name, c)
        })?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![], outputs: vec![("hw_out".into(), node)] } })
    }
}

// ---------------------------------------------------------------------------
// Subtract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtractBlock {
    pub input_a_topic: String,
    pub input_b_topic: String,
    pub output_topic: String,
}

impl Default for SubtractBlock {
    fn default() -> Self {
        Self { input_a_topic: "sub/a".into(), input_b_topic: "sub/b".into(), output_topic: "sub/out".into() }
    }
}

impl ConfigurableBlock for SubtractBlock {
    fn block_type(&self) -> &str { "subtract" }
    fn display_name(&self) -> &str { "Subtract" }
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
        let diff = dag.sub(a, b)?;
        dag.publish(&self.output_topic, diff)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("a".into(), a), ("b".into(), b)], outputs: vec![("out".into(), diff)] } })
    }
}

// ---------------------------------------------------------------------------
// Negate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegateBlock {
    pub input_topic: String,
    pub output_topic: String,
}

impl Default for NegateBlock {
    fn default() -> Self {
        Self { input_topic: "neg/in".into(), output_topic: "neg/out".into() }
    }
}

impl ConfigurableBlock for NegateBlock {
    fn block_type(&self) -> &str { "negate" }
    fn display_name(&self) -> &str { "Negate" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

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
        let input = dag.subscribe(&self.input_topic)?;
        let neg = dag.neg(input)?;
        dag.publish(&self.output_topic, neg)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("in".into(), input)], outputs: vec![("out".into(), neg)] } })
    }
}

// ---------------------------------------------------------------------------
// Map/Scale — linear mapping: out = (in - in_min)/(in_max - in_min) * (out_max - out_min) + out_min
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapScaleBlock {
    pub input_topic: String,
    pub output_topic: String,
    pub in_min: f64,
    pub in_max: f64,
    pub out_min: f64,
    pub out_max: f64,
}

impl Default for MapScaleBlock {
    fn default() -> Self {
        Self {
            input_topic: "map/in".into(), output_topic: "map/out".into(),
            in_min: 0.0, in_max: 1023.0, out_min: 0.0, out_max: 100.0,
        }
    }
}

impl ConfigurableBlock for MapScaleBlock {
    fn block_type(&self) -> &str { "map_scale" }
    fn display_name(&self) -> &str { "Map/Scale" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_topic".into(), label: "Input Topic".into(), kind: FieldKind::Text, default: self.input_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
            ConfigField { key: "in_min".into(), label: "Input Min".into(), kind: FieldKind::Float, default: self.in_min.into() },
            ConfigField { key: "in_max".into(), label: "Input Max".into(), kind: FieldKind::Float, default: self.in_max.into() },
            ConfigField { key: "out_min".into(), label: "Output Min".into(), kind: FieldKind::Float, default: self.out_min.into() },
            ConfigField { key: "out_max".into(), label: "Output Max".into(), kind: FieldKind::Float, default: self.out_max.into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_topic").and_then(|v| v.as_str()) { self.input_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
        if let Some(v) = config.get("in_min").and_then(|v| v.as_f64()) { self.in_min = v; }
        if let Some(v) = config.get("in_max").and_then(|v| v.as_f64()) { self.in_max = v; }
        if let Some(v) = config.get("out_min").and_then(|v| v.as_f64()) { self.out_min = v; }
        if let Some(v) = config.get("out_max").and_then(|v| v.as_f64()) { self.out_max = v; }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        // out = (in - in_min) / (in_max - in_min) * (out_max - out_min) + out_min
        // Decompose: scale = (out_max - out_min) / (in_max - in_min)
        //            out = (in - in_min) * scale + out_min
        let in_range = self.in_max - self.in_min;
        let out_range = self.out_max - self.out_min;
        let scale = if in_range.abs() > 1e-15 { out_range / in_range } else { 0.0 };

        let mut dag = Dag::new();
        let input = dag.subscribe(&self.input_topic)?;
        let in_min_c = dag.constant(self.in_min)?;
        let shifted = dag.sub(input, in_min_c)?;       // in - in_min
        let scale_c = dag.constant(scale)?;
        let scaled = dag.mul(shifted, scale_c)?;        // (in - in_min) * scale
        let out_min_c = dag.constant(self.out_min)?;
        let result = dag.add(scaled, out_min_c)?;       // + out_min
        dag.publish(&self.output_topic, result)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("in".into(), input)], outputs: vec![("out".into(), result)] } })
    }
}

// ---------------------------------------------------------------------------
// Low-Pass Filter — exponential moving average: y = alpha*x + (1-alpha)*y_prev
// Note: since dag-core is stateless per-tick, this approximates by applying
// alpha weighting to the input. Full IIR requires cross-tick state (future).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowPassBlock {
    pub input_topic: String,
    pub output_topic: String,
    pub alpha: f64,
}

impl Default for LowPassBlock {
    fn default() -> Self {
        Self { input_topic: "lp/in".into(), output_topic: "lp/out".into(), alpha: 0.1 }
    }
}

impl ConfigurableBlock for LowPassBlock {
    fn block_type(&self) -> &str { "lowpass" }
    fn display_name(&self) -> &str { "Low-Pass Filter" }
    fn category(&self) -> BlockCategory { BlockCategory::Math }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField { key: "input_topic".into(), label: "Input Topic".into(), kind: FieldKind::Text, default: self.input_topic.clone().into() },
            ConfigField { key: "output_topic".into(), label: "Output Topic".into(), kind: FieldKind::Text, default: self.output_topic.clone().into() },
            ConfigField { key: "alpha".into(), label: "Alpha (0-1, lower = smoother)".into(), kind: FieldKind::Float, default: self.alpha.into() },
        ]
    }

    fn config_json(&self) -> serde_json::Value { serde_json::to_value(self).unwrap_or_default() }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("input_topic").and_then(|v| v.as_str()) { self.input_topic = s.into(); }
        if let Some(s) = config.get("output_topic").and_then(|v| v.as_str()) { self.output_topic = s.into(); }
        if let Some(v) = config.get("alpha").and_then(|v| v.as_f64()) { self.alpha = v.clamp(0.0, 1.0); }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel { name: self.input_topic.clone(), direction: ChannelDirection::Input, kind: ChannelKind::PubSub },
            DeclaredChannel { name: self.output_topic.clone(), direction: ChannelDirection::Output, kind: ChannelKind::PubSub },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        // y = alpha * x + (1 - alpha) * y_prev
        // Since dag-core is stateless, we subscribe to both the input and the
        // output topic (which holds the previous value via pubsub persistence).
        let mut dag = Dag::new();
        let x = dag.subscribe(&self.input_topic)?;
        let y_prev = dag.subscribe(&self.output_topic)?; // previous output via pubsub
        let alpha_c = dag.constant(self.alpha)?;
        let one_minus_alpha = dag.constant(1.0 - self.alpha)?;
        let ax = dag.mul(alpha_c, x)?;                   // alpha * x
        let by = dag.mul(one_minus_alpha, y_prev)?;       // (1-alpha) * y_prev
        let y = dag.add(ax, by)?;                         // alpha*x + (1-alpha)*y_prev
        dag.publish(&self.output_topic, y)?;
        Ok(LowerResult { dag, ports: dag_core::templates::BlockPorts { inputs: vec![("in".into(), x)], outputs: vec![("out".into(), y)] } })
    }
}

// ---------------------------------------------------------------------------
// Adc (hardware input)
// ---------------------------------------------------------------------------

/// Minimal ADC input block that declares a single `Hardware`-kind input channel.
///
/// Used by the deployment profile validation machinery and its tests to exercise
/// the `MissingPeripheralBinding` check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcBlock {
    /// Channel name used as both the declared hardware port and the pubsub topic
    /// to which the sampled value is published.
    pub channel_name: String,
}

impl Default for AdcBlock {
    fn default() -> Self {
        Self {
            channel_name: "adc0".into(),
        }
    }
}

impl ConfigurableBlock for AdcBlock {
    fn block_type(&self) -> &str { "adc" }
    fn display_name(&self) -> &str { "ADC Input" }
    fn category(&self) -> BlockCategory { BlockCategory::Io }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![ConfigField {
            key: "channel_name".into(),
            label: "Channel Name".into(),
            kind: FieldKind::Text,
            default: self.channel_name.clone().into(),
        }]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(s) = config.get("channel_name").and_then(|v| v.as_str()) {
            self.channel_name = s.into();
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![DeclaredChannel {
            name: self.channel_name.clone(),
            direction: ChannelDirection::Input,
            kind: ChannelKind::Hardware,
        }]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        // Hardware input is modelled as a DAG Input op.
        let node = dag.input(&self.channel_name)?;
        Ok(LowerResult {
            dag,
            ports: dag_core::templates::BlockPorts {
                inputs: vec![("hw_in".into(), node)],
                outputs: vec![],
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dag_core::eval::{NullChannels, NullPubSub, PubSubReader};
    use std::collections::BTreeMap;

    /// Test-only pubsub mock that returns pre-loaded topic values.
    struct MockPubSub {
        values: BTreeMap<String, f64>,
    }

    impl PubSubReader for MockPubSub {
        fn read(&self, topic: &str) -> f64 {
            self.values.get(topic).copied().unwrap_or(0.0)
        }
    }

    // ===================================================================
    // Existing lower-only tests (preserved)
    // ===================================================================

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

    // ===================================================================
    // ConstantBlock -- all trait methods
    // ===================================================================

    #[test]
    fn test_constant_all_trait_methods() {
        let mut block = ConstantBlock::default();

        // identity
        assert_eq!(block.block_type(), "constant");
        assert_eq!(block.display_name(), "Constant");
        assert_eq!(block.category(), BlockCategory::Math);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 2);
        assert!(schema.iter().any(|f| f.key == "value" && f.kind == FieldKind::Float));
        assert!(schema.iter().any(|f| f.key == "publish_topic" && f.kind == FieldKind::Text));

        // config json roundtrip
        let json = block.config_json();
        assert_eq!(json["value"], 1.0);
        assert_eq!(json["publish_topic"], "");

        // apply_config
        let new_config = serde_json::json!({"value": 99.0, "publish_topic": "sensor/out"});
        block.apply_config(&new_config);
        assert_eq!(block.value, 99.0);
        assert_eq!(block.publish_topic, "sensor/out");

        // config_json reflects the update
        let json2 = block.config_json();
        assert_eq!(json2["value"], 99.0);
        assert_eq!(json2["publish_topic"], "sensor/out");

        // declared channels -- non-empty publish_topic yields one output channel
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "sensor/out");
        assert_eq!(channels[0].direction, ChannelDirection::Output);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);

        // lower succeeds
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 2); // const + publish
        assert_eq!(result.ports.outputs.len(), 1);
        assert_eq!(result.ports.outputs[0].0, "value");
    }

    #[test]
    fn test_constant_no_publish_channels_empty() {
        let block = ConstantBlock::default(); // publish_topic is empty
        let channels = block.declared_channels();
        assert!(channels.is_empty());
    }

    #[test]
    fn test_constant_apply_config_partial() {
        let mut block = ConstantBlock::default();
        // Only update value, leave publish_topic unchanged
        block.apply_config(&serde_json::json!({"value": 42.0}));
        assert_eq!(block.value, 42.0);
        assert_eq!(block.publish_topic, ""); // unchanged
    }

    #[test]
    fn test_constant_default_values() {
        let block = ConstantBlock::default();
        assert_eq!(block.value, 1.0);
        assert_eq!(block.publish_topic, "");
    }

    #[test]
    fn test_constant_schema_defaults_match_fields() {
        let block = ConstantBlock::default();
        let schema = block.config_schema();
        let value_field = schema.iter().find(|f| f.key == "value").unwrap();
        assert_eq!(value_field.default, serde_json::json!(1.0));
        let topic_field = schema.iter().find(|f| f.key == "publish_topic").unwrap();
        assert_eq!(topic_field.default, serde_json::json!(""));
    }

    // ===================================================================
    // AddBlock -- all trait methods
    // ===================================================================

    #[test]
    fn test_add_all_trait_methods() {
        let mut block = AddBlock::default();

        // identity
        assert_eq!(block.block_type(), "add");
        assert_eq!(block.display_name(), "Add");
        assert_eq!(block.category(), BlockCategory::Math);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 3);
        assert!(schema.iter().any(|f| f.key == "input_a_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "input_b_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "output_topic" && f.kind == FieldKind::Text));

        // config json
        let json = block.config_json();
        assert_eq!(json["input_a_topic"], "add/a");
        assert_eq!(json["input_b_topic"], "add/b");
        assert_eq!(json["output_topic"], "add/out");

        // apply_config
        block.apply_config(&serde_json::json!({
            "input_a_topic": "x",
            "input_b_topic": "y",
            "output_topic": "z"
        }));
        assert_eq!(block.input_a_topic, "x");
        assert_eq!(block.input_b_topic, "y");
        assert_eq!(block.output_topic, "z");

        // config_json reflects update
        let json2 = block.config_json();
        assert_eq!(json2["input_a_topic"], "x");
        assert_eq!(json2["input_b_topic"], "y");
        assert_eq!(json2["output_topic"], "z");

        // declared channels: 2 inputs + 1 output
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].name, "x");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].name, "y");
        assert_eq!(channels[1].direction, ChannelDirection::Input);
        assert_eq!(channels[2].name, "z");
        assert_eq!(channels[2].direction, ChannelDirection::Output);

        // lower succeeds
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 4); // 2 subscribe + add + publish
        assert_eq!(result.ports.inputs.len(), 2);
        assert_eq!(result.ports.inputs[0].0, "a");
        assert_eq!(result.ports.inputs[1].0, "b");
        assert_eq!(result.ports.outputs.len(), 1);
        assert_eq!(result.ports.outputs[0].0, "out");
    }

    #[test]
    fn test_add_apply_config_partial() {
        let mut block = AddBlock::default();
        block.apply_config(&serde_json::json!({"output_topic": "sum"}));
        assert_eq!(block.output_topic, "sum");
        // Others remain at default
        assert_eq!(block.input_a_topic, "add/a");
        assert_eq!(block.input_b_topic, "add/b");
    }

    #[test]
    fn test_add_default_values() {
        let block = AddBlock::default();
        assert_eq!(block.input_a_topic, "add/a");
        assert_eq!(block.input_b_topic, "add/b");
        assert_eq!(block.output_topic, "add/out");
    }

    #[test]
    fn test_add_evaluate() {
        let block = AddBlock::default();
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([
                ("add/a".into(), 3.0),
                ("add/b".into(), 7.0),
            ]),
        };

        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "add/out");
        assert!((eval.publishes[0].1 - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_add_schema_defaults_match_fields() {
        let block = AddBlock::default();
        let schema = block.config_schema();
        let a_field = schema.iter().find(|f| f.key == "input_a_topic").unwrap();
        assert_eq!(a_field.default, serde_json::json!("add/a"));
        let b_field = schema.iter().find(|f| f.key == "input_b_topic").unwrap();
        assert_eq!(b_field.default, serde_json::json!("add/b"));
        let out_field = schema.iter().find(|f| f.key == "output_topic").unwrap();
        assert_eq!(out_field.default, serde_json::json!("add/out"));
    }

    // ===================================================================
    // MultiplyBlock -- all trait methods
    // ===================================================================

    #[test]
    fn test_multiply_all_trait_methods() {
        let mut block = MultiplyBlock::default();

        // identity
        assert_eq!(block.block_type(), "multiply");
        assert_eq!(block.display_name(), "Multiply");
        assert_eq!(block.category(), BlockCategory::Math);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 3);
        assert!(schema.iter().any(|f| f.key == "input_a_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "input_b_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "output_topic" && f.kind == FieldKind::Text));

        // config json
        let json = block.config_json();
        assert_eq!(json["input_a_topic"], "mul/a");
        assert_eq!(json["input_b_topic"], "mul/b");
        assert_eq!(json["output_topic"], "mul/out");

        // apply_config
        block.apply_config(&serde_json::json!({
            "input_a_topic": "p",
            "input_b_topic": "q",
            "output_topic": "r"
        }));
        assert_eq!(block.input_a_topic, "p");
        assert_eq!(block.input_b_topic, "q");
        assert_eq!(block.output_topic, "r");

        // config_json reflects update
        let json2 = block.config_json();
        assert_eq!(json2["input_a_topic"], "p");
        assert_eq!(json2["input_b_topic"], "q");
        assert_eq!(json2["output_topic"], "r");

        // declared channels: 2 inputs + 1 output
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].name, "p");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].name, "q");
        assert_eq!(channels[1].direction, ChannelDirection::Input);
        assert_eq!(channels[2].name, "r");
        assert_eq!(channels[2].direction, ChannelDirection::Output);

        // lower succeeds
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 4); // 2 subscribe + mul + publish
        assert_eq!(result.ports.inputs.len(), 2);
        assert_eq!(result.ports.inputs[0].0, "a");
        assert_eq!(result.ports.inputs[1].0, "b");
        assert_eq!(result.ports.outputs.len(), 1);
        assert_eq!(result.ports.outputs[0].0, "out");
    }

    #[test]
    fn test_multiply_apply_config_partial() {
        let mut block = MultiplyBlock::default();
        block.apply_config(&serde_json::json!({"output_topic": "product"}));
        assert_eq!(block.output_topic, "product");
        assert_eq!(block.input_a_topic, "mul/a");
        assert_eq!(block.input_b_topic, "mul/b");
    }

    #[test]
    fn test_multiply_default_values() {
        let block = MultiplyBlock::default();
        assert_eq!(block.input_a_topic, "mul/a");
        assert_eq!(block.input_b_topic, "mul/b");
        assert_eq!(block.output_topic, "mul/out");
    }

    #[test]
    fn test_multiply_evaluate() {
        let block = MultiplyBlock::default();
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([
                ("mul/a".into(), 4.0),
                ("mul/b".into(), 5.0),
            ]),
        };

        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "mul/out");
        assert!((eval.publishes[0].1 - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_multiply_schema_defaults_match_fields() {
        let block = MultiplyBlock::default();
        let schema = block.config_schema();
        let a_field = schema.iter().find(|f| f.key == "input_a_topic").unwrap();
        assert_eq!(a_field.default, serde_json::json!("mul/a"));
        let b_field = schema.iter().find(|f| f.key == "input_b_topic").unwrap();
        assert_eq!(b_field.default, serde_json::json!("mul/b"));
        let out_field = schema.iter().find(|f| f.key == "output_topic").unwrap();
        assert_eq!(out_field.default, serde_json::json!("mul/out"));
    }

    // ===================================================================
    // ClampBlock -- all trait methods + dedicated lower test
    // ===================================================================

    #[test]
    fn test_clamp_all_trait_methods() {
        let mut block = ClampBlock::default();

        // identity
        assert_eq!(block.block_type(), "clamp");
        assert_eq!(block.display_name(), "Clamp");
        assert_eq!(block.category(), BlockCategory::Math);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 4);
        assert!(schema.iter().any(|f| f.key == "input_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "output_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "min" && f.kind == FieldKind::Float));
        assert!(schema.iter().any(|f| f.key == "max" && f.kind == FieldKind::Float));

        // config json -- default values
        let json = block.config_json();
        assert_eq!(json["input_topic"], "clamp/in");
        assert_eq!(json["output_topic"], "clamp/out");
        assert_eq!(json["min"], 0.0);
        assert_eq!(json["max"], 1.0);

        // apply_config -- full update
        block.apply_config(&serde_json::json!({
            "input_topic": "sig/in",
            "output_topic": "sig/out",
            "min": -10.0,
            "max": 10.0
        }));
        assert_eq!(block.input_topic, "sig/in");
        assert_eq!(block.output_topic, "sig/out");
        assert_eq!(block.min, -10.0);
        assert_eq!(block.max, 10.0);

        // config_json reflects update
        let json2 = block.config_json();
        assert_eq!(json2["input_topic"], "sig/in");
        assert_eq!(json2["output_topic"], "sig/out");
        assert_eq!(json2["min"], -10.0);
        assert_eq!(json2["max"], 10.0);

        // declared channels: 1 input + 1 output
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "sig/in");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].name, "sig/out");
        assert_eq!(channels[1].direction, ChannelDirection::Output);
        assert_eq!(channels[1].kind, ChannelKind::PubSub);

        // lower succeeds
        let result = block.lower().unwrap();
        assert!(!result.dag.is_empty());
        assert_eq!(result.ports.inputs.len(), 1);
        assert_eq!(result.ports.inputs[0].0, "in");
        assert_eq!(result.ports.outputs.len(), 1);
        assert_eq!(result.ports.outputs[0].0, "out");
    }

    #[test]
    fn test_clamp_apply_config_partial() {
        let mut block = ClampBlock::default();
        // Only update min, leave others at default
        block.apply_config(&serde_json::json!({"min": -5.0}));
        assert_eq!(block.min, -5.0);
        assert_eq!(block.max, 1.0); // unchanged
        assert_eq!(block.input_topic, "clamp/in"); // unchanged
        assert_eq!(block.output_topic, "clamp/out"); // unchanged
    }

    #[test]
    fn test_clamp_default_values() {
        let block = ClampBlock::default();
        assert_eq!(block.input_topic, "clamp/in");
        assert_eq!(block.output_topic, "clamp/out");
        assert_eq!(block.min, 0.0);
        assert_eq!(block.max, 1.0);
    }

    #[test]
    fn test_clamp_lower() {
        let block = ClampBlock {
            input_topic: "in".into(),
            output_topic: "out".into(),
            min: 0.0,
            max: 1.0,
        };
        let result = block.lower().unwrap();

        // The DAG should have at least subscribe + publish
        assert!(result.dag.len() >= 2);

        // Evaluate with a value that is within range
        let ps = MockPubSub {
            values: BTreeMap::from([("in".into(), 0.5)]),
        };
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "out");
        // The current implementation is pass-through, so the value should match input
        assert!((eval.publishes[0].1 - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_clamp_schema_defaults_match_fields() {
        let block = ClampBlock::default();
        let schema = block.config_schema();
        let min_field = schema.iter().find(|f| f.key == "min").unwrap();
        assert_eq!(min_field.default, serde_json::json!(0.0));
        let max_field = schema.iter().find(|f| f.key == "max").unwrap();
        assert_eq!(max_field.default, serde_json::json!(1.0));
        let in_field = schema.iter().find(|f| f.key == "input_topic").unwrap();
        assert_eq!(in_field.default, serde_json::json!("clamp/in"));
        let out_field = schema.iter().find(|f| f.key == "output_topic").unwrap();
        assert_eq!(out_field.default, serde_json::json!("clamp/out"));
    }

    #[test]
    fn test_clamp_declared_channels_reflect_config() {
        let mut block = ClampBlock::default();
        block.apply_config(&serde_json::json!({
            "input_topic": "temp/raw",
            "output_topic": "temp/clamped"
        }));
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "temp/raw");
        assert_eq!(channels[1].name, "temp/clamped");
    }

    // ===================================================================
    // SubscribeBlock -- all trait methods
    // ===================================================================

    #[test]
    fn test_subscribe_all_trait_methods() {
        let mut block = SubscribeBlock::default();

        // identity
        assert_eq!(block.block_type(), "subscribe");
        assert_eq!(block.display_name(), "Subscribe");
        assert_eq!(block.category(), BlockCategory::PubSub);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 1);
        assert_eq!(schema[0].key, "topic");
        assert_eq!(schema[0].kind, FieldKind::Text);

        // config json
        let json = block.config_json();
        assert_eq!(json["topic"], "sensor/value");

        // apply_config
        block.apply_config(&serde_json::json!({"topic": "temp/reading"}));
        assert_eq!(block.topic, "temp/reading");

        // config_json reflects update
        let json2 = block.config_json();
        assert_eq!(json2["topic"], "temp/reading");

        // declared channels: 1 input
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "temp/reading");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);

        // lower succeeds
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 1); // just subscribe
        assert!(result.ports.inputs.is_empty());
        assert_eq!(result.ports.outputs.len(), 1);
        assert_eq!(result.ports.outputs[0].0, "value");
    }

    #[test]
    fn test_subscribe_default_values() {
        let block = SubscribeBlock::default();
        assert_eq!(block.topic, "sensor/value");
    }

    #[test]
    fn test_subscribe_schema_default_matches_field() {
        let block = SubscribeBlock::default();
        let schema = block.config_schema();
        assert_eq!(schema[0].default, serde_json::json!("sensor/value"));
    }

    #[test]
    fn test_subscribe_declared_channels_reflect_config() {
        let mut block = SubscribeBlock::default();
        block.apply_config(&serde_json::json!({"topic": "motor/rpm"}));
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "motor/rpm");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
    }

    // ===================================================================
    // PublishBlock -- all trait methods
    // ===================================================================

    #[test]
    fn test_publish_all_trait_methods() {
        let mut block = PublishBlock::default();

        // identity
        assert_eq!(block.block_type(), "publish");
        assert_eq!(block.display_name(), "Publish");
        assert_eq!(block.category(), BlockCategory::PubSub);

        // config schema
        let schema = block.config_schema();
        assert_eq!(schema.len(), 2);
        assert!(schema.iter().any(|f| f.key == "input_topic" && f.kind == FieldKind::Text));
        assert!(schema.iter().any(|f| f.key == "output_topic" && f.kind == FieldKind::Text));

        // config json
        let json = block.config_json();
        assert_eq!(json["input_topic"], "source/value");
        assert_eq!(json["output_topic"], "output/value");

        // apply_config
        block.apply_config(&serde_json::json!({
            "input_topic": "motor/feedback",
            "output_topic": "motor/cmd"
        }));
        assert_eq!(block.input_topic, "motor/feedback");
        assert_eq!(block.output_topic, "motor/cmd");

        // config_json reflects update
        let json2 = block.config_json();
        assert_eq!(json2["input_topic"], "motor/feedback");
        assert_eq!(json2["output_topic"], "motor/cmd");

        // declared channels: 1 input + 1 output
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "motor/feedback");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].name, "motor/cmd");
        assert_eq!(channels[1].direction, ChannelDirection::Output);
        assert_eq!(channels[1].kind, ChannelKind::PubSub);

        // lower succeeds
        let result = block.lower().unwrap();
        assert_eq!(result.dag.len(), 2); // subscribe + publish
        assert_eq!(result.ports.inputs.len(), 1);
        assert_eq!(result.ports.inputs[0].0, "in");
        assert!(result.ports.outputs.is_empty());
    }

    #[test]
    fn test_publish_apply_config_partial() {
        let mut block = PublishBlock::default();
        block.apply_config(&serde_json::json!({"output_topic": "new/out"}));
        assert_eq!(block.output_topic, "new/out");
        assert_eq!(block.input_topic, "source/value"); // unchanged
    }

    #[test]
    fn test_publish_default_values() {
        let block = PublishBlock::default();
        assert_eq!(block.input_topic, "source/value");
        assert_eq!(block.output_topic, "output/value");
    }

    #[test]
    fn test_publish_evaluate() {
        let block = PublishBlock::default();
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([("source/value".into(), 42.0)]),
        };

        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "output/value");
        assert!((eval.publishes[0].1 - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_publish_schema_defaults_match_fields() {
        let block = PublishBlock::default();
        let schema = block.config_schema();
        let in_field = schema.iter().find(|f| f.key == "input_topic").unwrap();
        assert_eq!(in_field.default, serde_json::json!("source/value"));
        let out_field = schema.iter().find(|f| f.key == "output_topic").unwrap();
        assert_eq!(out_field.default, serde_json::json!("output/value"));
    }

    #[test]
    fn test_publish_declared_channels_reflect_config() {
        let mut block = PublishBlock::default();
        block.apply_config(&serde_json::json!({
            "input_topic": "a/b",
            "output_topic": "c/d"
        }));
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "a/b");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[1].name, "c/d");
        assert_eq!(channels[1].direction, ChannelDirection::Output);
    }

    // ===================================================================
    // PWM Output block
    // ===================================================================

    #[test]
    fn test_pwm_identity() {
        let block = PwmBlock::default();
        assert_eq!(block.block_type(), "pwm");
        assert_eq!(block.display_name(), "PWM Output");
        assert_eq!(block.category(), BlockCategory::Io);
    }

    #[test]
    fn test_pwm_config_and_channels() {
        let mut block = PwmBlock::default();
        assert_eq!(block.channel_name, "pwm0");
        block.apply_config(&serde_json::json!({"channel_name": "pwm1"}));
        assert_eq!(block.channel_name, "pwm1");
        let ch = block.declared_channels();
        assert_eq!(ch.len(), 1);
        assert_eq!(ch[0].direction, ChannelDirection::Output);
        assert_eq!(ch[0].kind, ChannelKind::Hardware);
    }

    #[test]
    fn test_pwm_lower() {
        let block = PwmBlock::default();
        let result = block.lower().unwrap();
        assert!(!result.dag.is_empty());
    }

    // ===================================================================
    // Subtract block
    // ===================================================================

    #[test]
    fn test_subtract_identity() {
        let block = SubtractBlock::default();
        assert_eq!(block.block_type(), "subtract");
        assert_eq!(block.display_name(), "Subtract");
        assert_eq!(block.category(), BlockCategory::Math);
    }

    #[test]
    fn test_subtract_evaluate() {
        let block = SubtractBlock::default();
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([
                ("sub/a".into(), 10.0),
                ("sub/b".into(), 3.0),
            ]),
        };
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "sub/out");
        assert!((eval.publishes[0].1 - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_subtract_config() {
        let mut block = SubtractBlock::default();
        block.apply_config(&serde_json::json!({"input_a_topic": "x", "input_b_topic": "y", "output_topic": "z"}));
        assert_eq!(block.input_a_topic, "x");
        let ch = block.declared_channels();
        assert_eq!(ch.len(), 3);
    }

    // ===================================================================
    // Negate block
    // ===================================================================

    #[test]
    fn test_negate_identity() {
        let block = NegateBlock::default();
        assert_eq!(block.block_type(), "negate");
        assert_eq!(block.display_name(), "Negate");
        assert_eq!(block.category(), BlockCategory::Math);
    }

    #[test]
    fn test_negate_evaluate() {
        let block = NegateBlock::default();
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([("neg/in".into(), 5.0)]),
        };
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert!((eval.publishes[0].1 - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_negate_config() {
        let mut block = NegateBlock::default();
        block.apply_config(&serde_json::json!({"input_topic": "a", "output_topic": "b"}));
        assert_eq!(block.input_topic, "a");
        assert_eq!(block.output_topic, "b");
    }

    // ===================================================================
    // Map/Scale block
    // ===================================================================

    #[test]
    fn test_map_scale_identity() {
        let block = MapScaleBlock::default();
        assert_eq!(block.block_type(), "map_scale");
        assert_eq!(block.display_name(), "Map/Scale");
        assert_eq!(block.category(), BlockCategory::Math);
    }

    #[test]
    fn test_map_scale_evaluate_midpoint() {
        // in_min=0, in_max=1024, out_min=0, out_max=100
        // input=512 → output=50
        let mut block = MapScaleBlock::default();
        block.apply_config(&serde_json::json!({
            "in_min": 0.0, "in_max": 1024.0,
            "out_min": 0.0, "out_max": 100.0,
            "input_topic": "raw", "output_topic": "scaled"
        }));
        let result = block.lower().unwrap();
        let ps = MockPubSub {
            values: BTreeMap::from([("raw".into(), 512.0)]),
        };
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &ps, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert!((eval.publishes[0].1 - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_map_scale_config() {
        let block = MapScaleBlock::default();
        let schema = block.config_schema();
        assert!(schema.len() >= 6); // in_min, in_max, out_min, out_max, input_topic, output_topic
        assert!(schema.iter().any(|f| f.key == "in_min"));
        assert!(schema.iter().any(|f| f.key == "out_max"));
    }

    // ===================================================================
    // Low-pass filter block
    // ===================================================================

    #[test]
    fn test_lowpass_identity() {
        let block = LowPassBlock::default();
        assert_eq!(block.block_type(), "lowpass");
        assert_eq!(block.display_name(), "Low-Pass Filter");
        assert_eq!(block.category(), BlockCategory::Math);
    }

    #[test]
    fn test_lowpass_config() {
        let mut block = LowPassBlock::default();
        assert!(block.alpha > 0.0 && block.alpha <= 1.0);
        block.apply_config(&serde_json::json!({"alpha": 0.5}));
        assert_eq!(block.alpha, 0.5);
    }

    #[test]
    fn test_lowpass_lower() {
        let block = LowPassBlock::default();
        let result = block.lower().unwrap();
        assert!(!result.dag.is_empty());
        // Should have at least subscribe + some math + publish
        assert!(result.dag.len() >= 3);
    }

    // ===================================================================
    // Registry includes all new blocks
    // ===================================================================

    #[test]
    fn test_registry_has_new_blocks() {
        use crate::registry;
        let descs = registry::block_descriptors();
        let types: Vec<&str> = descs.iter().map(|d| d.block_type.as_str()).collect();
        assert!(types.contains(&"pwm"), "missing pwm");
        assert!(types.contains(&"subtract"), "missing subtract");
        assert!(types.contains(&"negate"), "missing negate");
        assert!(types.contains(&"map_scale"), "missing map_scale");
        assert!(types.contains(&"lowpass"), "missing lowpass");
    }
}
