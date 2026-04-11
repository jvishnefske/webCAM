//! Data-driven block: a single generic block type parameterized by [`FunctionDef`].
//!
//! Replaces per-block Rust structs (ConstantBlock, FunctionBlock, PlotBlock, etc.)
//! with a single implementation that reads its behaviour from a [`FunctionDef`]
//! and a JSON config.  This eliminates block-specific model code from WASM and
//! lets the frontend derive all schema, naming, and UI from the registry.

use std::collections::HashMap;

use module_traits::function_def::{FunctionDef, FunctionOp};
use module_traits::value::{PortDef, Value};
use module_traits::{Module, Tick};

/// A block instance driven by a [`FunctionDef`] + runtime config values.
pub struct DataDrivenBlock {
    def: FunctionDef,
    /// Runtime parameter values parsed from config JSON.
    params: HashMap<String, f64>,
    /// String parameter values (for topics, channel names, etc.).
    string_params: HashMap<String, String>,
    /// Internal state (plot buffer, pubsub last value, etc.).
    state: BlockState,
}

/// Minimal internal state — only what's needed for simulation.
/// Hardware-specific state lives in the BSP/channel layer, not here.
enum BlockState {
    None,
    /// Plot accumulator buffer.
    PlotBuffer { buffer: Vec<f64>, max_samples: usize },
    /// PubSub source: holds current value until overwritten.
    PubSubSource { current: Option<Value> },
    /// PubSub sink: holds last received value.
    PubSubSink { last: Option<Value> },
}

impl DataDrivenBlock {
    /// Create a new block from a function definition and JSON config.
    pub fn new(def: FunctionDef, config_json: &str) -> Result<Self, String> {
        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| format!("invalid config JSON for {}: {e}", def.id))?;

        let mut params = HashMap::new();
        let mut string_params = HashMap::new();

        // Extract parameter values from config, falling back to defaults.
        for p in &def.params {
            match p.kind {
                module_traits::function_def::ParamKind::Float => {
                    let val = config
                        .get(&p.name)
                        .and_then(|v| v.as_f64())
                        .unwrap_or_else(|| p.default.parse::<f64>().unwrap_or(0.0));
                    params.insert(p.name.clone(), val);
                }
                module_traits::function_def::ParamKind::Int => {
                    let val = config
                        .get(&p.name)
                        .and_then(|v| v.as_i64())
                        .unwrap_or_else(|| p.default.parse::<i64>().unwrap_or(0))
                        as f64;
                    params.insert(p.name.clone(), val);
                }
                module_traits::function_def::ParamKind::String => {
                    let val = config
                        .get(&p.name)
                        .and_then(|v| v.as_str())
                        .unwrap_or(&p.default)
                        .to_string();
                    string_params.insert(p.name.clone(), val);
                }
                module_traits::function_def::ParamKind::Bool => {
                    let val = config
                        .get(&p.name)
                        .and_then(|v| v.as_bool())
                        .unwrap_or_else(|| p.default == "true");
                    params.insert(p.name.clone(), if val { 1.0 } else { 0.0 });
                }
            }
        }

        // Legacy config support: map old field names to new param names.
        // These override defaults when the old keys are present in the config.
        if def.op == FunctionOp::Gain {
            if let Some(v) = config.get("param1").and_then(|v| v.as_f64()) {
                params.insert("gain".into(), v);
            }
        }
        if def.op == FunctionOp::Clamp {
            if let Some(v) = config.get("param1").and_then(|v| v.as_f64()) {
                params.insert("min".into(), v);
            }
            if let Some(v) = config.get("param2").and_then(|v| v.as_f64()) {
                params.insert("max".into(), v);
            }
        }

        // Legacy: map "port_kind" string to string_params for pubsub
        if matches!(def.op, FunctionOp::PubSubSource | FunctionOp::PubSubSink) {
            if !string_params.contains_key("topic") {
                if let Some(t) = config.get("topic").and_then(|v| v.as_str()) {
                    string_params.insert("topic".into(), t.to_string());
                }
            }
            if !string_params.contains_key("port_kind") {
                if let Some(pk) = config.get("port_kind").and_then(|v| v.as_str()) {
                    string_params.insert("port_kind".into(), pk.to_string());
                }
            }
        }

        let state = match def.op {
            FunctionOp::PlotAccum => {
                let max = params.get("max_samples").copied().unwrap_or(500.0) as usize;
                BlockState::PlotBuffer {
                    buffer: Vec::new(),
                    max_samples: max,
                }
            }
            FunctionOp::PubSubSource => BlockState::PubSubSource { current: None },
            FunctionOp::PubSubSink => BlockState::PubSubSink { last: None },
            _ => BlockState::None,
        };

