//! Register block: generic z⁻¹ delay element.
//!
//! The only block in the system with internal persistent state.
//! Output at tick N equals the input at tick N-1.
//!
//! Config JSON:
//! ```json
//! { "initial_value": 0.0 }
//! ```

use crate::dataflow::block::{Codegen, Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(default)]
pub struct RegisterConfig {
    pub initial_value: f64,
}

impl Default for RegisterConfig {
    fn default() -> Self {
        Self { initial_value: 0.0 }
    }
}

// ---------------------------------------------------------------------------
// Block implementation
// ---------------------------------------------------------------------------

pub struct RegisterBlock {
    config: RegisterConfig,
    stored_value: Value,
}

impl RegisterBlock {
    pub fn new(config: RegisterConfig) -> Self {
        let stored_value = Value::Float(config.initial_value);
        Self {
            config,
            stored_value,
        }
    }
}

impl Module for RegisterBlock {
    fn name(&self) -> &str {
        "Register"
    }

    fn block_type(&self) -> &str {
        "register"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("in", PortKind::Float)]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("out", PortKind::Float)]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }

    fn is_delay(&self) -> bool {
        true
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }

    fn as_codegen(&self) -> Option<&dyn Codegen> {
        Some(self)
    }
}

impl Tick for RegisterBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // z⁻¹ semantics: output the previously stored value
        let output = self.stored_value.clone();

        // If input is present, store it for next tick
        if let Some(Some(input)) = inputs.first() {
            self.stored_value = (*input).clone();
        }

        vec![Some(output)]
    }
}

impl Codegen for RegisterBlock {
    fn emit_rust(&self, _target: &str) -> Result<String, String> {
        // Emit a comment documenting this register's role.
        // The actual state field + read/write is handled by emit.rs (Phase 4).
        // For now, just emit a placeholder function.
        let initial = self.config.initial_value;
        Ok(format!("// Register: initial_value = {initial}"))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register(reg: &mut Vec<super::registry::BlockRegistration>) {
    reg.push(super::registry::BlockRegistration {
        block_type: "register",
        display_name: "Register",
        category: "Control",
        create_from_json: |json| {
            let cfg: RegisterConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(RegisterBlock::new(cfg)))
        },
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_register(initial: f64) -> RegisterBlock {
        RegisterBlock::new(RegisterConfig {
            initial_value: initial,
        })
    }

    #[test]
    fn initial_output_is_initial_value() {
        let mut reg = make_register(5.0);
        // Tick with no input — output should be initial value
        let result = reg.tick(&[], 0.01);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Some(Value::Float(5.0)));
    }

    #[test]
    fn z_minus_1_delay() {
        let mut reg = make_register(5.0);
        let v10 = Value::Float(10.0);
        let v20 = Value::Float(20.0);

        // First tick with input=10.0 — output should still be 5.0 (initial)
        let result = reg.tick(&[Some(&v10)], 0.01);
        assert_eq!(result[0], Some(Value::Float(5.0)));

        // Second tick with input=20.0 — output should be 10.0 (previous input)
        let result = reg.tick(&[Some(&v20)], 0.01);
        assert_eq!(result[0], Some(Value::Float(10.0)));
    }

    #[test]
    fn no_input_holds_value() {
        let mut reg = make_register(5.0);
        let v10 = Value::Float(10.0);

        // Store a value
        let _ = reg.tick(&[Some(&v10)], 0.01);

        // Tick with None input — output should be 10.0 (stored), value held
        let result = reg.tick(&[None], 0.01);
        assert_eq!(result[0], Some(Value::Float(10.0)));

        // Another tick with no input — output should still be 10.0
        let result = reg.tick(&[], 0.01);
        assert_eq!(result[0], Some(Value::Float(10.0)));
    }

    #[test]
    fn module_trait_methods() {
        let mut reg = make_register(0.0);
        assert_eq!(reg.name(), "Register");
        assert_eq!(reg.block_type(), "register");
        assert!(reg.is_delay());
        assert!(reg.as_tick().is_some());
        assert!(reg.as_codegen().is_some());
        assert!(reg.as_analysis().is_none());
        assert!(reg.as_sim_model().is_none());
    }

    #[test]
    fn input_output_ports() {
        let reg = make_register(0.0);
        let inputs = reg.input_ports();
        let outputs = reg.output_ports();

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "in");
        assert_eq!(inputs[0].kind, PortKind::Float);

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].name, "out");
        assert_eq!(outputs[0].kind, PortKind::Float);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = RegisterConfig {
            initial_value: 42.5,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: RegisterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn create_block_register() {
        let block =
            super::super::create_block("register", r#"{"initial_value": 3.14}"#).unwrap();
        assert_eq!(block.block_type(), "register");
        assert_eq!(block.name(), "Register");
    }

    #[test]
    fn codegen_returns_ok() {
        let reg = make_register(7.0);
        let cg = reg.as_codegen();
        assert!(cg.is_some());
        let result = cg.unwrap().emit_rust("host");
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("Register"));
        assert!(code.contains("7"));
    }
}
