//! Serde blocks: serialize/deserialize between typed values and bytes/text.

use crate::dataflow::block::{Module, Tick, PortDef, PortKind, Value};

/// Encode a Float input as JSON text.
#[derive(Default)]
pub struct JsonEncodeBlock;

impl JsonEncodeBlock {
    pub fn new() -> Self {
        Self
    }
}

impl Module for JsonEncodeBlock {
    fn name(&self) -> &str {
        "JSON Encode"
    }
    fn block_type(&self) -> &str {
        "json_encode"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("in", PortKind::Any)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("text", PortKind::Text)]
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for JsonEncodeBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        let out = inputs
            .first()
            .and_then(|i| *i)
            .and_then(|v| serde_json::to_string(v).ok())
            .map(Value::Text);
        vec![out]
    }
}

/// Decode JSON text back to a Float value.
#[derive(Default)]
pub struct JsonDecodeBlock;

impl JsonDecodeBlock {
    pub fn new() -> Self {
        Self
    }
}

impl Module for JsonDecodeBlock {
    fn name(&self) -> &str {
        "JSON Decode"
    }
    fn block_type(&self) -> &str {
        "json_decode"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("text", PortKind::Text)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("out", PortKind::Any)]
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for JsonDecodeBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        let out = inputs
            .first()
            .and_then(|i| i.and_then(|v| v.as_text()))
            .and_then(|text| serde_json::from_str::<Value>(text).ok());
        vec![out]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json() {
        let mut enc = JsonEncodeBlock::new();
        let mut dec = JsonDecodeBlock::new();

        let input = Value::Float(1.234);
        let encoded = enc.tick(&[Some(&input)], 0.01);
        let text = encoded[0].as_ref().unwrap();
        assert!(text.as_text().is_some());

        let decoded = dec.tick(&[Some(text)], 0.01);
        let result = decoded[0].as_ref().unwrap();
        assert_eq!(result.as_float(), Some(1.234));
    }

    #[test]
    fn json_encode_module_trait() {
        let mut b = JsonEncodeBlock::new();
        assert_eq!(b.name(), "JSON Encode");
        assert_eq!(b.block_type(), "json_encode");
        assert_eq!(b.input_ports().len(), 1);
        assert_eq!(b.output_ports().len(), 1);
        assert_eq!(b.config_json(), "{}");
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
        assert!(b.as_tick().is_some());
    }

    #[test]
    fn json_decode_module_trait() {
        let mut b = JsonDecodeBlock::new();
        assert_eq!(b.name(), "JSON Decode");
        assert_eq!(b.block_type(), "json_decode");
        assert_eq!(b.input_ports().len(), 1);
        assert_eq!(b.output_ports().len(), 1);
        assert_eq!(b.config_json(), "{}");
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
        assert!(b.as_tick().is_some());
    }
}
