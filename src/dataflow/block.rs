//! Core block trait and value types for the dataflow graph.

use serde::{Deserialize, Serialize};

/// Opaque block identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u32);

/// The kinds of data that can flow through a port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortKind {
    Float,
    Bytes,
    Text,
    Series,
    Any,
}

/// Metadata for a single port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDef {
    pub name: String,
    pub kind: PortKind,
}

impl PortDef {
    pub fn new(name: &str, kind: PortKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
        }
    }
}

/// A value flowing through a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Value {
    Float(f64),
    Bytes(Vec<u8>),
    Text(String),
    Series(Vec<f64>),
}

impl Value {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_series(&self) -> Option<&[f64]> {
        match self {
            Value::Series(s) => Some(s),
            _ => None,
        }
    }

    pub fn kind(&self) -> PortKind {
        match self {
            Value::Float(_) => PortKind::Float,
            Value::Bytes(_) => PortKind::Bytes,
            Value::Text(_) => PortKind::Text,
            Value::Series(_) => PortKind::Series,
        }
    }
}

/// A processing node in the dataflow graph.
///
/// Each tick, the graph delivers input values and collects outputs.
pub trait Block {
    /// Human-readable block name (e.g. "Constant", "Gain").
    fn name(&self) -> &str;

    /// Block type identifier used for serialization.
    fn block_type(&self) -> &str;

    /// Input port definitions.
    fn input_ports(&self) -> Vec<PortDef>;

    /// Output port definitions.
    fn output_ports(&self) -> Vec<PortDef>;

    /// Process one tick.  `inputs` is indexed by input port order.
    /// Returns one `Option<Value>` per output port.
    fn tick(&mut self, inputs: &[Option<&Value>], dt: f64) -> Vec<Option<Value>>;

    /// Serialise block-specific config to JSON.
    fn config_json(&self) -> String {
        "{}".to_string()
    }
}
