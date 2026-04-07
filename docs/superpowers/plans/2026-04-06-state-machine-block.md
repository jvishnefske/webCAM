# State Machine Block Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the state machine block into a deployable event-driven FSM with typed pubsub topic bindings, a structured form editor, and MLIR codegen.

**Architecture:** Bottom-up — add Message types to module-traits, refactor the Rust block with topic-driven transitions, update MLIR codegen and runtime, then build the form editor UI. The block remains a pure computation node; pubsub transport is handled by wiring Message-typed ports to PubSubSource/Sink blocks.

**Tech Stack:** Rust (no_std module-traits, std blocks/codegen), TypeScript (form editor), WASM (wasm-bindgen bridge), MLIR (text emission)

**Spec:** `docs/superpowers/specs/2026-04-06-state-machine-block-design.md`

---

## File Structure

### New files:
| File | Responsibility |
|------|---------------|
| `www/src/dataflow/state-machine-editor.ts` | Form editor component: states, topics, transitions tables |

### Modified files:
| File | Changes |
|------|---------|
| `module-traits/src/value.rs` | Add `FieldType`, `MessageField`, `MessageSchema`, `MessageData`, `PortKind::Message`, `Value::Message` |
| `module-traits/src/lib.rs` | Re-export new types |
| `src/dataflow/block.rs` | Re-export new types |
| `src/dataflow/blocks/state_machine.rs` | New config model (topic bindings, TransitionGuard enum), topic-driven tick |
| `src/dataflow/blocks/mod.rs` | Update `create_block` for `#[serde(default)]` on StateMachineConfig |
| `www/src/dataflow/types.ts` | TS mirror types for Message, schema, state machine config |
| `www/src/dataflow/node-view.ts` | Mount state machine editor when SM block is selected |
| `mlir-codegen/src/ir.rs` | Add `DataflowOp::ChannelRead`, `ChannelWrite`, `MessageFieldExtract`, `StateMachine` |
| `mlir-codegen/src/state_machine.rs` | Emit typed channel reads, message field extraction in guards |
| `mlir-codegen/src/lower.rs` | Wire up updated state_machine emitter |
| `mlir-codegen/src/runtime.rs` | Extend `BlockFn::StateMachine` with topic-based transitions |
| `src/dataflow/codegen/emit.rs` | Update legacy Rust codegen for topic-driven FSM |

---

### Task 1: Add Message types to module-traits

**Files:**
- Modify: `module-traits/src/value.rs`
- Modify: `module-traits/src/lib.rs`
- Modify: `src/dataflow/block.rs`

- [ ] **Step 1: Write tests for FieldType, MessageField, MessageSchema**

Add to the bottom of `module-traits/src/value.rs` inside `mod tests`:

```rust
    // -- Message types --

    #[test]
    fn test_field_type_serde_roundtrip() {
        let types = vec![
            FieldType::F32, FieldType::F64, FieldType::U8,
            FieldType::U16, FieldType::U32, FieldType::I32, FieldType::Bool,
        ];
        for ft in &types {
            let json = serde_json::to_string(ft).expect("serialize FieldType");
            let restored: FieldType = serde_json::from_str(&json).expect("deserialize FieldType");
            assert_eq!(ft, &restored);
        }
    }

    #[test]
    fn test_message_schema_construction() {
        let schema = MessageSchema {
            name: String::from("motor_cmd"),
            fields: vec![
                MessageField { name: String::from("speed"), field_type: FieldType::F32 },
                MessageField { name: String::from("dir"), field_type: FieldType::Bool },
            ],
        };
        assert_eq!(schema.name, "motor_cmd");
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.fields[0].name, "speed");
        assert_eq!(schema.fields[1].field_type, FieldType::Bool);
    }

    #[test]
    fn test_message_schema_serde_roundtrip() {
        let schema = MessageSchema {
            name: String::from("sensor"),
            fields: vec![
                MessageField { name: String::from("temp"), field_type: FieldType::F32 },
            ],
        };
        let json = serde_json::to_string(&schema).expect("serialize");
        let restored: MessageSchema = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(schema, restored);
    }

    #[test]
    fn test_port_kind_message_serde_roundtrip() {
        let schema = MessageSchema {
            name: String::from("cmd"),
            fields: vec![
                MessageField { name: String::from("val"), field_type: FieldType::F64 },
            ],
        };
        let kind = PortKind::Message(schema);
        let json = serde_json::to_string(&kind).expect("serialize");
        let restored: PortKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(kind, restored);
    }

    #[test]
    fn test_message_data_construction() {
        let msg = MessageData {
            schema_name: String::from("motor_cmd"),
            fields: vec![
                (String::from("speed"), 1.5),
                (String::from("dir"), 1.0),
            ],
        };
        assert_eq!(msg.schema_name, "motor_cmd");
        assert_eq!(msg.fields.len(), 2);
        assert_eq!(msg.fields[0].1, 1.5);
    }

    #[test]
    fn test_value_message_serde_roundtrip() {
        let msg = Value::Message(MessageData {
            schema_name: String::from("s"),
            fields: vec![(String::from("x"), 42.0)],
        });
        let json = serde_json::to_string(&msg).expect("serialize");
        let restored: Value = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, restored);
    }

    #[test]
    fn test_value_message_kind() {
        let msg = Value::Message(MessageData {
            schema_name: String::from("s"),
            fields: vec![],
        });
        // Message kind returns a Message variant with the schema_name
        match msg.kind() {
            PortKind::Message(s) => assert_eq!(s.name, "s"),
            other => panic!("expected PortKind::Message, got {:?}", other),
        }
    }

    #[test]
    fn test_value_as_message() {
        let data = MessageData {
            schema_name: String::from("t"),
            fields: vec![(String::from("a"), 1.0)],
        };
        let val = Value::Message(data.clone());
        assert_eq!(val.as_message(), Some(&data));
        assert_eq!(Value::Float(1.0).as_message(), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p module-traits`
