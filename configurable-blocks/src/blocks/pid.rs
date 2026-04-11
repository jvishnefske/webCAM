//! Demo configurable blocks: PID controller, simple gain, and pubsub bridge.

use dag_core::op::{Dag, DagError};
use dag_core::templates::BlockPorts;
use serde::{Deserialize, Serialize};

use crate::lower::{ConfigurableBlock, LowerResult};
use crate::schema::*;

// ---------------------------------------------------------------------------
// PID Controller
// ---------------------------------------------------------------------------

/// PID controller block.
///
/// Subscribes to setpoint and feedback pubsub topics, computes the PID
/// output, clamps it, and publishes to an output topic.
///
/// The block lowers to DAG IL as:
/// ```text
///   %0 = Subscribe("setpoint_topic")
///   %1 = Subscribe("feedback_topic")
///   %2 = Sub(%0, %1)                    // error = setpoint - feedback
///   %3 = Const(kp)
///   %4 = Mul(%2, %3)                    // P = error * kp
///   %5 = Const(ki)
///   %6 = Mul(%2, %5)                    // I_term = error * ki (per-tick accumulation)
///   %7 = Const(kd)
///   %8 = Neg(%2)
///   %9 = Mul(%8, %7)                    // D_approx = -error * kd (derivative approx)
///   %10 = Add(%4, %6)                   // P + I_term
///   %11 = Add(%10, %9)                  // P + I_term + D_approx = raw output
///   // Clamp to [out_min, out_max]
///   ...clamp ops...
///   %N = Publish("output_topic", clamped)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidBlock {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
    pub setpoint_topic: String,
    pub feedback_topic: String,
    pub output_topic: String,
    pub out_min: f64,
    pub out_max: f64,
    pub deploy_node: String,
}

impl Default for PidBlock {
    fn default() -> Self {
        PidBlock {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_topic: "ctrl/setpoint".into(),
            feedback_topic: "ctrl/feedback".into(),
            output_topic: "ctrl/output".into(),
            out_min: -100.0,
            out_max: 100.0,
            deploy_node: "pico2".into(),
        }
    }
}