        Ok(Self {
            def,
            params,
            string_params,
            state,
        })
    }

    fn param_f64(&self, name: &str) -> f64 {
        self.params.get(name).copied().unwrap_or(0.0)
    }
}

impl Module for DataDrivenBlock {
    fn name(&self) -> &str {
        &self.def.display_name
    }

    fn block_type(&self) -> &str {
        &self.def.id
    }

    fn input_ports(&self) -> Vec<PortDef> {
        self.def
            .inputs
            .iter()
            .map(|p| PortDef::new(&p.name, p.kind.clone()))
            .collect()
    }

    fn output_ports(&self) -> Vec<PortDef> {
        self.def
            .outputs
            .iter()
            .map(|p| PortDef::new(&p.name, p.kind.clone()))
            .collect()
    }

    fn config_json(&self) -> String {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.params {
            map.insert(k.clone(), serde_json::Value::from(*v));
        }
        for (k, v) in &self.string_params {
            map.insert(k.clone(), serde_json::Value::from(v.clone()));
        }
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for DataDrivenBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        match self.def.op {
            FunctionOp::Constant => {
                vec![Some(Value::Float(self.param_f64("value")))]
            }
            FunctionOp::Gain => {
                let v = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                vec![v.map(|x| Value::Float(x * self.param_f64("gain")))]
            }
            FunctionOp::Add => {
                let a = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                let result = match (a, b) {
                    (Some(a), Some(b)) => Some(a + b),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                vec![result.map(Value::Float)]
            }
            FunctionOp::Multiply => {
                let a = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                vec![match (a, b) {
                    (Some(a), Some(b)) => Some(Value::Float(a * b)),
                    _ => None,
                }]
            }
            FunctionOp::Subtract => {
                let a = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                vec![match (a, b) {
                    (Some(a), Some(b)) => Some(Value::Float(a - b)),
                    _ => None,
                }]
            }
            FunctionOp::Clamp => {
                let v = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let min = self.param_f64("min");
                let max = self.param_f64("max");
                vec![v.map(|x| Value::Float(x.clamp(min, max)))]
            }
            FunctionOp::Select => {
                let cond = inputs.first().and_then(|i| i.and_then(|v| v.as_float()));
                let a = inputs.get(1).and_then(|i| i.and_then(|v| v.as_float()));
                let b = inputs.get(2).and_then(|i| i.and_then(|v| v.as_float()));
                let result = match (cond, a, b) {
                    (Some(c), Some(a), Some(b)) => {
                        Some(Value::Float(if c > 0.0 { a } else { b }))
                    }
                    _ => None,
                };
                vec![result]
            }
            FunctionOp::ChannelRead => {
                // In WASM, channel reads return 0.0 (no hardware).
                // Real values come from BSP-bound typed channels at deploy time.
                vec![Some(Value::Float(0.0))]
            }
            FunctionOp::ChannelWrite => {
                // In WASM, channel writes are no-ops.
                vec![]
            }
            FunctionOp::PlotAccum => {
                if let BlockState::PlotBuffer {
                    ref mut buffer,
                    max_samples,
                } = self.state
                {
                    if let Some(v) =
                        inputs.first().and_then(|i| i.and_then(|v| v.as_float()))
                    {
                        buffer.push(v);
                        if buffer.len() > max_samples {
                            buffer.remove(0);
                        }
                    }
                    vec![Some(Value::Series(buffer.clone()))]
                } else {
                    vec![None]
                }
            }
            FunctionOp::JsonEncode => {
                let out = inputs
                    .first()
                    .and_then(|i| *i)
                    .and_then(|v| serde_json::to_string(v).ok())
                    .map(Value::Text);
                vec![out]
            }
            FunctionOp::JsonDecode => {
                let out = inputs
                    .first()
                    .and_then(|i| i.and_then(|v| v.as_text()))
                    .and_then(|text| serde_json::from_str::<Value>(text).ok());
                vec![out]
            }
            FunctionOp::PubSubSource => {
                if let BlockState::PubSubSource { ref current } = self.state {
                    vec![current.clone()]
                } else {
                    vec![None]
                }
            }
            FunctionOp::PubSubSink => {
                if let BlockState::PubSubSink { ref mut last } = self.state {
                    *last = inputs.first().and_then(|v| v.cloned());
                }
                vec![]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_traits::function_def::builtin_function_defs;

    fn find_def(id: &str) -> FunctionDef {
        builtin_function_defs()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap()
    }

    #[test]
    fn constant_block() {
        let def = find_def("constant");
        let mut b = DataDrivenBlock::new(def, r#"{"value": 42.0}"#).unwrap();
        let out = b.tick(&[], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(42.0));
    }

    #[test]
    fn constant_legacy_config() {
        let def = find_def("constant");
        let mut b = DataDrivenBlock::new(def, r#"{"value": 5.0}"#).unwrap();
        assert_eq!(b.name(), "Constant");
        assert_eq!(b.block_type(), "constant");
        let out = b.tick(&[], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(5.0));
    }

    #[test]
    fn gain_block() {
        let def = find_def("gain");
        let mut b = DataDrivenBlock::new(def, r#"{"gain": 3.0}"#).unwrap();
        let input = Value::Float(4.0);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(12.0));
    }

    #[test]
    fn gain_legacy_param1() {
        let def = find_def("gain");
        let mut b =
            DataDrivenBlock::new(def, r#"{"op":"Gain","param1":2.0}"#).unwrap();
        let input = Value::Float(5.0);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(10.0));
    }

    #[test]
    fn add_block() {
        let def = find_def("add");
        let mut b = DataDrivenBlock::new(def, "{}").unwrap();
        let a = Value::Float(2.0);
        let bv = Value::Float(3.0);
        let out = b.tick(&[Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(5.0));
    }

    #[test]
    fn multiply_block() {
        let def = find_def("multiply");
        let mut b = DataDrivenBlock::new(def, "{}").unwrap();
        let a = Value::Float(3.0);
        let bv = Value::Float(4.0);
        let out = b.tick(&[Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(12.0));
    }

    #[test]
    fn clamp_block() {
        let def = find_def("clamp");
        let mut b = DataDrivenBlock::new(def, r#"{"min": 0.0, "max": 10.0}"#).unwrap();
        let input = Value::Float(15.0);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(10.0));
    }

    #[test]
    fn clamp_legacy_params() {
        let def = find_def("clamp");
        let mut b = DataDrivenBlock::new(
            def,
            r#"{"op":"Clamp","param1":0.0,"param2":1.0}"#,
        )
        .unwrap();
        let input = Value::Float(0.5);
        let out = b.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(0.5));
    }

    #[test]
    fn select_block() {
        let def = find_def("select");
        let mut b = DataDrivenBlock::new(def, "{}").unwrap();
        let cond = Value::Float(1.0);
        let a = Value::Float(10.0);
        let bv = Value::Float(20.0);
        let out = b.tick(&[Some(&cond), Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(10.0));

        let cond_false = Value::Float(0.0);
        let out2 = b.tick(&[Some(&cond_false), Some(&a), Some(&bv)], 0.01);
        assert_eq!(out2[0].as_ref().unwrap().as_float(), Some(20.0));
    }

    #[test]
    fn subtract_block() {
        let def = find_def("subtract");
        let mut b = DataDrivenBlock::new(def, "{}").unwrap();
        let a = Value::Float(10.0);
        let bv = Value::Float(3.0);
        let out = b.tick(&[Some(&a), Some(&bv)], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(7.0));
    }

    #[test]
    fn plot_accumulates() {
        let def = find_def("plot");
        let mut b = DataDrivenBlock::new(def, r#"{"max_samples": 3}"#).unwrap();
        for i in 0..5 {
            let v = Value::Float(i as f64);
            b.tick(&[Some(&v)], 0.01);
        }
        let out = b.tick(&[Some(&Value::Float(5.0))], 0.01);
        let series = out[0].as_ref().unwrap().as_series().unwrap();
        assert_eq!(series, &[3.0, 4.0, 5.0]);
    }

    #[test]
    fn json_roundtrip() {
        let enc_def = find_def("json_encode");
        let dec_def = find_def("json_decode");
        let mut enc = DataDrivenBlock::new(enc_def, "{}").unwrap();
        let mut dec = DataDrivenBlock::new(dec_def, "{}").unwrap();

        let input = Value::Float(1.234);
        let encoded = enc.tick(&[Some(&input)], 0.01);
        let text = encoded[0].as_ref().unwrap();
        let decoded = dec.tick(&[Some(text)], 0.01);
        assert_eq!(decoded[0].as_ref().unwrap().as_float(), Some(1.234));
    }

    #[test]
    fn channel_read_returns_zero() {
        let def = find_def("channel_read");
        let mut b = DataDrivenBlock::new(def, r#"{"channel":"adc0"}"#).unwrap();
        let out = b.tick(&[], 0.01);
        assert_eq!(out[0].as_ref().unwrap().as_float(), Some(0.0));
    }

    #[test]
    fn channel_write_is_noop() {
        let def = find_def("channel_write");
        let mut b = DataDrivenBlock::new(def, r#"{"channel":"pwm0"}"#).unwrap();
        let out = b.tick(&[Some(&Value::Float(0.5))], 0.01);
        assert!(out.is_empty());
    }

    #[test]
    fn pubsub_source_and_sink() {
        let src_def = find_def("pubsub_source");
        let sink_def = find_def("pubsub_sink");
        let mut src = DataDrivenBlock::new(src_def, r#"{"topic":"t","port_kind":"Float"}"#).unwrap();
        let mut sink = DataDrivenBlock::new(sink_def, r#"{"topic":"t","port_kind":"Float"}"#).unwrap();

        // Source initially empty
        let out = src.tick(&[], 0.01);
        assert_eq!(out[0], None);

        // Sink stores value
        let v = Value::Float(42.0);
        let sink_out = sink.tick(&[Some(&v)], 0.01);
        assert!(sink_out.is_empty());
    }

    #[test]
    fn config_json_roundtrip() {
        let def = find_def("gain");
        let b = DataDrivenBlock::new(def, r#"{"gain": 2.5}"#).unwrap();
        let json = b.config_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["gain"], 2.5);
    }

    #[test]
    fn module_trait_accessors() {
        let def = find_def("add");
        let mut b = DataDrivenBlock::new(def, "{}").unwrap();
        assert_eq!(b.name(), "Add");
        assert_eq!(b.block_type(), "add");
        assert_eq!(b.input_ports().len(), 2);
        assert_eq!(b.output_ports().len(), 1);
        assert!(b.as_tick().is_some());
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
    }

    #[test]
    fn new_with_invalid_json_returns_error() {
        let def = find_def("constant");
        let result = DataDrivenBlock::new(def, "not valid json");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            err.contains("invalid config JSON"),
            "error should mention invalid config JSON, got: {err}"
        );
    }

    #[test]
    fn config_json_includes_string_params() {
        // channel_read has string params: "channel" and "peripheral"
        let def = find_def("channel_read");
        let b = DataDrivenBlock::new(def, r#"{"channel":"adc0","peripheral":"ADC1"}"#).unwrap();
        let json = b.config_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["channel"], "adc0");
        assert_eq!(parsed["peripheral"], "ADC1");
    }

    #[test]
    fn config_json_includes_pubsub_string_params() {
        // pubsub_source has string params: "topic" and "port_kind"
        let def = find_def("pubsub_source");
        let b = DataDrivenBlock::new(def, r#"{"topic":"my_topic","port_kind":"Text"}"#).unwrap();
        let json = b.config_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["topic"], "my_topic");
        assert_eq!(parsed["port_kind"], "Text");
    }

    #[test]
    fn bool_param_kind_handling() {
        use module_traits::function_def::*;
        use module_traits::PortKind;

        // Build a custom FunctionDef with a Bool param
        let def = FunctionDef {
            id: "test_bool".into(),
            display_name: "Test Bool".into(),
            category: "Test".into(),
            op: FunctionOp::Constant,
            inputs: vec![],
            outputs: vec![FuncPortDef::new("out", PortKind::Float)],
            params: vec![
                ParamDef::float("value", 0.0),
                ParamDef {
                    name: "enabled".into(),
                    kind: ParamKind::Bool,
                    default: "true".into(),
                },
            ],
            mlir_op: None,
        };

        // Test bool with explicit value
        let b = DataDrivenBlock::new(def.clone(), r#"{"enabled": false}"#).unwrap();
        assert_eq!(b.param_f64("enabled"), 0.0);

        // Test bool with default (true)
        let b2 = DataDrivenBlock::new(def.clone(), r#"{}"#).unwrap();
        assert_eq!(b2.param_f64("enabled"), 1.0);

        // Test bool with explicit true
        let b3 = DataDrivenBlock::new(def, r#"{"enabled": true}"#).unwrap();
        assert_eq!(b3.param_f64("enabled"), 1.0);
    }

    #[test]
    fn int_param_kind_handling() {
        // channel_read uses int params indirectly through FunctionDef params.
        // Let's build a custom def with Int kind.
        use module_traits::function_def::*;
        use module_traits::PortKind;

        let def = FunctionDef {
            id: "test_int".into(),
            display_name: "Test Int".into(),
            category: "Test".into(),
            op: FunctionOp::Constant,
            inputs: vec![],
            outputs: vec![FuncPortDef::new("out", PortKind::Float)],
            params: vec![
                ParamDef::float("value", 0.0),
                ParamDef::int("count", 5),
            ],
            mlir_op: None,
        };

        let b = DataDrivenBlock::new(def.clone(), r#"{"count": 10}"#).unwrap();
        assert_eq!(b.param_f64("count"), 10.0);

        // Default value
        let b2 = DataDrivenBlock::new(def, r#"{}"#).unwrap();
        assert_eq!(b2.param_f64("count"), 5.0);
    }
}