Expected: Compilation errors -- `FieldType`, `MessageField`, `MessageSchema`, `MessageData`, `PortKind::Message`, `Value::Message`, `as_message()` not defined.

- [ ] **Step 3: Implement Message types**

In `module-traits/src/value.rs`, add these types *before* the `PortKind` enum:

```rust
/// Primitive field types for message schemas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    F32,
    F64,
    U8,
    U16,
    U32,
    I32,
    Bool,
}

/// A named field in a message schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageField {
    pub name: String,
    pub field_type: FieldType,
}

/// Schema definition for a structured message type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageSchema {
    pub name: String,
    pub fields: Vec<MessageField>,
}

/// Runtime message data: flat f64 fields (bools as 0.0/1.0, ints cast to f64).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageData {
    pub schema_name: String,
    pub fields: Vec<(String, f64)>,
}
```

Add `Message` variant to `PortKind`:

```rust
pub enum PortKind {
    Float,
    Bytes,
    Text,
    Series,
    Any,
    Message(MessageSchema),
}
```

Add `Message` variant to `Value`:

```rust
pub enum Value {
    Float(f64),
    Bytes(Vec<u8>),
    Text(String),
    Series(Vec<f64>),
    Message(MessageData),
}
```

Add `as_message()` method to `impl Value`:

```rust
    pub fn as_message(&self) -> Option<&MessageData> {
        match self {
            Value::Message(m) => Some(m),
            _ => None,
        }
    }
```

Update `Value::kind()` to handle `Message`:

```rust
    pub fn kind(&self) -> PortKind {
        match self {
            Value::Float(_) => PortKind::Float,
            Value::Bytes(_) => PortKind::Bytes,
            Value::Text(_) => PortKind::Text,
            Value::Series(_) => PortKind::Series,
            Value::Message(m) => PortKind::Message(MessageSchema {
                name: m.schema_name.clone(),
                fields: Vec::new(), // runtime data does not carry field types
            }),
        }
    }
```

- [ ] **Step 4: Update re-exports**

In `module-traits/src/lib.rs`, change line 32:

```rust
pub use value::{FieldType, MessageData, MessageField, MessageSchema, PortDef, PortKind, Value};
```

In `src/dataflow/block.rs`, change lines 9-12:

```rust
pub use module_traits::{
    AnalysisMetadata, AnalysisModel, Codegen, FieldType, MessageData, MessageField, MessageSchema,
    Module, PortDef, PortKind, SimModel, SimPeripherals, Tick, Value,
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p module-traits`
Expected: All tests pass, including existing ones (serde roundtrips for existing PortKind variants must still work).

- [ ] **Step 6: Commit**

```bash
git add module-traits/src/value.rs module-traits/src/lib.rs src/dataflow/block.rs
git commit -m "feat: add PortKind::Message, Value::Message, and MessageSchema types"
```

---

### Task 2: Refactor StateMachineConfig with topic bindings

**Files:**
- Modify: `src/dataflow/blocks/state_machine.rs`

- [ ] **Step 1: Write tests for the new config model**

