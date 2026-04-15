//! Pure (non-wasm) logic for manipulating [`StateMachineConfig`] values.
//!
//! All helpers take an immutable reference and return a new config, keeping
//! state management simple for the Leptos signal-based UI.  These functions
//! are unit-testable on the host without a browser.

use block_registry::state_machine::{
    CompareOp, StateMachineConfig, TopicBinding, TransitionAction, TransitionConfig,
    TransitionGuard,
};
use module_traits::MessageSchema;

// ---------------------------------------------------------------------------
// Parse / serialize
// ---------------------------------------------------------------------------

/// Parse a [`serde_json::Value`] into a [`StateMachineConfig`].
///
/// Returns `None` when the JSON cannot be deserialized (e.g. null or
/// incompatible structure). An empty JSON object `{}` deserializes
/// successfully thanks to `#[serde(default)]` on the config struct.
pub fn parse_state_machine_config(json: &serde_json::Value) -> Option<StateMachineConfig> {
    serde_json::from_value(json.clone()).ok()
}

/// Serialize a [`StateMachineConfig`] to a [`serde_json::Value`].
pub fn serialize_state_machine_config(config: &StateMachineConfig) -> serde_json::Value {
    serde_json::to_value(config).unwrap_or(serde_json::Value::Null)
}

// ---------------------------------------------------------------------------
// State helpers
// ---------------------------------------------------------------------------

/// Add a new state with an auto-generated name that avoids collisions.
pub fn add_state(config: &StateMachineConfig) -> StateMachineConfig {
    let mut cfg = config.clone();
    let mut n = cfg.states.len();
    loop {
        let candidate = format!("state_{n}");
        if !cfg.states.contains(&candidate) {
            cfg.states.push(candidate);
            break;
        }
        n += 1;
    }
    cfg
}

/// Remove a state by index, cleaning up transitions and adjusting the
/// initial state if necessary. Refuses to remove the last remaining state.
pub fn remove_state(config: &StateMachineConfig, index: usize) -> StateMachineConfig {
    let mut cfg = config.clone();
    if index >= cfg.states.len() || cfg.states.len() <= 1 {
        return cfg;
    }
    let removed = cfg.states.remove(index);
    cfg.transitions
        .retain(|t| t.from != removed && t.to != removed);
    if cfg.initial == removed {
        cfg.initial = cfg.states.first().cloned().unwrap_or_default();
    }
    cfg
}

/// Rename a state at the given index, updating all transition references
/// and the initial field.
pub fn rename_state(
    config: &StateMachineConfig,
    index: usize,
    new_name: &str,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if index >= cfg.states.len() || new_name.is_empty() {
        return cfg;
    }
    let old_name = cfg.states[index].clone();
    cfg.states[index] = new_name.to_string();
    if cfg.initial == old_name {
        cfg.initial = new_name.to_string();
    }
    for t in &mut cfg.transitions {
        if t.from == old_name {
            t.from = new_name.to_string();
        }
        if t.to == old_name {
            t.to = new_name.to_string();
        }
    }
    cfg
}

/// Set the initial state to the state at the given index.
pub fn set_initial_state(config: &StateMachineConfig, index: usize) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(name) = cfg.states.get(index) {
        cfg.initial = name.clone();
    }
    cfg
}

// ---------------------------------------------------------------------------
// Transition helpers
// ---------------------------------------------------------------------------

/// Add a new unconditional self-transition on the first state.
pub fn add_transition(config: &StateMachineConfig) -> StateMachineConfig {
    let mut cfg = config.clone();
    let first = cfg.states.first().cloned().unwrap_or_default();
    cfg.transitions.push(TransitionConfig {
        from: first.clone(),
        to: first,
        guard: TransitionGuard::Unconditional,
        actions: vec![],
    });
    cfg
}

/// Remove a transition by index.
pub fn remove_transition(config: &StateMachineConfig, index: usize) -> StateMachineConfig {
    let mut cfg = config.clone();
    if index < cfg.transitions.len() {
        cfg.transitions.remove(index);
    }
    cfg
}

