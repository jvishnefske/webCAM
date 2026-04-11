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
//! Input ports: `state_in` (Float), then guard ports (`guard_0`, `guard_1`, ..., Float), then one per input_topic (Message).
//! Output ports: `next_state` (Float), `active_<name>` per state (Float), then one per output_topic (Message).

use module_traits::{
    Codegen, MessageData, MessageSchema, Module, PortDef, PortKind, Tick, Value,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
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
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TransitionConfig {
    pub from: String,
    pub to: String,
    pub guard: TransitionGuard,
    #[serde(default)]
    pub actions: Vec<TransitionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
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
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FieldCondition {
    pub field: String,
    pub op: CompareOp,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
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
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TransitionAction {
    pub topic: String,
    pub message: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TopicBinding {
    pub topic: String,
    #[cfg_attr(feature = "tsify", tsify(type = "{ name: string; fields: Array<{ name: string; field_type: string }> }"))]
    pub schema: MessageSchema,
}

// ---------------------------------------------------------------------------
// Block implementation
// ---------------------------------------------------------------------------

pub struct StateMachineBlock {
    pub(crate) config: StateMachineConfig,
}

impl StateMachineBlock {
    pub fn from_config(config: StateMachineConfig) -> Self {
        Self { config }
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
        let mut ports = vec![PortDef::new("state_in", PortKind::Float)];
        for i in 0..self.n_guard_ports() {
            ports.push(PortDef::new(&format!("guard_{i}"), PortKind::Float));
        }
        for binding in &self.config.input_topics {
            ports.push(PortDef::new(
                &binding.topic,
                PortKind::Message(binding.schema.clone()),
            ));
        }
        ports
    }

    fn output_ports(&self) -> Vec<PortDef> {
        let mut ports = vec![PortDef::new("next_state", PortKind::Float)];
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

    fn as_codegen(&self) -> Option<&dyn Codegen> {
        Some(self)
    }
}

impl Tick for StateMachineBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Read state_in from inputs[0], clamp to valid range.
        let n_states = self.config.states.len();
        let state_in = inputs
            .first()
            .and_then(|v| v.as_ref())
            .and_then(|v| v.as_float())
            .unwrap_or(0.0);
        let current_state = (state_in as usize).min(n_states.saturating_sub(1));
        let current_name = self.config.states[current_state].clone();

        // Guard and topic inputs start at inputs[1..].
        let guard_and_topic_inputs = if inputs.len() > 1 {
            &inputs[1..]
        } else {
            &[]
        };

        // Evaluate transitions from current state (first match wins).
        let mut next_state = current_state;
        let mut fired_actions: Vec<TransitionAction> = Vec::new();
        for t in &self.config.transitions {
            if t.from != current_name {
                continue;
            }
            if self.check_guard(&t.guard, guard_and_topic_inputs) {
                if let Some(idx) = self.config.states.iter().position(|s| s == &t.to) {
                    next_state = idx;
                    fired_actions = t.actions.clone();
                    break;
                }
            }
        }

        // Build outputs: next_state index, active flags, then output topic messages.
        let mut outputs: Vec<Option<Value>> =
            Vec::with_capacity(1 + n_states + self.config.output_topics.len());

        outputs.push(Some(Value::Float(next_state as f64)));

        for i in 0..n_states {
            outputs.push(Some(Value::Float(if i == next_state {
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

impl Codegen for StateMachineBlock {
    fn emit_rust(&self, _target: &str) -> Result<String, String> {
        let states = &self.config.states;
        let n_guard = self.n_guard_ports();

        // Build parameter list: state_in, guard_0..guard_N, topic inputs
        let mut params = vec!["state_in: f64".to_string()];
        for i in 0..n_guard {
            params.push(format!("guard_{i}: f64"));
        }
        for binding in &self.config.input_topics {
            // Topic inputs represented as f64 for codegen (simplified)
            params.push(format!("{}: f64", binding.topic));
        }
        let params_str = params.join(", ");

        // Build return tuple type: (next_state, active_0, active_1, ...)
        let n_outputs = 1 + states.len();
        let ret_types: Vec<&str> = (0..n_outputs).map(|_| "f64").collect();
        let ret_str = ret_types.join(", ");

        let mut code = String::new();
        code.push_str(&format!(
            "pub fn block_state_machine({params_str}) -> ({ret_str}) {{\n"
        ));
        code.push_str("    let state_idx = state_in as u8;\n");
        code.push_str("    let next_state = match state_idx {\n");

        // For each state, emit match arm with transition logic
        for (si, state_name) in states.iter().enumerate() {
            code.push_str(&format!("        {si} => {{\n"));

            // Find transitions from this state
            let transitions: Vec<&TransitionConfig> = self
                .config
                .transitions
                .iter()
                .filter(|t| t.from == *state_name)
                .collect();

            let mut first = true;
            for t in &transitions {
                let target_idx = states
                    .iter()
                    .position(|s| s == &t.to)
                    .unwrap_or(si);

                match &t.guard {
                    TransitionGuard::GuardPort { port } => {
                        let kw = if first { "if" } else { "else if" };
                        code.push_str(&format!(
                            "            {kw} guard_{port} > 0.5 {{ {target_idx} }}\n"
                        ));
                        first = false;
                    }
                    TransitionGuard::Topic { topic, condition } => {
                        let kw = if first { "if" } else { "else if" };
                        match condition {
                            Some(cond) => {
                                let op_str = match cond.op {
                                    CompareOp::Eq => "==",
                                    CompareOp::Ne => "!=",
                                    CompareOp::Gt => ">",
                                    CompareOp::Lt => "<",
                                    CompareOp::Ge => ">=",
                                    CompareOp::Le => "<=",
                                };
                                code.push_str(&format!(
                                    "            {kw} {topic} {op_str} {:?} {{ {target_idx} }}\n",
                                    cond.value
                                ));
                            }
                            None => {
                                // Topic present is sufficient (simplified: non-zero check)
                                code.push_str(&format!(
                                    "            {kw} {topic} != 0.0 {{ {target_idx} }}\n"
                                ));
                            }
                        }
                        first = false;
                    }
                    TransitionGuard::Unconditional => {
                        if first {
                            code.push_str(&format!(
                                "            {target_idx}\n"
                            ));
                        } else {
                            code.push_str(&format!(
                                "            else {{ {target_idx} }}\n"
                            ));
                        }
                        first = false;
                    }
                }
            }

            if transitions.is_empty() || transitions.iter().all(|t| !matches!(t.guard, TransitionGuard::Unconditional)) {
                if !first {
                    code.push_str(&format!("            else {{ {si} }}\n"));
                } else {
                    code.push_str(&format!("            {si}\n"));
                }
            }

            code.push_str("        }\n");
        }

        code.push_str("        _ => state_idx as usize,\n    };\n");

        // Emit active flags
        let mut tuple_parts = vec!["next_state as f64".to_string()];
        for (i, _) in states.iter().enumerate() {
            tuple_parts.push(format!(
                "if next_state == {i} {{ 1.0 }} else {{ 0.0 }}"
            ));
        }
        code.push_str(&format!("    ({})\n", tuple_parts.join(", ")));
        code.push_str("}\n");

        Ok(code)
    }
}

pub(crate) fn register(reg: &mut Vec<crate::registry::BlockRegistration>) {
    reg.push(crate::registry::BlockRegistration {
        block_type: "state_machine",
        display_name: "State Machine",
        category: "Logic",
        create_from_json: |json| {
            let cfg: StateMachineConfig =
                serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(StateMachineBlock::from_config(cfg)))
        },
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use module_traits::{FieldType, MessageField};

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
        let state_in = Value::Float(0.0); // idle
        // inputs: [state_in, guard_0, guard_1]
        let result = sm.tick(&[Some(&state_in)], 0.01);
        // next_state=0 (idle), active_idle=1, active_running=0, active_error=0
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
        let state_idle = Value::Float(0.0);
        let state_running = Value::Float(1.0);

        // state_in=0 (idle), guard_0=high, guard_1=low -> idle->running
        let result = sm.tick(&[Some(&state_idle), Some(&high), Some(&low)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
        assert_eq!(result[2], Some(Value::Float(1.0))); // active_running

        // state_in=1 (running), guard_0=low, guard_1=high -> running->error
        let result = sm.tick(&[Some(&state_running), Some(&low), Some(&high)], 0.01);
        assert_eq!(result[0], Some(Value::Float(2.0))); // error
        assert_eq!(result[3], Some(Value::Float(1.0))); // active_error
    }

    #[test]
    fn legacy_unconditional_transition() {
        let mut sm = make_legacy_sm();
        let state_error = Value::Float(2.0);
        // state_in=2 (error), error->idle is unconditional
        let result = sm.tick(&[Some(&state_error)], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // back to idle
    }

    #[test]
    fn topic_input_ports() {
        let sm = make_topic_sm();
        let ports = sm.input_ports();
        // state_in + no guard ports + 1 topic input = 2
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].name, "state_in");
        assert_eq!(ports[0].kind, PortKind::Float);
        assert_eq!(ports[1].name, "motor_cmd");
        match &ports[1].kind {
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
        // next_state + active_idle + active_running + motor_status
        assert_eq!(ports.len(), 4);
        assert_eq!(ports[0].name, "next_state");
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
        let state_idle = Value::Float(0.0);
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        // inputs: [state_in=0, motor_cmd msg]
        let result = sm.tick(&[Some(&state_idle), Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
        assert_eq!(result[1], Some(Value::Float(0.0))); // active_idle=0
        assert_eq!(result[2], Some(Value::Float(1.0))); // active_running=1
    }

    #[test]
    fn topic_guard_condition_not_met() {
        let mut sm = make_topic_sm();
        let state_idle = Value::Float(0.0);
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 0.0)],
        });
        // speed=0.0, condition is Gt 0.0 -> fails
        let result = sm.tick(&[Some(&state_idle), Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // stays idle
        assert_eq!(result[1], Some(Value::Float(1.0))); // active_idle=1
    }

    #[test]
    fn topic_guard_no_message() {
        let mut sm = make_topic_sm();
        let state_idle = Value::Float(0.0);
        // state_in=0, topic input=None -> guard not satisfied
        let result = sm.tick(&[Some(&state_idle), None], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // stays idle
    }

    #[test]
    fn topic_action_emits_message() {
        let mut sm = make_topic_sm();
        let state_idle = Value::Float(0.0);
        let msg = Value::Message(MessageData {
            schema_name: "MotorCmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        let result = sm.tick(&[Some(&state_idle), Some(&msg)], 0.01);
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
        assert!(sm.as_codegen().is_some());
        assert!(sm.as_sim_model().is_none());
    }

    #[test]
    fn codegen_emits_valid_rust() {
        let sm = make_legacy_sm();
        let cg = sm.as_codegen().expect("should have codegen");
        let code = cg.emit_rust("host").expect("emit_rust should succeed");
        // Should contain function signature
        assert!(code.contains("pub fn block_state_machine("));
        assert!(code.contains("state_in: f64"));
        assert!(code.contains("guard_0: f64"));
        assert!(code.contains("guard_1: f64"));
        // Should contain match on state_idx
        assert!(code.contains("let state_idx = state_in as u8;"));
        assert!(code.contains("let next_state = match state_idx"));
        // Should contain guard checks
        assert!(code.contains("guard_0 > 0.5"));
        assert!(code.contains("guard_1 > 0.5"));
        // Should contain active flag outputs
        assert!(code.contains("next_state as f64"));
    }

    #[test]
    fn codegen_topic_sm() {
        let sm = make_topic_sm();
        let cg = sm.as_codegen().expect("should have codegen");
        let code = cg.emit_rust("host").expect("emit_rust should succeed");
        assert!(code.contains("pub fn block_state_machine("));
        assert!(code.contains("state_in: f64"));
        assert!(code.contains("motor_cmd: f64"));
        // Should contain topic condition check
        assert!(code.contains("motor_cmd >"));
    }
}
