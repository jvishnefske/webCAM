//! Data-driven function definitions for the dataflow engine.
//!
//! A [`FunctionDef`] is a schema that fully describes a block's identity,
//! ports, configuration parameters, and evaluation semantics.  Instead of
//! one Rust struct per block type, the engine stores a flat registry of
//! `FunctionDef` values and instantiates generic [`super::Module`] impls
//! from them.
//!
//! The WASM layer exposes the full registry to JavaScript so the frontend
//! can build its palette, config panels, and validation from the schema
//! alone — no hardcoded type mirrors needed on the JS side.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::value::PortKind;

/// How a function computes its outputs from inputs and params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FunctionOp {
    /// out = constant value (param "value")
    Constant,
    /// out = in * param("gain")
    Gain,
    /// out = a + b
    Add,
    /// out = a * b
    Multiply,
    /// out = a - b
    Subtract,
    /// out = clamp(in, param("min"), param("max"))
    Clamp,
    /// out = select(cond, a, b) — if cond > 0 then a else b
    Select,
    /// Hardware read — bound to a typed channel at deploy time
    ChannelRead,
    /// Hardware write — bound to a typed channel at deploy time
    ChannelWrite,
    /// Accumulate float samples into a series (UI-only visualization)
    PlotAccum,
    /// Encode value to JSON text
    JsonEncode,
    /// Decode JSON text to value
    JsonDecode,
    /// Pub/sub source — reads from a named topic
    PubSubSource,
    /// Pub/sub sink — writes to a named topic
    PubSubSink,
}

/// A parameter definition for a function's config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    /// Parameter name (e.g., "gain", "channel", "topic").
    pub name: String,
    /// The kind of value this parameter takes.
    pub kind: ParamKind,
    /// Default value as a JSON-compatible string.
    pub default: String,
}

/// The type of a configuration parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamKind {
    Float,
    Int,
    String,
    Bool,
}

/// Port definition within a function def.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncPortDef {
    pub name: String,
    pub kind: PortKind,
}

/// A complete, data-driven function definition.
///
/// This is the single source of truth for a block type's schema.
/// The WASM layer serializes the full registry to JS so the frontend
/// can build palette, config panels, and validation without
/// hardcoding block-specific knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDef {
    /// Unique type identifier (e.g., "constant", "gain", "adc_read").
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Category for palette grouping (e.g., "Math", "Sources", "I/O").
    pub category: String,
    /// The operation this function performs.
    pub op: FunctionOp,
    /// Input port definitions.
    pub inputs: Vec<FuncPortDef>,
    /// Output port definitions.
    pub outputs: Vec<FuncPortDef>,
    /// Configurable parameters with defaults.
    pub params: Vec<ParamDef>,
    /// MLIR dialect op this maps to (e.g., "arith.addf", "arith.mulf").
    /// Used by codegen to emit the correct MLIR op.
    pub mlir_op: Option<String>,
}

impl FuncPortDef {
    pub fn new(name: &str, kind: PortKind) -> Self {
        Self {
            name: String::from(name),
            kind,
        }
    }
}

impl ParamDef {
    pub fn float(name: &str, default: f64) -> Self {
        Self {
            name: String::from(name),
            kind: ParamKind::Float,
            default: alloc::format!("{default}"),
        }
    }

    pub fn int(name: &str, default: i64) -> Self {
        Self {
            name: String::from(name),
            kind: ParamKind::Int,
            default: alloc::format!("{default}"),
        }
    }

    pub fn string(name: &str, default: &str) -> Self {
        Self {
            name: String::from(name),
            kind: ParamKind::String,
            default: String::from(default),
        }
    }
}

