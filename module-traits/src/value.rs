//! Core value types for the dataflow graph.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

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
            name: String::from(name),
            kind,
        }
    }
}

/// A value flowing through a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