Replace the entire `#[cfg(test)] mod tests` block in `src/dataflow/blocks/state_machine.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_topic_sm() -> StateMachineBlock {
        StateMachineBlock::from_config(StateMachineConfig {
            states: vec!["idle".to_string(), "running".to_string()],
            initial: "idle".to_string(),
            transitions: vec![
                TransitionConfig {
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
                        topic: "status".to_string(),
                        message: vec![("running".to_string(), 1.0)],
                    }],
                },
            ],
            input_topics: vec![TopicBinding {
                topic: "motor_cmd".to_string(),
                schema: MessageSchema {
                    name: "motor_cmd".to_string(),
                    fields: vec![
                        MessageField { name: "speed".to_string(), field_type: FieldType::F32 },
                    ],
                },
            }],
            output_topics: vec![TopicBinding {
                topic: "status".to_string(),
                schema: MessageSchema {
                    name: "status".to_string(),
                    fields: vec![
                        MessageField { name: "running".to_string(), field_type: FieldType::Bool },
                    ],
                },
            }],
        })
    }

    #[test]
    fn legacy_initial_state() {
        let mut sm = make_legacy_sm();
        let result = sm.tick(&[], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0)));
        assert_eq!(result[1], Some(Value::Float(1.0))); // active_idle
        assert_eq!(result[2], Some(Value::Float(0.0)));
        assert_eq!(result[3], Some(Value::Float(0.0)));
    }

    #[test]
    fn legacy_guard_transition() {
        let mut sm = make_legacy_sm();
        let high = Value::Float(1.0);
        let low = Value::Float(0.0);
        let result = sm.tick(&[Some(&high), Some(&low)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
    }

    #[test]
    fn legacy_unconditional_transition() {
        let mut sm = make_legacy_sm();
        sm.current_state = 2; // error
        let result = sm.tick(&[], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // back to idle
    }

    #[test]
    fn topic_input_ports() {
        let sm = make_topic_sm();
        let inputs = sm.input_ports();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "motor_cmd");
        match &inputs[0].kind {
            PortKind::Message(schema) => {
                assert_eq!(schema.name, "motor_cmd");
                assert_eq!(schema.fields.len(), 1);
            }
            other => panic!("expected PortKind::Message, got {:?}", other),
        }
    }

    #[test]
    fn topic_output_ports() {
        let sm = make_topic_sm();
        let outputs = sm.output_ports();
        // state + active_idle + active_running + status topic
        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].name, "state");
        assert_eq!(outputs[1].name, "active_idle");
        assert_eq!(outputs[2].name, "active_running");
        assert_eq!(outputs[3].name, "status");
        match &outputs[3].kind {
            PortKind::Message(schema) => assert_eq!(schema.name, "status"),
            other => panic!("expected PortKind::Message, got {:?}", other),
        }
    }

    #[test]
    fn topic_guard_transition_with_message() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "motor_cmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        let result = sm.tick(&[Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
    }

    #[test]
    fn topic_guard_condition_not_met() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "motor_cmd".to_string(),
            fields: vec![("speed".to_string(), 0.0)], // not > 0
        });
        let result = sm.tick(&[Some(&msg)], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // still idle
    }

    #[test]
    fn topic_guard_no_message() {
        let mut sm = make_topic_sm();
        let result = sm.tick(&[None], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // still idle
    }

    #[test]
    fn topic_action_emits_message() {
        let mut sm = make_topic_sm();
        let msg = Value::Message(MessageData {
            schema_name: "motor_cmd".to_string(),
            fields: vec![("speed".to_string(), 1.5)],
        });
        let result = sm.tick(&[Some(&msg)], 0.01);
        // Output port 3 is the status topic
        let action_msg = result[3].as_ref().expect("action should emit message");
        match action_msg {
            Value::Message(data) => {
                assert_eq!(data.schema_name, "status");
                assert_eq!(data.fields[0], ("running".to_string(), 1.0));
            }
            other => panic!("expected Value::Message, got {:?}", other),
        }
    }

    #[test]
    fn config_default() {
        let cfg = StateMachineConfig::default();
        assert_eq!(cfg.states, vec!["idle"]);
        assert_eq!(cfg.initial, "idle");
        assert!(cfg.transitions.is_empty());
        assert!(cfg.input_topics.is_empty());
        assert!(cfg.output_topics.is_empty());
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = make_topic_sm().config;
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: StateMachineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.states, restored.states);
        assert_eq!(cfg.initial, restored.initial);
        assert_eq!(cfg.transitions.len(), restored.transitions.len());
    }

    #[test]
    fn config_deserialize_minimal() {
        // Minimal JSON -- only required fields
        let json = r#"{"states":["a"],"initial":"a"}"#;
        let cfg: StateMachineConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.states, vec!["a"]);
        assert!(cfg.transitions.is_empty());
        assert!(cfg.input_topics.is_empty());
    }

    #[test]
    fn module_trait_methods() {
        let mut sm = make_legacy_sm();
        assert_eq!(sm.name(), "State Machine");
        assert_eq!(sm.block_type(), "state_machine");
        assert!(sm.as_tick().is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustcam -- state_machine`