/// Build the complete builtin function registry.
///
/// This is the single source of truth for all block types.
/// WASM serializes this to JS; MLIR codegen reads `mlir_op` from it.
pub fn builtin_function_defs() -> Vec<FunctionDef> {
    alloc::vec![
        // ── Sources ──────────────────────────────────────────
        FunctionDef {
            id: s("constant"),
            display_name: s("Constant"),
            category: s("Sources"),
            op: FunctionOp::Constant,
            inputs: alloc::vec![],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![ParamDef::float("value", 1.0)],
            mlir_op: Some(s("arith.constant")),
        },
        // ── Math ─────────────────────────────────────────────
        FunctionDef {
            id: s("gain"),
            display_name: s("Gain"),
            category: s("Math"),
            op: FunctionOp::Gain,
            inputs: alloc::vec![FuncPortDef::new("in", PortKind::Float)],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![ParamDef::float("gain", 1.0)],
            mlir_op: Some(s("arith.mulf")),
        },
        FunctionDef {
            id: s("add"),
            display_name: s("Add"),
            category: s("Math"),
            op: FunctionOp::Add,
            inputs: alloc::vec![
                FuncPortDef::new("a", PortKind::Float),
                FuncPortDef::new("b", PortKind::Float),
            ],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![],
            mlir_op: Some(s("arith.addf")),
        },
        FunctionDef {
            id: s("multiply"),
            display_name: s("Multiply"),
            category: s("Math"),
            op: FunctionOp::Multiply,
            inputs: alloc::vec![
                FuncPortDef::new("a", PortKind::Float),
                FuncPortDef::new("b", PortKind::Float),
            ],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![],
            mlir_op: Some(s("arith.mulf")),
        },
        FunctionDef {
            id: s("subtract"),
            display_name: s("Subtract"),
            category: s("Math"),
            op: FunctionOp::Subtract,
            inputs: alloc::vec![
                FuncPortDef::new("a", PortKind::Float),
                FuncPortDef::new("b", PortKind::Float),
            ],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![],
            mlir_op: Some(s("arith.subf")),
        },
        FunctionDef {
            id: s("clamp"),
            display_name: s("Clamp"),
            category: s("Math"),
            op: FunctionOp::Clamp,
            inputs: alloc::vec![FuncPortDef::new("in", PortKind::Float)],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![
                ParamDef::float("min", 0.0),
                ParamDef::float("max", 1.0),
            ],
            mlir_op: None, // clamp = max(min, min(max, x)) — compound arith
        },
        FunctionDef {
            id: s("select"),
            display_name: s("Select"),
            category: s("Math"),
            op: FunctionOp::Select,
            inputs: alloc::vec![
                FuncPortDef::new("cond", PortKind::Float),
                FuncPortDef::new("a", PortKind::Float),
                FuncPortDef::new("b", PortKind::Float),
            ],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Float)],
            params: alloc::vec![],
            mlir_op: Some(s("arith.select")),
        },
        // ── I/O Channels (bound to BSP at deploy time) ──────
        FunctionDef {
            id: s("channel_read"),
            display_name: s("Channel Read"),
            category: s("I/O"),
            op: FunctionOp::ChannelRead,
            inputs: alloc::vec![],
            outputs: alloc::vec![FuncPortDef::new("value", PortKind::Float)],
            params: alloc::vec![
                ParamDef::string("channel", ""),
                ParamDef::string("peripheral", ""),
            ],
            mlir_op: Some(s("func.call")),
        },
        FunctionDef {
            id: s("channel_write"),
            display_name: s("Channel Write"),
            category: s("I/O"),
            op: FunctionOp::ChannelWrite,
            inputs: alloc::vec![FuncPortDef::new("value", PortKind::Float)],
            outputs: alloc::vec![],
            params: alloc::vec![
                ParamDef::string("channel", ""),
                ParamDef::string("peripheral", ""),
            ],
            mlir_op: Some(s("func.call")),
        },
        // ── Visualization ────────────────────────────────────
        FunctionDef {
            id: s("plot"),
            display_name: s("Plot"),
            category: s("Visualization"),
            op: FunctionOp::PlotAccum,
            inputs: alloc::vec![FuncPortDef::new("in", PortKind::Float)],
            outputs: alloc::vec![FuncPortDef::new("series", PortKind::Series)],
            params: alloc::vec![ParamDef::int("max_samples", 500)],
            mlir_op: None,
        },
        // ── Serialization ────────────────────────────────────
        FunctionDef {
            id: s("json_encode"),
            display_name: s("JSON Encode"),
            category: s("Serialization"),
            op: FunctionOp::JsonEncode,
            inputs: alloc::vec![FuncPortDef::new("in", PortKind::Any)],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Text)],
            params: alloc::vec![],
            mlir_op: None,
        },
        FunctionDef {
            id: s("json_decode"),
            display_name: s("JSON Decode"),
            category: s("Serialization"),
            op: FunctionOp::JsonDecode,
            inputs: alloc::vec![FuncPortDef::new("in", PortKind::Text)],
            outputs: alloc::vec![FuncPortDef::new("out", PortKind::Any)],
            params: alloc::vec![],
            mlir_op: None,
        },
        // ── Pub/Sub ──────────────────────────────────────────
        FunctionDef {
            id: s("pubsub_source"),
            display_name: s("PubSub Source"),
            category: s("Pub/Sub"),
            op: FunctionOp::PubSubSource,
            inputs: alloc::vec![],
            outputs: alloc::vec![FuncPortDef::new("value", PortKind::Float)],
            params: alloc::vec![
                ParamDef::string("topic", "default"),
                ParamDef::string("port_kind", "Float"),
            ],
            mlir_op: Some(s("func.call")),
        },
        FunctionDef {
            id: s("pubsub_sink"),
            display_name: s("PubSub Sink"),
            category: s("Pub/Sub"),
            op: FunctionOp::PubSubSink,
            inputs: alloc::vec![FuncPortDef::new("value", PortKind::Float)],
            outputs: alloc::vec![],
            params: alloc::vec![
                ParamDef::string("topic", "default"),
                ParamDef::string("port_kind", "Float"),
            ],
            mlir_op: Some(s("func.call")),
        },
    ]
}