/// Update a transition's `from` field.
pub fn set_transition_from(
    config: &StateMachineConfig,
    index: usize,
    from: &str,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(t) = cfg.transitions.get_mut(index) {
        t.from = from.to_string();
    }
    cfg
}

/// Update a transition's `to` field.
pub fn set_transition_to(
    config: &StateMachineConfig,
    index: usize,
    to: &str,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(t) = cfg.transitions.get_mut(index) {
        t.to = to.to_string();
    }
    cfg
}

/// Set a transition's guard.
pub fn set_transition_guard(
    config: &StateMachineConfig,
    index: usize,
    guard: TransitionGuard,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(t) = cfg.transitions.get_mut(index) {
        t.guard = guard;
    }
    cfg
}

// ---------------------------------------------------------------------------
// Action helpers
// ---------------------------------------------------------------------------

/// Add an empty action to a transition.
pub fn add_transition_action(
    config: &StateMachineConfig,
    transition_index: usize,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(t) = cfg.transitions.get_mut(transition_index) {
        t.actions.push(TransitionAction {
            topic: String::new(),
            message: vec![],
        });
    }
    cfg
}

/// Remove an action from a transition.
pub fn remove_transition_action(
    config: &StateMachineConfig,
    transition_index: usize,
    action_index: usize,
) -> StateMachineConfig {
    let mut cfg = config.clone();
    if let Some(t) = cfg.transitions.get_mut(transition_index) {
        if action_index < t.actions.len() {
            t.actions.remove(action_index);
        }
    }
    cfg
}

// ---------------------------------------------------------------------------
// Topic binding helpers
// ---------------------------------------------------------------------------

/// Add an empty input topic binding.
pub fn add_input_topic(config: &StateMachineConfig) -> StateMachineConfig {
    let mut cfg = config.clone();
    cfg.input_topics.push(TopicBinding {
        topic: String::new(),
        schema: MessageSchema {
            name: String::new(),
            fields: vec![],
        },
    });
    cfg
}

/// Remove an input topic binding by index.
pub fn remove_input_topic(config: &StateMachineConfig, index: usize) -> StateMachineConfig {
    let mut cfg = config.clone();
    if index < cfg.input_topics.len() {
        cfg.input_topics.remove(index);
    }
    cfg
}

/// Add an empty output topic binding.
pub fn add_output_topic(config: &StateMachineConfig) -> StateMachineConfig {
    let mut cfg = config.clone();
    cfg.output_topics.push(TopicBinding {
        topic: String::new(),
        schema: MessageSchema {
            name: String::new(),
            fields: vec![],
        },
    });
    cfg
}

/// Remove an output topic binding by index.
pub fn remove_output_topic(config: &StateMachineConfig, index: usize) -> StateMachineConfig {
    let mut cfg = config.clone();
    if index < cfg.output_topics.len() {
        cfg.output_topics.remove(index);
    }
    cfg
}

// ---------------------------------------------------------------------------
// Label / display helpers
// ---------------------------------------------------------------------------

/// Human-readable label for a [`CompareOp`].
pub fn compare_op_label(op: &CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "==",
        CompareOp::Ne => "!=",
        CompareOp::Gt => ">",
        CompareOp::Lt => "<",
        CompareOp::Ge => ">=",
        CompareOp::Le => "<=",
    }
}

/// Parse a [`CompareOp`] from its label string.
pub fn compare_op_from_label(label: &str) -> CompareOp {
    match label {
        "==" => CompareOp::Eq,
        "!=" => CompareOp::Ne,
        ">" => CompareOp::Gt,
        "<" => CompareOp::Lt,
        ">=" => CompareOp::Ge,
        "<=" => CompareOp::Le,
        _ => CompareOp::Eq,
    }
}