Expected: Compilation errors -- `TransitionConfig`, `TransitionGuard`, `TopicBinding`, etc. not defined.

- [ ] **Step 3: Implement the new config model and tick logic**

Replace the contents of `src/dataflow/blocks/state_machine.rs` (keeping the test module from step 1) with the full implementation. The key types are:

- `StateMachineConfig` with `states`, `initial`, `transitions`, `input_topics`, `output_topics`
- `TransitionConfig` with `from`, `to`, `guard` (TransitionGuard enum), `actions`
- `TransitionGuard` enum: `Topic { topic, condition }`, `Unconditional`, `GuardPort { port }`
- `FieldCondition` with `field`, `op` (CompareOp enum), `value`
- `TransitionAction` with `topic`, `message` (Vec of field name/value pairs)
- `TopicBinding` with `topic`, `schema` (MessageSchema)

The `StateMachineBlock` struct holds `config` and `current_state`. Its `input_ports()` returns guard ports first, then topic ports. Its `output_ports()` returns state index, active flags, then topic output ports.

The `tick()` method:
1. Finds transitions from current state
2. Evaluates guards (guard port threshold, topic message + optional field condition, or unconditional)
3. First match wins, updates current_state, collects fired actions
4. Emits: state index float, active flags, and Message values for fired actions

Full implementation:

```rust
//! State machine block: configurable finite state machine with typed pubsub topics.
//!
//! Supports three guard types:
//! - `Topic`: fires when a Message arrives on a subscribed topic, with optional field condition
//! - `GuardPort`: legacy float > 0.5 threshold guard
//! - `Unconditional`: always fires (epsilon transition)

use crate::dataflow::block::{
    FieldType, MessageData, MessageField, MessageSchema, Module, PortDef, PortKind, Tick, Value,
};
use serde::{Deserialize, Serialize};

// -- Config types --

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

// -- Block --

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

    /// Number of legacy guard ports needed (max guard_port index + 1).
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

    /// Find the input port index for a given topic name.
    fn topic_input_index(&self, topic: &str) -> Option<usize> {
        let guard_offset = self.n_guard_ports();
        self.config
            .input_topics
            .iter()
            .position(|t| t.topic == topic)
            .map(|i| guard_offset + i)
    }

    fn check_guard(&self, guard: &TransitionGuard, inputs: &[Option<&Value>]) -> bool {
        match guard {
            TransitionGuard::Unconditional => true,
            TransitionGuard::GuardPort { port } => {
                inputs
                    .get(*port)
                    .and_then(|v| v.as_ref())
                    .and_then(|v| v.as_float())
                    .unwrap_or(0.0)
                    > 0.5
            }
            TransitionGuard::Topic { topic, condition } => {
                let Some(port_idx) = self.topic_input_index(topic) else {
                    return false;
                };
                let Some(Some(value)) = inputs.get(port_idx) else {
                    return false;
                };
                match condition {
                    None => true, // message arrived, no field condition
                    Some(cond) => {
                        let Some(msg) = value.as_message() else {
                            return false;
                        };
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
        let mut ports = Vec::new();
        // Legacy guard ports
        for i in 0..self.n_guard_ports() {
            ports.push(PortDef::new(&format!("guard_{i}"), PortKind::Float));
        }
        // Topic input ports
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
        // Topic output ports
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

        // Evaluate transitions from current state
        let mut fired_actions: Vec<&TransitionAction> = Vec::new();
        for t in &self.config.transitions {
            if t.from != current_name {
                continue;
            }
            if self.check_guard(&t.guard, inputs) {
                if let Some(idx) = self.config.states.iter().position(|s| s == &t.to) {
                    self.current_state = idx;
                    fired_actions = t.actions.iter().collect();
                    break;
                }
            }
        }

        // Build outputs
        let n_outputs = 1 + self.config.states.len() + self.config.output_topics.len();
        let mut outputs: Vec<Option<Value>> = Vec::with_capacity(n_outputs);

        // State index
        outputs.push(Some(Value::Float(self.current_state as f64)));

        // Active flags
        for (i, _) in self.config.states.iter().enumerate() {
            outputs.push(Some(Value::Float(if i == self.current_state {
                1.0
            } else {
                0.0
            })));
        }

        // Output topic messages (None unless an action fires for that topic)
        for binding in &self.config.output_topics {
            let msg = fired_actions
                .iter()
                .find(|a| a.topic == binding.topic)
                .map(|action| {
                    Value::Message(MessageData {
                        schema_name: binding.schema.name.clone(),
                        fields: action.message.clone(),
                    })
                });
            outputs.push(msg);
        }

        outputs
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rustcam -- state_machine`
Expected: All state_machine tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/dataflow/blocks/state_machine.rs
git commit -m "feat: refactor state machine with topic bindings and field conditions"
```

---

### Task 3: Update create_block and ensure WASM pipeline works

**Files:**
- Modify: `src/dataflow/blocks/mod.rs`

- [ ] **Step 1: Write tests for new config deserialization paths**

Add to the existing `mod tests` in `src/dataflow/blocks/mod.rs`:

```rust
    #[test]
    fn create_block_state_machine_minimal() {
        // Minimal config with only required fields (serde defaults handle the rest)
        let block = create_block("state_machine", r#"{"states":["idle"],"initial":"idle"}"#).unwrap();
        assert_eq!(block.block_type(), "state_machine");
    }

    #[test]
    fn create_block_state_machine_with_topics() {
        let cfg = r#"{
            "states": ["idle", "running"],
            "initial": "idle",
            "transitions": [{
                "from": "idle",
                "to": "running",
                "guard": {"type": "Topic", "topic": "cmd", "condition": {"field": "go", "op": "Gt", "value": 0.0}},
                "actions": []
            }],
            "input_topics": [{"topic": "cmd", "schema": {"name": "cmd", "fields": [{"name": "go", "field_type": "F32"}]}}],
            "output_topics": []
        }"#;
        let block = create_block("state_machine", cfg).unwrap();
        assert_eq!(block.block_type(), "state_machine");
        assert_eq!(block.input_ports().len(), 1); // one topic port
        assert_eq!(block.input_ports()[0].name, "cmd");
    }

    #[test]
    fn create_block_state_machine_legacy_guard() {
        let cfg = r#"{
            "states": ["a", "b"],
            "initial": "a",
            "transitions": [{"from": "a", "to": "b", "guard": {"type": "GuardPort", "port": 0}}]
        }"#;
        let block = create_block("state_machine", cfg).unwrap();
        assert_eq!(block.input_ports().len(), 1);
        assert_eq!(block.input_ports()[0].name, "guard_0");
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rustcam -- create_block_state_machine`
Expected: All pass -- the `create_block` match arm already deserializes `StateMachineConfig` and the new config is backward-compatible via `#[serde(default)]`.