fn s(v: &str) -> String {
    String::from(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_has_entries() {
        let defs = builtin_function_defs();
        assert!(defs.len() >= 10);
    }

    #[test]
    fn all_ids_unique() {
        let defs = builtin_function_defs();
        let mut ids: Vec<_> = defs.iter().map(|d| &d.id).collect();
        let len_before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), len_before, "duplicate function def ids found");
    }

    #[test]
    fn constant_has_arith_mlir_op() {
        let defs = builtin_function_defs();
        let c = defs.iter().find(|d| d.id == "constant").unwrap();
        assert_eq!(c.mlir_op.as_deref(), Some("arith.constant"));
    }

    #[test]
    fn add_has_arith_addf() {
        let defs = builtin_function_defs();
        let a = defs.iter().find(|d| d.id == "add").unwrap();
        assert_eq!(a.mlir_op.as_deref(), Some("arith.addf"));
    }

    #[test]
    fn channel_read_uses_func_call() {
        let defs = builtin_function_defs();
        let cr = defs.iter().find(|d| d.id == "channel_read").unwrap();
        assert_eq!(cr.mlir_op.as_deref(), Some("func.call"));
    }

    #[test]
    fn serde_roundtrip() {
        let defs = builtin_function_defs();
        let json = serde_json::to_string(&defs).unwrap();
        let restored: Vec<FunctionDef> = serde_json::from_str(&json).unwrap();
        assert_eq!(defs.len(), restored.len());
        assert_eq!(defs[0].id, restored[0].id);
    }

    #[test]
    fn param_def_constructors() {
        let f = ParamDef::float("gain", 2.5);
        assert_eq!(f.name, "gain");
        assert_eq!(f.kind, ParamKind::Float);
        assert_eq!(f.default, "2.5");

        let i = ParamDef::int("channel", 0);
        assert_eq!(i.kind, ParamKind::Int);

        let s = ParamDef::string("topic", "hello");
        assert_eq!(s.kind, ParamKind::String);
        assert_eq!(s.default, "hello");
    }
}
