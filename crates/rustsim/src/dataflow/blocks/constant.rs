//! Constant block: emits a fixed value every tick.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ConstantConfig {
    pub value: f64,
}

pub struct ConstantBlock {
    value: f64,
}

impl ConstantBlock {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn from_config(cfg: ConstantConfig) -> Self {
        Self::new(cfg.value)
    }
}

impl Module for ConstantBlock {
    fn name(&self) -> &str {
        "Constant"
    }

    fn block_type(&self) -> &str {
        "constant"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("out", PortKind::Float)]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&ConstantConfig { value: self.value }).unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for ConstantBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![Some(Value::Float(self.value))]
    }
}

pub(crate) fn register(reg: &mut Vec<super::registry::BlockRegistration>) {
    reg.push(super::registry::BlockRegistration {
        block_type: "constant",
        display_name: "Constant",
        category: "Sources",
        create_from_json: |json| {
            let cfg: ConstantConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(ConstantBlock::from_config(cfg)))
        },
    });
}