- [ ] **Step 3: Commit**

```bash
git add src/dataflow/blocks/mod.rs
git commit -m "test: add create_block tests for topic-driven state machine config"
```

---

### Task 4: Add TypeScript mirror types

**Files:**
- Modify: `www/src/dataflow/types.ts`

- [ ] **Step 1: Add Message and StateMachine config types**

Append to `www/src/dataflow/types.ts`:

```typescript
// -- Message types --

export type FieldType = 'F32' | 'F64' | 'U8' | 'U16' | 'U32' | 'I32' | 'Bool';

export interface MessageField {
  name: string;
  field_type: FieldType;
}

export interface MessageSchema {
  name: string;
  fields: MessageField[];
}

export interface MessageData {
  schema_name: string;
  fields: [string, number][];
}

export interface ValueMessage { type: 'Message'; data: MessageData }

// -- State machine config --

export interface TopicBinding {
  topic: string;
  schema: MessageSchema;
}

export interface FieldCondition {
  field: string;
  op: 'Eq' | 'Ne' | 'Gt' | 'Lt' | 'Ge' | 'Le';
  value: number;
}

export type TransitionGuard =
  | { type: 'Topic'; topic: string; condition?: FieldCondition }
  | { type: 'Unconditional' }
  | { type: 'GuardPort'; port: number };

export interface TransitionAction {
  topic: string;
  message: [string, number][];
}

export interface TransitionConfig {
  from: string;
  to: string;
  guard: TransitionGuard;
  actions: TransitionAction[];
}

export interface StateMachineConfig {
  states: string[];
  initial: string;
  transitions: TransitionConfig[];
  input_topics: TopicBinding[];
  output_topics: TopicBinding[];
}
```

Also update the `Value` union type (line 12) to include `ValueMessage`:

```typescript
export type Value = ValueFloat | ValueBytes | ValueText | ValueSeries | ValueMessage;
```

- [ ] **Step 2: Verify the frontend builds**

Run: `cd www && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add www/src/dataflow/types.ts
git commit -m "feat: add TypeScript mirror types for Message and StateMachineConfig"
```

---

### Task 5: Build the state machine form editor

**Files:**
- Create: `www/src/dataflow/state-machine-editor.ts`
- Modify: `www/src/dataflow/node-view.ts`

- [ ] **Step 1: Create the form editor component**

Create `www/src/dataflow/state-machine-editor.ts` with a `mountStateMachineEditor` function that takes a container element, block ID, config, DataflowManager, and onChange callback.