impl ConfigurableBlock for PidBlock {
    fn block_type(&self) -> &str {
        "pid"
    }
    fn display_name(&self) -> &str {
        "PID Controller"
    }
    fn category(&self) -> BlockCategory {
        BlockCategory::Control
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "kp".into(),
                label: "Proportional Gain (Kp)".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(1.0),
            },
            ConfigField {
                key: "ki".into(),
                label: "Integral Gain (Ki)".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(0.1),
            },
            ConfigField {
                key: "kd".into(),
                label: "Derivative Gain (Kd)".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(0.01),
            },
            ConfigField {
                key: "setpoint_topic".into(),
                label: "Setpoint Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("ctrl/setpoint"),
            },
            ConfigField {
                key: "feedback_topic".into(),
                label: "Feedback Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("ctrl/feedback"),
            },
            ConfigField {
                key: "output_topic".into(),
                label: "Output Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("ctrl/output"),
            },
            ConfigField {
                key: "out_min".into(),
                label: "Output Min".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(-100.0),
            },
            ConfigField {
                key: "out_max".into(),
                label: "Output Max".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(100.0),
            },
            ConfigField {
                key: "deploy_node".into(),
                label: "Deploy Target Node".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("pico2"),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("kp").and_then(|v| v.as_f64()) {
            self.kp = v;
        }
        if let Some(v) = config.get("ki").and_then(|v| v.as_f64()) {
            self.ki = v;
        }
        if let Some(v) = config.get("kd").and_then(|v| v.as_f64()) {
            self.kd = v;
        }
        if let Some(v) = config.get("setpoint_topic").and_then(|v| v.as_str()) {
            self.setpoint_topic = v.into();
        }
        if let Some(v) = config.get("feedback_topic").and_then(|v| v.as_str()) {
            self.feedback_topic = v.into();
        }
        if let Some(v) = config.get("output_topic").and_then(|v| v.as_str()) {
            self.output_topic = v.into();
        }
        if let Some(v) = config.get("out_min").and_then(|v| v.as_f64()) {
            self.out_min = v;
        }
        if let Some(v) = config.get("out_max").and_then(|v| v.as_f64()) {
            self.out_max = v;
        }
        if let Some(v) = config.get("deploy_node").and_then(|v| v.as_str()) {
            self.deploy_node = v.into();
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel {
                name: self.setpoint_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::PubSub,
            },
            DeclaredChannel {
                name: self.feedback_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::PubSub,
            },
            DeclaredChannel {
                name: self.output_topic.clone(),
                direction: ChannelDirection::Output,
                kind: ChannelKind::PubSub,
            },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();

        // Subscribe to inputs
        let setpoint = dag.subscribe(&self.setpoint_topic)?; // %0
        let feedback = dag.subscribe(&self.feedback_topic)?; // %1

        // Error = setpoint - feedback
        let error = dag.sub(setpoint, feedback)?; // %2

        // P term: error * kp
        let kp_const = dag.constant(self.kp)?; // %3
        let p_term = dag.mul(error, kp_const)?; // %4

        // I term (per-tick contribution): error * ki
        let ki_const = dag.constant(self.ki)?; // %5
        let i_term = dag.mul(error, ki_const)?; // %6

        // D term approximation: -error * kd (negative for derivative)
        let kd_const = dag.constant(self.kd)?; // %7
        let neg_error = dag.neg(error)?; // %8
        let d_term = dag.mul(neg_error, kd_const)?; // %9

        // Sum: P + I + D
        let pi = dag.add(p_term, i_term)?; // %10
        let raw_output = dag.add(pi, d_term)?; // %11

        // Clamp to [out_min, out_max] using relu decomposition:
        // clamp(x, min, max) = relu(x - min) + min - relu(relu(x - min) + min - max)
        let min_const = dag.constant(self.out_min)?;
        let max_const = dag.constant(self.out_max)?;
        let shifted_low = dag.sub(raw_output, min_const)?;
        let clamped_low = dag.relu(shifted_low)?;
        let above_min = dag.add(clamped_low, min_const)?;
        let shifted_high = dag.sub(above_min, max_const)?;
        let excess = dag.relu(shifted_high)?;
        let clamped = dag.sub(above_min, excess)?;

        // Publish clamped output
        let pub_node = dag.publish(&self.output_topic, clamped)?;

        let ports = BlockPorts {
            inputs: vec![("setpoint".into(), setpoint), ("feedback".into(), feedback)],
            outputs: vec![("output".into(), pub_node)],
        };

        Ok(LowerResult { ports, dag })
    }
}

// ---------------------------------------------------------------------------
// Simple Gain Block
// ---------------------------------------------------------------------------

/// Simple gain block: subscribe to input, multiply by factor, publish to output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleGainBlock {
    pub factor: f64,
    pub input_topic: String,
    pub output_topic: String,
    pub deploy_node: String,
}

impl Default for SimpleGainBlock {
    fn default() -> Self {
        SimpleGainBlock {
            factor: 1.0,
            input_topic: "signal/in".into(),
            output_topic: "signal/out".into(),
            deploy_node: "pico2".into(),
        }
    }
}

