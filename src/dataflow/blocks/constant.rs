//! Constant block: emits a fixed value every tick.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
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
