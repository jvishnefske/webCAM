//! State machine block: configurable finite state machine with topic bindings.
//!
//! Config JSON (legacy guard-port style):
//! ```json
//! {
//!   "states": ["idle", "running", "error"],
//!   "initial": "idle",
//!   "transitions": [
//!     { "from": "idle", "to": "running", "guard": { "type": "GuardPort", "port": 0 } },
//!     { "from": "running", "to": "error", "guard": { "type": "GuardPort", "port": 1 } },
//!     { "from": "error", "to": "idle", "guard": { "type": "Unconditional" } }
//!   ]
//! }
//! ```
//!
//! Config JSON (topic-based style):
//! ```json
//! {
//!   "states": ["idle", "running"],
//!   "initial": "idle",
//!   "transitions": [
//!     {
//!       "from": "idle", "to": "running",
//!       "guard": { "type": "Topic", "topic": "motor_cmd", "condition": { "field": "speed", "op": "Gt", "value": 0.0 } },
//!       "actions": [{ "topic": "motor_status", "message": [["running", 1.0]] }]
//!     }
//!   ],
//!   "input_topics": [{ "topic": "motor_cmd", "schema": { "name": "MotorCmd", "fields": [{ "name": "speed", "field_type": "F64" }] } }],
//!   "output_topics": [{ "topic": "motor_status", "schema": { "name": "MotorStatus", "fields": [{ "name": "running", "field_type": "F64" }] } }]
//! }
//! ```
//!
//! Input ports: guard ports first (`guard_0`, `guard_1`, ..., Float), then one per input_topic (Message).
//! Output ports: `state` (Float), `active_<name>` per state (Float), then one per output_topic (Message).