impl ConfigurableBlock for SimpleGainBlock {
    fn block_type(&self) -> &str {
        "gain"
    }
    fn display_name(&self) -> &str {
        "Gain"
    }
    fn category(&self) -> BlockCategory {
        BlockCategory::Math
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "factor".into(),
                label: "Gain Factor".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(1.0),
            },
            ConfigField {
                key: "input_topic".into(),
                label: "Input Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("signal/in"),
            },
            ConfigField {
                key: "output_topic".into(),
                label: "Output Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("signal/out"),
            },
            ConfigField {
                key: "deploy_node".into(),
                label: "Deploy Target Node".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("pico2"),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("factor").and_then(|v| v.as_f64()) {
            self.factor = v;
        }
        if let Some(v) = config.get("input_topic").and_then(|v| v.as_str()) {
            self.input_topic = v.into();
        }
        if let Some(v) = config.get("output_topic").and_then(|v| v.as_str()) {
            self.output_topic = v.into();
        }
        if let Some(v) = config.get("deploy_node").and_then(|v| v.as_str()) {
            self.deploy_node = v.into();
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel {
                name: self.input_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::PubSub,
            },
            DeclaredChannel {
                name: self.output_topic.clone(),
                direction: ChannelDirection::Output,
                kind: ChannelKind::PubSub,
            },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let input = dag.subscribe(&self.input_topic)?;
        let factor = dag.constant(self.factor)?;
        let scaled = dag.mul(input, factor)?;
        let pub_node = dag.publish(&self.output_topic, scaled)?;

        let ports = BlockPorts {
            inputs: vec![("in".into(), input)],
            outputs: vec![("out".into(), pub_node)],
        };
        Ok(LowerResult { ports, dag })
    }
}

// ---------------------------------------------------------------------------
// PubSub Bridge Block
// ---------------------------------------------------------------------------

/// Bridge between two pubsub topics with optional gain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubBridgeBlock {
    pub subscribe_topic: String,
    pub publish_topic: String,
    pub gain: f64,
    pub deploy_node: String,
}

impl Default for PubSubBridgeBlock {
    fn default() -> Self {
        PubSubBridgeBlock {
            subscribe_topic: "sensor/raw".into(),
            publish_topic: "sensor/scaled".into(),
            gain: 1.0,
            deploy_node: "pico2".into(),
        }
    }
}