/// Guard variant label (e.g. "Topic", "Unconditional", "GuardPort").
pub fn guard_type_label(guard: &TransitionGuard) -> &'static str {
    match guard {
        TransitionGuard::Topic { .. } => "Topic",
        TransitionGuard::Unconditional => "Unconditional",
        TransitionGuard::GuardPort { .. } => "GuardPort",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use block_registry::state_machine::FieldCondition;
    use module_traits::{FieldType, MessageField, MessageSchema};

    #[test]
    fn test_parse_empty_config() {
        let json = serde_json::json!({});
        let cfg = parse_state_machine_config(&json).expect("should parse empty object");
        assert_eq!(cfg.states, vec!["idle".to_string()]);
        assert_eq!(cfg.initial, "idle");
        assert!(cfg.transitions.is_empty());
        assert!(cfg.input_topics.is_empty());
        assert!(cfg.output_topics.is_empty());
    }

    #[test]
    fn test_parse_null_returns_none() {
        let json = serde_json::Value::Null;
        assert!(parse_state_machine_config(&json).is_none());
    }

    #[test]
    fn test_roundtrip_config() {
        let config = StateMachineConfig {
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
        };

        let json = serialize_state_machine_config(&config);
        let parsed = parse_state_machine_config(&json).expect("should roundtrip");
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_roundtrip_guard_port() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string()],
            initial: "a".to_string(),
            transitions: vec![TransitionConfig {
                from: "a".to_string(),
                to: "b".to_string(),
                guard: TransitionGuard::GuardPort { port: 2 },
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let json = serialize_state_machine_config(&config);
        let parsed = parse_state_machine_config(&json).expect("should roundtrip guard port");
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_roundtrip_unconditional() {
        let config = StateMachineConfig {
            states: vec!["x".to_string()],
            initial: "x".to_string(),
            transitions: vec![TransitionConfig {
                from: "x".to_string(),
                to: "x".to_string(),
                guard: TransitionGuard::Unconditional,
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let json = serialize_state_machine_config(&config);
        let parsed = parse_state_machine_config(&json).expect("should roundtrip unconditional");
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_add_state() {
        let config = StateMachineConfig::default();
        assert_eq!(config.states.len(), 1);

        let updated = add_state(&config);
        assert_eq!(updated.states.len(), 2);
        assert_eq!(updated.states[1], "state_1");

        let updated2 = add_state(&updated);
        assert_eq!(updated2.states.len(), 3);
        assert_eq!(updated2.states[2], "state_2");
    }

    #[test]
    fn test_add_state_avoids_collision() {
        let mut config = StateMachineConfig::default();
        config.states.push("state_1".to_string());

        let updated = add_state(&config);
        assert_eq!(updated.states.len(), 3);
        assert_eq!(updated.states[2], "state_2");
    }

    #[test]
    fn test_remove_state() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            initial: "a".to_string(),
            transitions: vec![
                TransitionConfig {
                    from: "a".to_string(),
                    to: "b".to_string(),
                    guard: TransitionGuard::Unconditional,
                    actions: vec![],
                },
                TransitionConfig {
                    from: "b".to_string(),
                    to: "c".to_string(),
                    guard: TransitionGuard::Unconditional,
                    actions: vec![],
                },
            ],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = remove_state(&config, 1); // remove "b"
        assert_eq!(updated.states, vec!["a".to_string(), "c".to_string()]);
        assert!(updated.transitions.is_empty());
    }

    #[test]
    fn test_remove_initial_state() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string()],
            initial: "a".to_string(),
            transitions: vec![],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = remove_state(&config, 0);
        assert_eq!(updated.states, vec!["b".to_string()]);
        assert_eq!(updated.initial, "b");
    }

    #[test]
    fn test_remove_last_state_is_noop() {
        let config = StateMachineConfig {
            states: vec!["only".to_string()],
            initial: "only".to_string(),
            transitions: vec![],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = remove_state(&config, 0);
        assert_eq!(updated.states.len(), 1);
    }

    #[test]
    fn test_rename_state() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string()],
            initial: "a".to_string(),
            transitions: vec![TransitionConfig {
                from: "a".to_string(),
                to: "b".to_string(),
                guard: TransitionGuard::Unconditional,
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = rename_state(&config, 0, "alpha");
        assert_eq!(updated.states[0], "alpha");
        assert_eq!(updated.initial, "alpha");
        assert_eq!(updated.transitions[0].from, "alpha");
        assert_eq!(updated.transitions[0].to, "b");
    }

    #[test]
    fn test_set_initial_state() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string()],
            initial: "a".to_string(),
            transitions: vec![],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = set_initial_state(&config, 1);
        assert_eq!(updated.initial, "b");
    }

    #[test]
    fn test_add_transition() {
        let config = StateMachineConfig {
            states: vec!["idle".to_string(), "running".to_string()],
            initial: "idle".to_string(),
            transitions: vec![],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = add_transition(&config);
        assert_eq!(updated.transitions.len(), 1);
        assert_eq!(updated.transitions[0].from, "idle");
        assert_eq!(updated.transitions[0].to, "idle");
        assert_eq!(updated.transitions[0].guard, TransitionGuard::Unconditional);
        assert!(updated.transitions[0].actions.is_empty());
    }

    #[test]
    fn test_remove_transition() {
        let config = StateMachineConfig {
            states: vec!["a".to_string()],
            initial: "a".to_string(),
            transitions: vec![
                TransitionConfig {
                    from: "a".to_string(),
                    to: "a".to_string(),
                    guard: TransitionGuard::Unconditional,
                    actions: vec![],
                },
                TransitionConfig {
                    from: "a".to_string(),
                    to: "a".to_string(),
                    guard: TransitionGuard::GuardPort { port: 0 },
                    actions: vec![],
                },
            ],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = remove_transition(&config, 0);
        assert_eq!(updated.transitions.len(), 1);
        assert_eq!(
            updated.transitions[0].guard,
            TransitionGuard::GuardPort { port: 0 }
        );
    }

    #[test]
    fn test_set_transition_from_to() {
        let config = StateMachineConfig {
            states: vec!["a".to_string(), "b".to_string()],
            initial: "a".to_string(),
            transitions: vec![TransitionConfig {
                from: "a".to_string(),
                to: "a".to_string(),
                guard: TransitionGuard::Unconditional,
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = set_transition_from(&config, 0, "b");
        assert_eq!(updated.transitions[0].from, "b");
        assert_eq!(updated.transitions[0].to, "a");

        let updated2 = set_transition_to(&updated, 0, "b");
        assert_eq!(updated2.transitions[0].to, "b");
    }

    #[test]
    fn test_set_transition_guard() {
        let config = StateMachineConfig {
            states: vec!["a".to_string()],
            initial: "a".to_string(),
            transitions: vec![TransitionConfig {
                from: "a".to_string(),
                to: "a".to_string(),
                guard: TransitionGuard::Unconditional,
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = set_transition_guard(
            &config,
            0,
            TransitionGuard::Topic {
                topic: "cmd".to_string(),
                condition: None,
            },
        );
        match &updated.transitions[0].guard {
            TransitionGuard::Topic { topic, condition } => {
                assert_eq!(topic, "cmd");
                assert!(condition.is_none());
            }
            other => panic!("expected Topic guard, got {:?}", other),
        }
    }

    #[test]
    fn test_add_remove_transition_action() {
        let config = StateMachineConfig {
            states: vec!["a".to_string()],
            initial: "a".to_string(),
            transitions: vec![TransitionConfig {
                from: "a".to_string(),
                to: "a".to_string(),
                guard: TransitionGuard::Unconditional,
                actions: vec![],
            }],
            input_topics: vec![],
            output_topics: vec![],
        };

        let updated = add_transition_action(&config, 0);
        assert_eq!(updated.transitions[0].actions.len(), 1);
        assert_eq!(updated.transitions[0].actions[0].topic, "");
        assert!(updated.transitions[0].actions[0].message.is_empty());

        let updated2 = remove_transition_action(&updated, 0, 0);
        assert!(updated2.transitions[0].actions.is_empty());
    }

    #[test]
    fn test_add_remove_input_topic() {
        let config = StateMachineConfig::default();
        assert!(config.input_topics.is_empty());

        let updated = add_input_topic(&config);
        assert_eq!(updated.input_topics.len(), 1);
        assert_eq!(updated.input_topics[0].topic, "");

        let updated2 = remove_input_topic(&updated, 0);
        assert!(updated2.input_topics.is_empty());
    }

    #[test]
    fn test_add_remove_output_topic() {
        let config = StateMachineConfig::default();
        assert!(config.output_topics.is_empty());

        let updated = add_output_topic(&config);
        assert_eq!(updated.output_topics.len(), 1);

        let updated2 = remove_output_topic(&updated, 0);
        assert!(updated2.output_topics.is_empty());
    }

    #[test]
    fn test_compare_op_roundtrip() {
        let ops = [
            CompareOp::Eq,
            CompareOp::Ne,
            CompareOp::Gt,
            CompareOp::Lt,
            CompareOp::Ge,
            CompareOp::Le,
        ];
        for op in &ops {
            let label = compare_op_label(op);
            let parsed = compare_op_from_label(label);
            assert_eq!(*op, parsed, "roundtrip failed for {label}");
        }
    }

    #[test]
    fn test_guard_type_label() {
        assert_eq!(
            guard_type_label(&TransitionGuard::Unconditional),
            "Unconditional"
        );
        assert_eq!(
            guard_type_label(&TransitionGuard::Topic {
                topic: "x".to_string(),
                condition: None
            }),
            "Topic"
        );
        assert_eq!(
            guard_type_label(&TransitionGuard::GuardPort { port: 0 }),
            "GuardPort"
        );
    }

    #[test]
    fn test_parse_minimal_json() {
        let json = serde_json::json!({
            "states": ["a"],
            "initial": "a"
        });
        let cfg = parse_state_machine_config(&json).expect("should parse minimal");
        assert_eq!(cfg.states, vec!["a".to_string()]);
        assert_eq!(cfg.initial, "a");
        assert!(cfg.transitions.is_empty());
    }

    #[test]
    fn test_parse_full_topic_config() {
        let json = serde_json::json!({
            "states": ["idle", "running"],
            "initial": "idle",
            "transitions": [{
                "from": "idle",
                "to": "running",
                "guard": {
                    "type": "Topic",
                    "topic": "motor_cmd",
                    "condition": {
                        "field": "speed",
                        "op": "Gt",
                        "value": 0.0
                    }
                },
                "actions": [{
                    "topic": "motor_status",
                    "message": [["running", 1.0]]
                }]
            }],
            "input_topics": [{
                "topic": "motor_cmd",
                "schema": {
                    "name": "MotorCmd",
                    "fields": [{"name": "speed", "field_type": "F64"}]
                }
            }],
            "output_topics": [{
                "topic": "motor_status",
                "schema": {
                    "name": "MotorStatus",
                    "fields": [{"name": "running", "field_type": "F64"}]
                }
            }]
        });

        let cfg = parse_state_machine_config(&json).expect("should parse full config");
        assert_eq!(cfg.states.len(), 2);
        assert_eq!(cfg.transitions.len(), 1);
        assert_eq!(cfg.input_topics.len(), 1);
        assert_eq!(cfg.output_topics.len(), 1);
        assert_eq!(cfg.transitions[0].from, "idle");
        assert_eq!(cfg.transitions[0].to, "running");
        match &cfg.transitions[0].guard {
            TransitionGuard::Topic { topic, condition } => {
                assert_eq!(topic, "motor_cmd");
                let cond = condition.as_ref().expect("should have condition");
                assert_eq!(cond.field, "speed");
                assert_eq!(cond.op, CompareOp::Gt);
                assert!((cond.value - 0.0).abs() < f64::EPSILON);
            }
            other => panic!("expected Topic guard, got {:?}", other),
        }
        assert_eq!(cfg.transitions[0].actions.len(), 1);
        assert_eq!(cfg.transitions[0].actions[0].topic, "motor_status");
        assert_eq!(
            cfg.transitions[0].actions[0].message,
            vec![("running".to_string(), 1.0)]
        );
    }

    #[test]
    fn test_serialize_produces_valid_json() {
        let config = StateMachineConfig::default();
        let json = serialize_state_machine_config(&config);
        assert!(json.is_object());
        assert!(json.get("states").is_some());
        assert!(json.get("initial").is_some());
    }
}