The editor has four sections:
1. **States** -- text inputs for state names, radio for initial, delete buttons
2. **Input Topics** -- topic name, inline field editor (name + type dropdown), delete
3. **Output Topics** -- same structure as input topics
4. **Transitions** -- from/to dropdowns (populated from states), guard type selector (Unconditional/Topic), topic dropdown (from input topics), optional field condition (field dropdown, op dropdown, value input), delete

Each change calls `mgr.updateBlock(blockId, 'state_machine', cfg)` and the onChange callback.

See spec section 4 for the full table layouts. The component uses plain DOM (no framework), consistent with the rest of the editor codebase.

- [ ] **Step 2: Mount the editor in node-view.ts**

In `www/src/dataflow/node-view.ts`, add import:

```typescript
import { mountStateMachineEditor } from './state-machine-editor.js';
```

In `reconcileNodes`, after `createPorts(...)` (line 51), add a container div for state machine blocks:

```typescript
      if (block.block_type === 'state_machine') {
        const editorDiv = document.createElement('div');
        editorDiv.className = 'sm-editor-container';
        nodeEl.appendChild(editorDiv);
      }
```

Add a new exported function `updateStateMachineEditor` that:
- Takes `NodeElements`, `selectedId`, snapshot, mgr, onChange callback
- Clears all `.sm-editor-container` elements
- If selected block is a state_machine, mounts the editor into its container

```typescript
export function updateStateMachineEditor(
  elements: NodeElements,
  selectedId: number | null,
  snap: { blocks: import('./types.js').BlockSnapshot[] } | null,
  mgr: DataflowManager,
  onConfigChanged: () => void,
): void {
  for (const [, nodeEl] of elements.nodes) {
    const container = nodeEl.querySelector('.sm-editor-container');
    if (container) container.textContent = '';
  }
  if (selectedId === null || !snap) return;
  const block = snap.blocks.find(b => b.id === selectedId);
  if (!block || block.block_type !== 'state_machine') return;
  const nodeEl = elements.nodes.get(selectedId);
  if (!nodeEl) return;
  const container = nodeEl.querySelector('.sm-editor-container') as HTMLElement | null;
  if (!container) return;
  mountStateMachineEditor(
    container,
    selectedId,
    block.config as unknown as import('./types.js').StateMachineConfig,
    mgr,
    onConfigChanged,
  );
}
```

- [ ] **Step 3: Verify the frontend builds**

Run: `cd www && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add www/src/dataflow/state-machine-editor.ts www/src/dataflow/node-view.ts
git commit -m "feat: add state machine form editor with states/topics/transitions"
```

---

### Task 6: Add MLIR DataflowOp variants for typed channels

**Files:**
- Modify: `mlir-codegen/src/ir.rs`

- [ ] **Step 1: Add new DataflowOp variants**

In `mlir-codegen/src/ir.rs`, add to the `DataflowOp` enum (after `EncoderRead`, before the closing `}`):

```rust
    /// `dataflow.channel_read "topic"` -- read from a named typed channel (subscribe).
    ChannelRead,
    /// `dataflow.channel_write "topic"` -- write to a named typed channel (publish).
    ChannelWrite,
    /// `dataflow.message_field "name"` -- extract a typed field from a message value.
    MessageFieldExtract,
    /// `dataflow.state_machine` -- region-based FSM op.
    StateMachine,
```

Update the `mlir_name()` match arm in the `DataflowOp` branch to include the new variants:

```rust
                DataflowOp::ChannelRead => "channel_read",
                DataflowOp::ChannelWrite => "channel_write",
                DataflowOp::MessageFieldExtract => "message_field",
                DataflowOp::StateMachine => "state_machine",
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mlir-codegen`
Expected: All existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add mlir-codegen/src/ir.rs
git commit -m "feat: add MLIR DataflowOp variants for typed channels and state machine"
```

---

### Task 7: Update MLIR state machine emission for typed channels

**Files:**
- Modify: `mlir-codegen/src/state_machine.rs`

- [ ] **Step 1: Write test for topic-driven state machine MLIR emission**

Add to the existing `mod tests` in `mlir-codegen/src/state_machine.rs`:

```rust
    #[test]
    fn emit_sm_with_topic_guards() {
        let block = BlockSnapshot {
            id: 20,
            block_type: "state_machine".to_string(),
            name: "topic_fsm".to_string(),
            inputs: vec![PortDef {
                name: "motor_cmd".to_string(),
                kind: PortKind::Float,
            }],
            outputs: vec![
                PortDef { name: "state".to_string(), kind: PortKind::Float },
                PortDef { name: "active_idle".to_string(), kind: PortKind::Float },
                PortDef { name: "active_running".to_string(), kind: PortKind::Float },
            ],
            config: serde_json::json!({
                "states": ["idle", "running"],
                "initial": "idle",
                "transitions": [{
                    "from": "idle",
                    "to": "running",
                    "guard": {
                        "type": "Topic",
                        "topic": "motor_cmd",
                        "condition": {"field": "speed", "op": "Gt", "value": 0.0}
                    },
                    "actions": []
                }],
                "input_topics": [{"topic": "motor_cmd", "schema": {"name": "motor_cmd", "fields": [{"name": "speed", "field_type": "F32"}]}}],
                "output_topics": []
            }),
            output_values: vec![],
            custom_codegen: None,
        };
        let inputs = vec!["%motor_cmd".to_string()];
        let mut out = String::new();
        emit_state_machine_tick(&mut out, 20, &block, &inputs).unwrap();
        assert!(out.contains("dataflow.state_machine"), "should contain state_machine op");
        assert!(out.contains("^idle"), "should contain idle region");
        assert!(out.contains("^running"), "should contain running region");
        assert!(out.contains("dataflow.message_field"), "should contain message_field op");
        assert!(out.contains("speed"), "should reference speed field");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mlir-codegen -- emit_sm_with_topic`
Expected: FAIL -- `emit_state_machine_tick` does not emit `dataflow.message_field` yet.

- [ ] **Step 3: Update emit_state_machine_tick for topic guards**

In `mlir-codegen/src/state_machine.rs`, update the transition emission loop (inside `emit_state_machine_tick`, the `for t in &from_transitions` block) to detect the guard type from JSON:

- If `guard.type == "Topic"`: emit `dataflow.message_field` to extract the field, then `arith.cmpf` + `cf.cond_br` for the condition. If no condition, emit `cf.br` (message presence is enough).
- If `guard.type == "GuardPort"`: existing float threshold logic (unchanged).
- If `guard.type == "Unconditional"` or missing: `cf.br` unconditional.
- Fall back to legacy `guard_port` field if `guard` object is not present (backward compat).

The key new MLIR pattern emitted for a Topic guard with condition:

```mlir
    // guard: motor_cmd.speed Gt 0.0
    %sm20_field_0_speed = dataflow.message_field %motor_cmd, "speed" : f64
    %sm20_thresh_0 = arith.constant 0.0 : f64
    %sm20_cmp_0 = arith.cmpf "ogt", %sm20_field_0_speed, %sm20_thresh_0 : f64
    cf.cond_br %sm20_cmp_0, ^running, ^idle
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mlir-codegen -- state_machine`
Expected: All tests pass, including the new topic guard test and existing tests.

- [ ] **Step 5: Commit**

```bash
git add mlir-codegen/src/state_machine.rs
git commit -m "feat: MLIR state machine emission with typed channel reads and field conditions"
```

---

### Task 8: Extend DagRuntime for new guard format

**Files:**
- Modify: `mlir-codegen/src/runtime.rs`

- [ ] **Step 1: Write test for new-format state machine in DagRuntime**

Add to the test module in `mlir-codegen/src/runtime.rs`:

```rust
    #[test]
    fn state_machine_new_guard_format() {
        let block = BlockSnapshot {
            id: 1,
            block_type: "state_machine".to_string(),
            name: "SM".to_string(),
            inputs: vec![PortDef { name: "guard_0".to_string(), kind: PortKind::Float }],
            outputs: vec![
                PortDef { name: "state".to_string(), kind: PortKind::Float },
                PortDef { name: "active_idle".to_string(), kind: PortKind::Float },
                PortDef { name: "active_running".to_string(), kind: PortKind::Float },
            ],
            config: serde_json::json!({
                "states": ["idle", "running"],
                "initial": "idle",
                "transitions": [{
                    "from": "idle",
                    "to": "running",
                    "guard": {"type": "GuardPort", "port": 0},
                    "actions": []
                }],
                "input_topics": [],
                "output_topics": []
            }),
            output_values: vec![],
            custom_codegen: None,
        };
        let bf = BlockFn::from_snapshot(&block).unwrap();
        match &bf {
            BlockFn::StateMachine { n_states, initial, transitions } => {
                assert_eq!(*n_states, 2);
                assert_eq!(*initial, 0);
                assert_eq!(transitions.len(), 1);
                assert_eq!(transitions[0].from, 0);
                assert_eq!(transitions[0].to, 1);
                assert_eq!(transitions[0].guard, 0);
            }
            other => panic!("expected StateMachine, got {:?}", other),
        }
    }
```

- [ ] **Step 2: Run test to see if it passes or fails**

Run: `cargo test -p mlir-codegen -- state_machine_new_guard`
Expected: May fail if `from_snapshot` cannot parse the new guard JSON format. If it fails, update the `"state_machine"` arm in `BlockFn::from_snapshot()`.

- [ ] **Step 3: Update from_snapshot if needed**

In the `"state_machine"` match arm of `BlockFn::from_snapshot()`, update the transition parsing to handle both the new `guard: {type: "GuardPort", port: N}` format and the legacy `guard_port: N` format:

```rust
            "state_machine" => {
                let states: Vec<String> = config
                    .get("states").and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let initial_name = config.get("initial").and_then(|v| v.as_str()).unwrap_or("");
                let initial = states.iter().position(|s| s == initial_name).unwrap_or(0) as u8;
                let transitions = config
                    .get("transitions").and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter().filter_map(|t| {
                            let from_name = t.get("from")?.as_str()?;
                            let to_name = t.get("to")?.as_str()?;
                            let from = states.iter().position(|s| s == from_name)? as u8;
                            let to = states.iter().position(|s| s == to_name)? as u8;
                            let guard = if let Some(g) = t.get("guard") {
                                match g.get("type").and_then(|v| v.as_str()) {
                                    Some("GuardPort") => {
                                        g.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u8
                                    }
                                    Some("Unconditional") => u8::MAX,
                                    _ => 0, // Topic guards: MVP runtime uses port 0
                                }
                            } else {
                                // Legacy: guard_port field directly on transition
                                t.get("guard_port").and_then(|v| v.as_u64()).unwrap_or(0) as u8
                            };
                            Some(SmTransition { from, to, guard })
                        }).collect()
                    }).unwrap_or_default();
                Ok(Self::StateMachine { n_states: states.len() as u8, initial, transitions })
            }
