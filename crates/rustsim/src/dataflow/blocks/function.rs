//! Function blocks: math operations on float inputs.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

#[derive(Debug, Clone, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub enum FunctionOp {
    Gain,
    Add,
    Multiply,
    Clamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[schemars(default)]
pub struct FunctionConfig {
    pub op: FunctionOp,
    #[serde(default)]
    pub param1: f64,
    #[serde(default)]
    pub param2: f64,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            op: FunctionOp::Gain,
            param1: 1.0,
            param2: 0.0,
        }
    }
}

pub struct FunctionBlock {
    config: FunctionConfig,
}

impl FunctionBlock {
    pub fn from_config(cfg: FunctionConfig) -> Self {
        Self { config: cfg }
    }

    /// Single-input gain: out = in * factor.
    pub fn gain(factor: f64) -> Self {
        Self {
            config: FunctionConfig {
                op: FunctionOp::Gain,
                param1: factor,
                param2: 0.0,
            },
        }
    }

    /// Two-input addition: out = a + b.
    pub fn add() -> Self {
        Self {
            config: FunctionConfig {
                op: FunctionOp::Add,
                param1: 0.0,
                param2: 0.0,
            },
        }
    }

    /// Two-input multiplication: out = a * b.
    pub fn multiply() -> Self {
        Self {
            config: FunctionConfig {
                op: FunctionOp::Multiply,
                param1: 0.0,
                param2: 0.0,
            },
        }
    }

    /// Single-input clamp: out = clamp(in, min, max).
    pub fn clamp(min: f64, max: f64) -> Self {
        Self {
            config: FunctionConfig {
                op: FunctionOp::Clamp,
                param1: min,
                param2: max,
            },
        }
    }
}

impl Module for FunctionBlock {
    fn name(&self) -> &str {
        match self.config.op {
            FunctionOp::Gain => "Gain",
            FunctionOp::Add => "Add",
            FunctionOp::Multiply => "Multiply",
            FunctionOp::Clamp => "Clamp",
        }
    }

    fn block_type(&self) -> &str {
        match self.config.op {
            FunctionOp::Gain => "gain",
            FunctionOp::Add => "add",
            FunctionOp::Multiply => "multiply",
            FunctionOp::Clamp => "clamp",
        }
    }

    fn input_ports(&self) -> Vec<PortDef> {
        match self.config.op {
            FunctionOp::Gain | FunctionOp::Clamp => {
                vec![PortDef::new("in", PortKind::Float)]
            }
            FunctionOp::Add | FunctionOp::Multiply => {
                vec![
                    PortDef::new("a", PortKind::Float),
                    PortDef::new("b", PortKind::Float),
                ]
            }
        }
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("out", PortKind::Float)]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for FunctionBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        let result = match self.config.op {
            FunctionOp::Gain => {
                let v = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                v.map(|x| x * self.config.param1)
            }
            FunctionOp::Add => {
                let a = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                match (a, b) {
                    (Some(a), Some(b)) => Some(a + b),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                }
            }
            FunctionOp::Multiply => {
                let a = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                match (a, b) {
                    (Some(a), Some(b)) => Some(a * b),
                    _ => None,
                }
            }
            FunctionOp::Clamp => {
                let v = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                v.map(|x| x.clamp(self.config.param1, self.config.param2))
            }
        };
        vec![result.map(Value::Float)]
    }
}

pub(crate) fn register(reg: &mut Vec<super::registry::BlockRegistration>) {
    reg.push(super::registry::BlockRegistration {
        block_type: "gain",
        display_name: "Gain",
        category: "Math",
        create_from_json: |json| {
            let cfg: FunctionConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(FunctionBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "add",
        display_name: "Add",
        category: "Math",
        create_from_json: |_json| Ok(Box::new(FunctionBlock::add())),
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "multiply",
        display_name: "Multiply",
        category: "Math",
        create_from_json: |_json| Ok(Box::new(FunctionBlock::multiply())),
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "clamp",
        display_name: "Clamp",
        category: "Math",
        create_from_json: |json| {
            let cfg: FunctionConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(FunctionBlock::from_config(cfg)))
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_block() {
        let mut b = FunctionBlock::gain(3.0);
        let input = Value::Float(4.0);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(12.0));
    }

    #[test]
    fn add_block() {
        let mut b = FunctionBlock::add();
        let a = Value::Float(2.0);
        let bv = Value::Float(3.0);
        let out = b.tick(&[Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(5.0));
    }

    #[test]
    fn clamp_block() {
        let mut b = FunctionBlock::clamp(0.0, 10.0);
        let input = Value::Float(15.0);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(10.0));
    }

    #[test]
    fn multiply_block() {
        let mut b = FunctionBlock::multiply();
        let a = Value::Float(3.0);
        let bv = Value::Float(4.0);
        let out = b.tick(&[Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(12.0));
        // Missing input returns None
        let out2 = b.tick(&[Some(&a), None], 0.01);
        assert_eq!(out2[0], None);
    }

    #[test]
    fn as_analysis_returns_none() {
        let b = FunctionBlock::gain(1.0);
        assert!(b.as_analysis().is_none());
    }
}