use crate::dataflow::block::{
    MessageData, MessageSchema, Module, PortDef, PortKind, Tick, Value,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateMachineConfig {
    pub states: Vec<String>,
    pub initial: String,
    #[serde(default)]
    pub transitions: Vec<TransitionConfig>,
    #[serde(default)]
    pub input_topics: Vec<TopicBinding>,
    #[serde(default)]
    pub output_topics: Vec<TopicBinding>,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            states: vec!["idle".to_string()],
            initial: "idle".to_string(),
            transitions: vec![],
            input_topics: vec![],
            output_topics: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionConfig {
    pub from: String,
    pub to: String,
    pub guard: TransitionGuard,
    #[serde(default)]
    pub actions: Vec<TransitionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TransitionGuard {
    Topic {
        topic: String,
        #[serde(default)]
        condition: Option<FieldCondition>,
    },
    Unconditional,
    GuardPort {
        port: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldCondition {
    pub field: String,
    pub op: CompareOp,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

impl CompareOp {
    fn apply(&self, lhs: f64, rhs: f64) -> bool {
        match self {
            Self::Eq => (lhs - rhs).abs() < f64::EPSILON,
            Self::Ne => (lhs - rhs).abs() >= f64::EPSILON,
            Self::Gt => lhs > rhs,
            Self::Lt => lhs < rhs,
            Self::Ge => lhs >= rhs,
            Self::Le => lhs <= rhs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionAction {
    pub topic: String,
    pub message: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopicBinding {
    pub topic: String,
    pub schema: MessageSchema,
}

// ---------------------------------------------------------------------------
// Block implementation
// ---------------------------------------------------------------------------

pub struct StateMachineBlock {
    pub(crate) config: StateMachineConfig,
    pub(crate) current_state: usize,
}

impl StateMachineBlock {
    pub fn from_config(config: StateMachineConfig) -> Self {
        let current_state = config
            .states
            .iter()
            .position(|s| s == &config.initial)
            .unwrap_or(0);
        Self {
            config,
            current_state,
        }
    }

    /// Number of guard ports required (max GuardPort index + 1).
    fn n_guard_ports(&self) -> usize {
        self.config
            .transitions
            .iter()
            .filter_map(|t| match &t.guard {
                TransitionGuard::GuardPort { port } => Some(*port),
                _ => None,
            })
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    /// Input index for a topic name (offset past guard ports).
    fn topic_input_index(&self, topic: &str) -> Option<usize> {
        let offset = self.n_guard_ports();
        self.config
            .input_topics
            .iter()
            .position(|b| b.topic == topic)
            .map(|i| offset + i)
    }

    /// Evaluate a transition guard against current inputs.
    fn check_guard(&self, guard: &TransitionGuard, inputs: &[Option<&Value>]) -> bool {
        match guard {
            TransitionGuard::Unconditional => true,
            TransitionGuard::GuardPort { port } => inputs
                .get(*port)
                .and_then(|v| v.as_ref())
                .and_then(|v| v.as_float())
                .unwrap_or(0.0)
                > 0.5,
            TransitionGuard::Topic { topic, condition } => {
                let idx = match self.topic_input_index(topic) {
                    Some(i) => i,
                    None => return false,
                };
                let msg = match inputs.get(idx).and_then(|v| v.as_ref()) {
                    Some(v) => match v.as_message() {
                        Some(m) => m,
                        None => return false,
                    },
                    None => return false,
                };
                match condition {
                    None => true, // message present is sufficient
                    Some(cond) => {
                        let field_val = msg
                            .fields
                            .iter()
                            .find(|(name, _)| name == &cond.field)
                            .map(|(_, v)| *v)
                            .unwrap_or(0.0);
                        cond.op.apply(field_val, cond.value)
                    }
                }
            }
        }
    }
}

impl Module for StateMachineBlock {
    fn name(&self) -> &str {
        "State Machine"
    }

    fn block_type(&self) -> &str {
        "state_machine"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        let mut ports: Vec<PortDef> = (0..self.n_guard_ports())
            .map(|i| PortDef::new(&format!("guard_{i}"), PortKind::Float))
            .collect();
        for binding in &self.config.input_topics {
            ports.push(PortDef::new(
                &binding.topic,
                PortKind::Message(binding.schema.clone()),
            ));
        }
        ports
    }

    fn output_ports(&self) -> Vec<PortDef> {
        let mut ports = vec![PortDef::new("state", PortKind::Float)];
        for state_name in &self.config.states {
            ports.push(PortDef::new(
                &format!("active_{state_name}"),
                PortKind::Float,
            ));
        }
        for binding in &self.config.output_topics {
            ports.push(PortDef::new(
                &binding.topic,
                PortKind::Message(binding.schema.clone()),
            ));
        }
        ports
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for StateMachineBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        let current_name = self.config.states[self.current_state].clone();

        // Evaluate transitions from current state (first match wins).
        let mut fired_actions: Vec<TransitionAction> = Vec::new();
        for t in &self.config.transitions {
            if t.from != current_name {
                continue;
            }
            if self.check_guard(&t.guard, inputs) {
                if let Some(idx) = self.config.states.iter().position(|s| s == &t.to) {
                    self.current_state = idx;
                    fired_actions = t.actions.clone();
                    break;
                }
            }
        }

        // Build outputs: state index, active flags, then output topic messages.
        let mut outputs: Vec<Option<Value>> =
            Vec::with_capacity(1 + self.config.states.len() + self.config.output_topics.len());

        outputs.push(Some(Value::Float(self.current_state as f64)));

        for (i, _) in self.config.states.iter().enumerate() {
            outputs.push(Some(Value::Float(if i == self.current_state {
                1.0
            } else {
                0.0
            })));
        }

        // For each output topic, emit a Message if an action fired for that topic.
        for binding in &self.config.output_topics {
            let action = fired_actions.iter().find(|a| a.topic == binding.topic);
            match action {
                Some(a) => {
                    outputs.push(Some(Value::Message(MessageData {
                        schema_name: binding.schema.name.clone(),
                        fields: a.message.clone(),
                    })));
                }
                None => {
                    outputs.push(None);
                }
            }
        }

        outputs
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::block::{FieldType, MessageField};

    /// Legacy-style state machine with GuardPort and Unconditional guards.
    fn make_legacy_sm() -> StateMachineBlock {
        StateMachineBlock::from_config(StateMachineConfig {
            states: vec![
                "idle".to_string(),
                "running".to_string(),
                "error".to_string(),
            ],
            initial: "idle".to_string(),
            transitions: vec![
                TransitionConfig {
                    from: "idle".to_string(),
                    to: "running".to_string(),
                    guard: TransitionGuard::GuardPort { port: 0 },
                    actions: vec![],
                },
                TransitionConfig {
                    from: "running".to_string(),
                    to: "error".to_string(),
                    guard: TransitionGuard::GuardPort { port: 1 },
                    actions: vec![],
                },
                TransitionConfig {
                    from: "error".to_string(),
                    to: "idle".to_string(),
                    guard: TransitionGuard::Unconditional,
                    actions: vec![],
                },
            ],
            input_topics: vec![],
            output_topics: vec![],
        })
    }

    /// Topic-based state machine with FieldCondition guard and TransitionAction.
    fn make_topic_sm() -> StateMachineBlock {
        StateMachineBlock::from_config(StateMachineConfig {
            states: vec!["idle".to_string(), "running".to_string()],
            initial: "idle".to_string(),
            transitions: vec![TransitionConfig {
                from: "idle".to_string(),
                to: "running".to_string(),
                guard: TransitionGuard::Topic {
                    topic: "motor_cmd".to_string(),
                    condition: Some(FieldCondition {
                        field: "speed".to_string(),
                        op: CompareOp::Gt,
                        value: 0.0,
                    }),
                },
                actions: vec![TransitionAction {
                    topic: "motor_status".to_string(),
                    message: vec![("running".to_string(), 1.0)],
                }],
            }],
            input_topics: vec![TopicBinding {
                topic: "motor_cmd".to_string(),
                schema: MessageSchema {
                    name: "MotorCmd".to_string(),
                    fields: vec![MessageField {
                        name: "speed".to_string(),
                        field_type: FieldType::F64,
                    }],
                },
            }],
            output_topics: vec![TopicBinding {
                topic: "motor_status".to_string(),
                schema: MessageSchema {
                    name: "MotorStatus".to_string(),
                    fields: vec![MessageField {
                        name: "running".to_string(),
                        field_type: FieldType::F64,
                    }],
                },
            }],
        })
    }

    #[test]
    fn legacy_initial_state() {
        let mut sm = make_legacy_sm();
        let result = sm.tick(&[], 0.01);
        // state=0 (idle), active_idle=1, active_running=0, active_error=0
        assert_eq!(result[0], Some(Value::Float(0.0)));
        assert_eq!(result[1], Some(Value::Float(1.0)));
        assert_eq!(result[2], Some(Value::Float(0.0)));
        assert_eq!(result[3], Some(Value::Float(0.0)));
    }

    #[test]
    fn legacy_guard_transition() {
        let mut sm = make_legacy_sm();
        let high = Value::Float(1.0);
        let low = Value::Float(0.0);

        // guard_0 = high -> idle->running
        let result = sm.tick(&[Some(&high), Some(&low)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
        assert_eq!(result[2], Some(Value::Float(1.0))); // active_running

        // guard_1 = high -> running->error
        let result = sm.tick(&[Some(&low), Some(&high)], 0.01);
        assert_eq!(result[0], Some(Value::Float(2.0))); // error
        assert_eq!(result[3], Some(Value::Float(1.0))); // active_error
    }

    #[test]
    fn legacy_unconditional_transition() {
        let mut sm = make_legacy_sm();
        // Force to error state
        sm.current_state = 2;
        // error->idle is unconditional
        let result = sm.tick(&[], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // back to idle
    }

    #[test]
    fn topic_input_ports() {
        let sm = make_topic_sm();
        let ports = sm.input_ports();
        // No guard ports, 1 topic input
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "motor_cmd");
        match &ports[0].kind {
            PortKind::Message(schema) => {
                assert_eq!(schema.name, "MotorCmd");
                assert_eq!(schema.fields.len(), 1);
                assert_eq!(schema.fields[0].name, "speed");
            }
            other => panic!("expected Message port, got {:?}", other),
        }
    }

    #[test]
    fn topic_output_ports() {
        let sm = make_topic_sm();
        let ports = sm.output_ports();
        // state + active_idle + active_running + motor_status
        assert_eq!(ports.len(), 4);
        assert_eq!(ports[0].name, "state");
        assert_eq!(ports[0].kind, PortKind::Float);
        assert_eq!(ports[1].name, "active_idle");
        assert_eq!(ports[2].name, "active_running");
        assert_eq!(ports[3].name, "motor_status");
        match &ports[3].kind {
            PortKind::Message(schema) => {
                assert_eq!(schema.name, "MotorStatus");
            }
            other => panic!("expected Message port, got {:?}", other),
        }
    }

    #[test]
    fn topic_guard_transition_with_message() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        // Topic input is at index 0 (no guard ports)
        let result = sm.tick(&[Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
        assert_eq!(result[1], Some(Value::Float(0.0))); // active_idle=0
        assert_eq!(result[2], Some(Value::Float(1.0))); // active_running=1
    }

    #[test]
    fn topic_guard_condition_not_met() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 0.0)],
        });
        // speed=0.0, condition is Gt 0.0 -> fails
        let result = sm.tick(&[Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // stays idle
        assert_eq!(result[1], Some(Value::Float(1.0))); // active_idle=1
    }

    #[test]
    fn topic_guard_no_message() {
        let mut sm = make_topic_sm();
        // None input -> guard not satisfied
        let result = sm.tick(&[None], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // stays idle
    }

    #[test]
    fn topic_action_emits_message() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        let result = sm.tick(&[Some(&msg)], 0.01);
        // Output index 3 = motor_status topic
        match &result[3] {
            Some(Value::Message(data)) => {
                assert_eq!(data.schema_name, "MotorStatus");
                assert_eq!(data.fields, vec![("running".to_string(), 1.0)]);
            }
            other => panic!("expected Some(Message), got {:?}", other),
        }
    }

    #[test]
    fn config_default() {
        let cfg = StateMachineConfig::default();
        assert_eq!(cfg.states, vec!["idle".to_string()]);
        assert_eq!(cfg.initial, "idle");
        assert!(cfg.transitions.is_empty());
        assert!(cfg.input_topics.is_empty());
        assert!(cfg.output_topics.is_empty());
    }

    #[test]
    fn config_serde_roundtrip() {
        let sm = make_topic_sm();
        let json = serde_json::to_string(&sm.config).unwrap();
        let deserialized: StateMachineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(sm.config, deserialized);
    }

    #[test]
    fn config_deserialize_minimal() {
        let json = r#"{"states":["a"],"initial":"a"}"#;
        let cfg: StateMachineConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.states, vec!["a".to_string()]);
        assert_eq!(cfg.initial, "a");
        assert!(cfg.transitions.is_empty());
        assert!(cfg.input_topics.is_empty());
        assert!(cfg.output_topics.is_empty());
    }

    #[test]
    fn module_trait_methods() {
        let mut sm = make_legacy_sm();
        assert_eq!(sm.name(), "State Machine");
        assert_eq!(sm.block_type(), "state_machine");
        let config: serde_json::Value = serde_json::from_str(&sm.config_json()).unwrap();
        assert_eq!(config["initial"], "idle");
        assert!(sm.as_tick().is_some());
        assert!(sm.as_analysis().is_none());
        assert!(sm.as_codegen().is_none());
        assert!(sm.as_sim_model().is_none());
    }
}