```

Also update the `call()` method to handle `guard == u8::MAX` (unconditional):

In the `StateMachine` arm of `call()`, change the guard check from:
```rust
if t.from == current && inp(inputs, t.guard as usize) > 0.5 {
```
to:
```rust
if t.from == current && (t.guard == u8::MAX || inp(inputs, t.guard as usize) > 0.5) {
```

- [ ] **Step 4: Run all mlir-codegen tests**

Run: `cargo test -p mlir-codegen`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add mlir-codegen/src/runtime.rs
git commit -m "feat: extend DagRuntime state machine for new guard format"
```

---

### Task 9: Update legacy Rust codegen (emit.rs)

**Files:**
- Modify: `src/dataflow/codegen/emit.rs`

- [ ] **Step 1: Write test for topic-driven state machine codegen**

Add to the test module in `src/dataflow/codegen/emit.rs`:

```rust
    #[test]
    fn state_machine_topic_codegen() {
        let snap = GraphSnapshot {
            blocks: vec![BlockSnapshot {
                id: 5,
                block_type: "state_machine".to_string(),
                name: "FSM".to_string(),
                inputs: vec![PortDef::new("motor_cmd", PortKind::Float)],
                outputs: vec![
                    PortDef::new("state", PortKind::Float),
                    PortDef::new("active_idle", PortKind::Float),
                    PortDef::new("active_running", PortKind::Float),
                ],
                config: serde_json::json!({
                    "states": ["idle", "running"],
                    "initial": "idle",
                    "transitions": [{
                        "from": "idle",
                        "to": "running",
                        "guard": {"type": "Topic", "topic": "motor_cmd", "condition": {"field": "speed", "op": "Gt", "value": 0.0}},
                        "actions": []
                    }],
                    "input_topics": [{"topic": "motor_cmd", "schema": {"name": "motor_cmd", "fields": [{"name": "speed", "field_type": "F32"}]}}],
                    "output_topics": []
                }),
                output_values: vec![],
                target: None,
                custom_codegen: None,
            }],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01);
        let blocks_rs = result.files.iter().find(|(n, _)| n == "src/blocks.rs").unwrap().1.as_str();
        assert!(blocks_rs.contains("idle"), "should contain idle state");
        assert!(blocks_rs.contains("running"), "should contain running state");
    }
```

- [ ] **Step 2: Run test**

Run: `cargo test -p rustcam -- state_machine_topic_codegen`
Expected: The existing `emit_state_machine_block` reads states from config JSON generically, so it should handle the new format. If it fails on the new guard structure, update the transition parsing in `emit_state_machine_block` to read `guard.type`/`guard.port` instead of `guard_port`.

- [ ] **Step 3: Fix any issues, run full test suite**

Run: `cargo test -p rustcam`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/dataflow/codegen/emit.rs
git commit -m "test: verify emit.rs handles topic-driven state machine config"
```

---

### Task 10: Integration smoke test

**Files:** None new -- validates end-to-end

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test`
Expected: All workspace tests pass.

- [ ] **Step 2: Run the frontend type check**

Run: `cd www && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 3: Build WASM**

Run: `wasm-pack build --target web`
Expected: Build succeeds.

- [ ] **Step 4: Commit any fixups**

If any changes were needed:
```bash
git add -A
git commit -m "fix: integration fixups for state machine block"
```