impl ConfigurableBlock for PubSubBridgeBlock {
    fn block_type(&self) -> &str {
        "pubsub_bridge"
    }
    fn display_name(&self) -> &str {
        "PubSub Bridge"
    }
    fn category(&self) -> BlockCategory {
        BlockCategory::PubSub
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "subscribe_topic".into(),
                label: "Subscribe Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("sensor/raw"),
            },
            ConfigField {
                key: "publish_topic".into(),
                label: "Publish Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("sensor/scaled"),
            },
            ConfigField {
                key: "gain".into(),
                label: "Gain".into(),
                kind: FieldKind::Float,
                default: serde_json::json!(1.0),
            },
            ConfigField {
                key: "deploy_node".into(),
                label: "Deploy Target Node".into(),
                kind: FieldKind::Text,
                default: serde_json::json!("pico2"),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("subscribe_topic").and_then(|v| v.as_str()) {
            self.subscribe_topic = v.into();
        }
        if let Some(v) = config.get("publish_topic").and_then(|v| v.as_str()) {
            self.publish_topic = v.into();
        }
        if let Some(v) = config.get("gain").and_then(|v| v.as_f64()) {
            self.gain = v;
        }
        if let Some(v) = config.get("deploy_node").and_then(|v| v.as_str()) {
            self.deploy_node = v.into();
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![
            DeclaredChannel {
                name: self.subscribe_topic.clone(),
                direction: ChannelDirection::Input,
                kind: ChannelKind::PubSub,
            },
            DeclaredChannel {
                name: self.publish_topic.clone(),
                direction: ChannelDirection::Output,
                kind: ChannelKind::PubSub,
            },
        ]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();
        let input = dag.subscribe(&self.subscribe_topic)?;
        let result = if (self.gain - 1.0).abs() < f64::EPSILON {
            // Unity gain: skip the multiply
            input
        } else {
            let factor = dag.constant(self.gain)?;
            dag.mul(input, factor)?
        };
        let pub_node = dag.publish(&self.publish_topic, result)?;

        let ports = BlockPorts {
            inputs: vec![("in".into(), input)],
            outputs: vec![("out".into(), pub_node)],
        };
        Ok(LowerResult { ports, dag })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dag_core::eval::{NullChannels, PubSubReader};
    use std::collections::BTreeMap;

    struct MockPubSub {
        values: BTreeMap<String, f64>,
    }

    impl PubSubReader for MockPubSub {
        fn read(&self, topic: &str) -> f64 {
            self.values.get(topic).copied().unwrap_or(0.0)
        }
    }

    #[test]
    fn test_pid_lower_and_evaluate() {
        let pid = PidBlock {
            kp: 2.0,
            ki: 0.0,
            kd: 0.0,
            setpoint_topic: "sp".into(),
            feedback_topic: "fb".into(),
            output_topic: "out".into(),
            out_min: -10.0,
            out_max: 10.0,
            deploy_node: "pico2".into(),
        };

        let result = pid.lower().expect("lower failed");
        let dag = &result.dag;

        // With pure P control (ki=kd=0), output = clamp(error * kp, -10, 10)
        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("sp".into(), 5.0);
        pubsub.values.insert("fb".into(), 3.0);

        let mut values = vec![0.0; dag.len()];
        let eval = dag.evaluate(&NullChannels, &pubsub, &mut values);

        // error = 5 - 3 = 2, P = 2 * 2 = 4, clamped to [-10, 10] -> 4.0
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "out");
        assert!((eval.publishes[0].1 - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_pid_clamping() {
        let pid = PidBlock {
            kp: 100.0,
            ki: 0.0,
            kd: 0.0,
            setpoint_topic: "sp".into(),
            feedback_topic: "fb".into(),
            output_topic: "out".into(),
            out_min: -5.0,
            out_max: 5.0,
            deploy_node: "pico2".into(),
        };

        let result = pid.lower().expect("lower failed");
        let dag = &result.dag;

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("sp".into(), 10.0);
        pubsub.values.insert("fb".into(), 0.0);

        let mut values = vec![0.0; dag.len()];
        let eval = dag.evaluate(&NullChannels, &pubsub, &mut values);

        // error = 10, P = 1000, clamped to 5.0
        assert!((eval.publishes[0].1 - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_pid_cbor_round_trip() {
        let pid = PidBlock::default();
        let result = pid.lower().expect("lower failed");
        let bytes = dag_core::cbor::encode_dag(&result.dag);
        let decoded = dag_core::cbor::decode_dag(&bytes).expect("decode failed");
        assert_eq!(result.dag.len(), decoded.len());
        assert_eq!(result.dag.nodes(), decoded.nodes());
    }

    #[test]
    fn test_pid_config_schema() {
        let pid = PidBlock::default();
        let schema = pid.config_schema();
        assert!(schema.iter().any(|f| f.key == "kp"));
        assert!(schema.iter().any(|f| f.key == "setpoint_topic"));
        assert!(schema.iter().any(|f| f.key == "deploy_node"));
    }

    #[test]
    fn test_pid_apply_config() {
        let mut pid = PidBlock::default();
        pid.apply_config(&serde_json::json!({
            "kp": 5.0,
            "setpoint_topic": "motor/sp",
            "output_topic": "motor/cmd"
        }));
        assert_eq!(pid.kp, 5.0);
        assert_eq!(pid.setpoint_topic, "motor/sp");
        assert_eq!(pid.output_topic, "motor/cmd");
    }

    #[test]
    fn test_pid_declared_channels() {
        let pid = PidBlock::default();
        let channels = pid.declared_channels();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[2].direction, ChannelDirection::Output);
    }

    #[test]
    fn test_gain_lower() {
        let gain = SimpleGainBlock {
            factor: 3.0,
            input_topic: "in".into(),
            output_topic: "out".into(),
            deploy_node: "pico2".into(),
        };
        let result = gain.lower().expect("lower failed");
        let dag = &result.dag;

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("in".into(), 4.0);

        let mut values = vec![0.0; dag.len()];
        let eval = dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(eval.publishes[0].0, "out");
        assert!((eval.publishes[0].1 - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_bridge_unity_gain() {
        let bridge = PubSubBridgeBlock {
            subscribe_topic: "a".into(),
            publish_topic: "b".into(),
            gain: 1.0,
            deploy_node: "pico2".into(),
        };
        let result = bridge.lower().expect("lower failed");
        // Unity gain should have only 2 nodes: Subscribe + Publish
        assert_eq!(result.dag.len(), 2);
    }

    #[test]
    fn test_registry() {
        let entries = super::super::registry();
        assert!(entries.len() >= 3);
        assert!(entries.iter().any(|e| e.block_type == "pid"));
    }

    #[test]
    fn test_registry_by_category() {
        let groups = super::super::registry_by_category();
        assert!(!groups.is_empty());
        // Control category should have PID
        let control = groups
            .iter()
            .find(|(cat, _)| *cat == BlockCategory::Control);
        assert!(control.is_some());
        assert!(control.unwrap().1.iter().any(|e| e.block_type == "pid"));
    }

    // --- PidBlock: block_type, display_name, category, config_json ---
    #[test]
    fn test_pid_block_type_and_display() {
        let pid = PidBlock::default();
        assert_eq!(pid.block_type(), "pid");
        assert_eq!(pid.display_name(), "PID Controller");
        assert_eq!(pid.category(), BlockCategory::Control);
    }

    #[test]
    fn test_pid_config_json() {
        let pid = PidBlock::default();
        let json = pid.config_json();
        assert_eq!(json["kp"], 1.0);
        assert_eq!(json["ki"], 0.1);
        assert_eq!(json["kd"], 0.01);
        assert_eq!(json["setpoint_topic"], "ctrl/setpoint");
        assert_eq!(json["feedback_topic"], "ctrl/feedback");
        assert_eq!(json["output_topic"], "ctrl/output");
        assert_eq!(json["out_min"], -100.0);
        assert_eq!(json["out_max"], 100.0);
        assert_eq!(json["deploy_node"], "pico2");
    }

    // --- SimpleGainBlock: all ConfigurableBlock methods ---
    #[test]
    fn test_gain_block_type_and_display() {
        let g = SimpleGainBlock::default();
        assert_eq!(g.block_type(), "gain");
        assert_eq!(g.display_name(), "Gain");
        assert_eq!(g.category(), BlockCategory::Math);
    }

    #[test]
    fn test_gain_config_schema() {
        let g = SimpleGainBlock::default();
        let schema = g.config_schema();
        assert_eq!(schema.len(), 4);
        assert!(schema.iter().any(|f| f.key == "factor"));
        assert!(schema.iter().any(|f| f.key == "input_topic"));
        assert!(schema.iter().any(|f| f.key == "output_topic"));
        assert!(schema.iter().any(|f| f.key == "deploy_node"));
    }

    #[test]
    fn test_gain_config_json() {
        let g = SimpleGainBlock::default();
        let json = g.config_json();
        assert_eq!(json["factor"], 1.0);
        assert_eq!(json["input_topic"], "signal/in");
        assert_eq!(json["output_topic"], "signal/out");
        assert_eq!(json["deploy_node"], "pico2");
    }

    #[test]
    fn test_gain_apply_config() {
        let mut g = SimpleGainBlock::default();
        g.apply_config(&serde_json::json!({
            "factor": 5.0,
            "input_topic": "x/in",
            "output_topic": "x/out",
            "deploy_node": "pico3"
        }));
        assert_eq!(g.factor, 5.0);
        assert_eq!(g.input_topic, "x/in");
        assert_eq!(g.output_topic, "x/out");
        assert_eq!(g.deploy_node, "pico3");
    }

    #[test]
    fn test_gain_declared_channels() {
        let g = SimpleGainBlock::default();
        let channels = g.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].direction, ChannelDirection::Output);
    }

    // --- PubSubBridgeBlock: all ConfigurableBlock methods ---
    #[test]
    fn test_bridge_block_type_and_display() {
        let b = PubSubBridgeBlock::default();
        assert_eq!(b.block_type(), "pubsub_bridge");
        assert_eq!(b.display_name(), "PubSub Bridge");
        assert_eq!(b.category(), BlockCategory::PubSub);
    }

    #[test]
    fn test_bridge_config_schema() {
        let b = PubSubBridgeBlock::default();
        let schema = b.config_schema();
        assert_eq!(schema.len(), 4);
        assert!(schema.iter().any(|f| f.key == "subscribe_topic"));
        assert!(schema.iter().any(|f| f.key == "publish_topic"));
        assert!(schema.iter().any(|f| f.key == "gain"));
        assert!(schema.iter().any(|f| f.key == "deploy_node"));
    }

    #[test]
    fn test_bridge_config_json() {
        let b = PubSubBridgeBlock::default();
        let json = b.config_json();
        assert_eq!(json["subscribe_topic"], "sensor/raw");
        assert_eq!(json["publish_topic"], "sensor/scaled");
        assert_eq!(json["gain"], 1.0);
        assert_eq!(json["deploy_node"], "pico2");
    }

    #[test]
    fn test_bridge_apply_config() {
        let mut b = PubSubBridgeBlock::default();
        b.apply_config(&serde_json::json!({
            "subscribe_topic": "a/b",
            "publish_topic": "c/d",
            "gain": 2.5,
            "deploy_node": "pico4"
        }));
        assert_eq!(b.subscribe_topic, "a/b");
        assert_eq!(b.publish_topic, "c/d");
        assert_eq!(b.gain, 2.5);
        assert_eq!(b.deploy_node, "pico4");
    }

    #[test]
    fn test_bridge_declared_channels() {
        let b = PubSubBridgeBlock::default();
        let channels = b.declared_channels();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].direction, ChannelDirection::Output);
    }

    #[test]
    fn test_bridge_non_unity_gain_lower() {
        let bridge = PubSubBridgeBlock {
            subscribe_topic: "a".into(),
            publish_topic: "b".into(),
            gain: 2.0,
            deploy_node: "pico2".into(),
        };
        let result = bridge.lower().expect("lower failed");
        // Non-unity gain: Subscribe + Const + Mul + Publish = 4 nodes
        assert_eq!(result.dag.len(), 4);

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("a".into(), 5.0);

        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "b");
        assert!((eval.publishes[0].1 - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_pid_default() {
        let pid = PidBlock::default();
        assert_eq!(pid.kp, 1.0);
        assert_eq!(pid.ki, 0.1);
        assert_eq!(pid.kd, 0.01);
        assert_eq!(pid.out_min, -100.0);
        assert_eq!(pid.out_max, 100.0);
    }

    #[test]
    fn test_gain_default() {
        let g = SimpleGainBlock::default();
        assert_eq!(g.factor, 1.0);
        assert_eq!(g.input_topic, "signal/in");
    }

    #[test]
    fn test_bridge_default() {
        let b = PubSubBridgeBlock::default();
        assert_eq!(b.subscribe_topic, "sensor/raw");
        assert_eq!(b.publish_topic, "sensor/scaled");
        assert_eq!(b.gain, 1.0);
    }

    // ===================================================================
    // PidBlock -- declared_channels directly
    // ===================================================================

    #[test]
    fn test_pid_declared_channels_names() {
        let pid = PidBlock {
            setpoint_topic: "sp".into(),
            feedback_topic: "fb".into(),
            output_topic: "out".into(),
            ..PidBlock::default()
        };
        let channels = pid.declared_channels();
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0].name, "sp");
        assert_eq!(channels[0].direction, ChannelDirection::Input);
        assert_eq!(channels[0].kind, ChannelKind::PubSub);
        assert_eq!(channels[1].name, "fb");
        assert_eq!(channels[1].direction, ChannelDirection::Input);
        assert_eq!(channels[2].name, "out");
        assert_eq!(channels[2].direction, ChannelDirection::Output);
    }

    // ===================================================================
    // PidBlock -- partial config application
    // ===================================================================

    #[test]
    fn test_pid_apply_config_partial() {
        let mut pid = PidBlock::default();
        // Only update kp and feedback_topic; leave others at default
        pid.apply_config(&serde_json::json!({
            "kp": 10.0,
            "feedback_topic": "temp/fb"
        }));
        assert_eq!(pid.kp, 10.0);
        assert_eq!(pid.feedback_topic, "temp/fb");
        // These should remain at defaults
        assert_eq!(pid.ki, 0.1);
        assert_eq!(pid.kd, 0.01);
        assert_eq!(pid.setpoint_topic, "ctrl/setpoint");
        assert_eq!(pid.output_topic, "ctrl/output");
        assert_eq!(pid.out_min, -100.0);
        assert_eq!(pid.out_max, 100.0);
        assert_eq!(pid.deploy_node, "pico2");
    }

    #[test]
    fn test_pid_apply_config_all_fields() {
        let mut pid = PidBlock::default();
        pid.apply_config(&serde_json::json!({
            "kp": 5.0,
            "ki": 0.5,
            "kd": 0.05,
            "setpoint_topic": "motor/sp",
            "feedback_topic": "motor/fb",
            "output_topic": "motor/cmd",
            "out_min": -50.0,
            "out_max": 50.0,
            "deploy_node": "stm32"
        }));
        assert_eq!(pid.kp, 5.0);
        assert_eq!(pid.ki, 0.5);
        assert_eq!(pid.kd, 0.05);
        assert_eq!(pid.setpoint_topic, "motor/sp");
        assert_eq!(pid.feedback_topic, "motor/fb");
        assert_eq!(pid.output_topic, "motor/cmd");
        assert_eq!(pid.out_min, -50.0);
        assert_eq!(pid.out_max, 50.0);
        assert_eq!(pid.deploy_node, "stm32");
    }

    // ===================================================================
    // SimpleGainBlock::lower -- evaluate
    // ===================================================================

    #[test]
    fn test_gain_lower_evaluate() {
        let gain = SimpleGainBlock {
            factor: 2.5,
            input_topic: "in".into(),
            output_topic: "out".into(),
            deploy_node: "pico2".into(),
        };
        let result = gain.lower().expect("lower failed");
        let dag = &result.dag;
        // Subscribe + Const + Mul + Publish = 4 nodes
        assert_eq!(dag.len(), 4);
        assert_eq!(result.ports.inputs.len(), 1);
        assert_eq!(result.ports.outputs.len(), 1);

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("in".into(), 4.0);
        let mut values = vec![0.0; dag.len()];
        let eval = dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "out");
        assert!((eval.publishes[0].1 - 10.0).abs() < 1e-10);
    }

    // ===================================================================
    // PubSubBridgeBlock::lower -- unity vs non-unity gain
    // ===================================================================

    #[test]
    fn test_bridge_unity_gain_evaluate() {
        let bridge = PubSubBridgeBlock {
            subscribe_topic: "x".into(),
            publish_topic: "y".into(),
            gain: 1.0,
            deploy_node: "pico2".into(),
        };
        let result = bridge.lower().expect("lower failed");
        // Unity: Subscribe + Publish = 2 nodes
        assert_eq!(result.dag.len(), 2);

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("x".into(), 7.0);
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "y");
        assert!((eval.publishes[0].1 - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_bridge_non_unity_gain_evaluate() {
        let bridge = PubSubBridgeBlock {
            subscribe_topic: "x".into(),
            publish_topic: "y".into(),
            gain: 0.5,
            deploy_node: "pico2".into(),
        };
        let result = bridge.lower().expect("lower failed");
        // Non-unity: Subscribe + Const + Mul + Publish = 4 nodes
        assert_eq!(result.dag.len(), 4);

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("x".into(), 10.0);
        let mut values = vec![0.0; result.dag.len()];
        let eval = result.dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(eval.publishes.len(), 1);
        assert_eq!(eval.publishes[0].0, "y");
        assert!((eval.publishes[0].1 - 5.0).abs() < 1e-10);
    }
}
