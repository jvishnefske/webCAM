//! State machine block: configurable finite state machine.
//!
//! Config JSON:
//! ```json
//! {
//!   "states": ["idle", "running", "error"],
//!   "initial": "idle",
//!   "transitions": [
//!     { "from": "idle", "to": "running", "guard_port": 0 },
//!     { "from": "running", "to": "error", "guard_port": 1 },
//!     { "from": "error", "to": "idle", "guard_port": null }
//!   ]
//! }
//! ```
//!
//! Input ports: one per guard (Float, >0.5 = true)
//! Output ports: `state` (Float, enum index), plus `active_<name>` per state (0.0 or 1.0)

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub guard_port: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineConfig {
    pub states: Vec<String>,
    pub initial: String,
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            states: vec!["idle".to_string()],
            initial: "idle".to_string(),
            transitions: vec![],
        }
    }
}

pub struct StateMachineBlock {
    config: StateMachineConfig,
    current_state: usize,
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

    fn n_guards(&self) -> usize {
        self.config
            .transitions
            .iter()
            .filter_map(|t| t.guard_port)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
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
        (0..self.n_guards())
            .map(|i| PortDef::new(&format!("guard_{i}"), PortKind::Float))
            .collect()
    }

    fn output_ports(&self) -> Vec<PortDef> {
        let mut ports = vec![PortDef::new("state", PortKind::Float)];
        for state_name in &self.config.states {
            ports.push(PortDef::new(
                &format!("active_{state_name}"),
                PortKind::Float,
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
        let current_name = &self.config.states[self.current_state].clone();

        // Evaluate transitions from current state
        for t in &self.config.transitions {
            if t.from != *current_name {
                continue;
            }
            let guard_active = match t.guard_port {
                Some(port) => {
                    inputs
                        .get(port)
                        .and_then(|v| v.as_ref())
                        .and_then(|v| v.as_float())
                        .unwrap_or(0.0)
                        > 0.5
                }
                None => true, // unconditional
            };
            if guard_active {
                if let Some(idx) = self.config.states.iter().position(|s| s == &t.to) {
                    self.current_state = idx;
                    break;
                }
            }
        }

        // Build outputs
        let mut outputs = vec![Some(Value::Float(self.current_state as f64))];
        for (i, _) in self.config.states.iter().enumerate() {
            outputs.push(Some(Value::Float(if i == self.current_state {
                1.0
            } else {
                0.0
            })));
        }
        outputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sm() -> StateMachineBlock {
        StateMachineBlock::from_config(StateMachineConfig {
            states: vec![
                "idle".to_string(),
                "running".to_string(),
                "error".to_string(),
            ],
            initial: "idle".to_string(),
            transitions: vec![
                Transition {
                    from: "idle".to_string(),
                    to: "running".to_string(),
                    guard_port: Some(0),
                },
                Transition {
                    from: "running".to_string(),
                    to: "error".to_string(),
                    guard_port: Some(1),
                },
                Transition {
                    from: "error".to_string(),
                    to: "idle".to_string(),
                    guard_port: None,
                },
            ],
        })
    }

    #[test]
    fn initial_state() {
        let mut sm = make_sm();
        let result = sm.tick(&[], 0.01);
        // state=0 (idle), active_idle=1, active_running=0, active_error=0
        assert_eq!(result[0], Some(Value::Float(0.0)));
        assert_eq!(result[1], Some(Value::Float(1.0)));
        assert_eq!(result[2], Some(Value::Float(0.0)));
        assert_eq!(result[3], Some(Value::Float(0.0)));
    }

    #[test]
    fn guard_transition() {
        let mut sm = make_sm();
        let high = Value::Float(1.0);
        let low = Value::Float(0.0);

        // guard_0 = high → idle→running
        let result = sm.tick(&[Some(&high), Some(&low)], 0.01);
        assert_eq!(result[0], Some(Value::Float(1.0))); // running
        assert_eq!(result[2], Some(Value::Float(1.0))); // active_running

        // guard_1 = high → running→error
        let result = sm.tick(&[Some(&low), Some(&high)], 0.01);
        assert_eq!(result[0], Some(Value::Float(2.0))); // error
        assert_eq!(result[3], Some(Value::Float(1.0))); // active_error
    }

    #[test]
    fn unconditional_transition() {
        let mut sm = make_sm();
        // Force to error state
        sm.current_state = 2;
        // error→idle is unconditional
        let result = sm.tick(&[], 0.01);
        assert_eq!(result[0], Some(Value::Float(0.0))); // back to idle
    }

    #[test]
    fn ports() {
        let sm = make_sm();
        assert_eq!(sm.input_ports().len(), 2); // guard_0, guard_1
        assert_eq!(sm.output_ports().len(), 4); // state + 3 active flags
    }

    #[test]
    fn module_trait_methods() {
        let mut sm = make_sm();
        assert_eq!(sm.name(), "State Machine");
        assert_eq!(sm.block_type(), "state_machine");
        let config: serde_json::Value = serde_json::from_str(&sm.config_json()).unwrap();
        assert_eq!(config["initial"], "idle");
        assert!(sm.as_tick().is_some());
        assert!(sm.as_analysis().is_none());
        assert!(sm.as_codegen().is_none());
        assert!(sm.as_sim_model().is_none());
    }

    #[test]
    fn config_default() {
        let cfg = StateMachineConfig::default();
        assert!(!cfg.states.is_empty());
        assert!(!cfg.initial.is_empty());
    }
}
