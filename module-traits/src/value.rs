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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    // 1. Value::Float construction
    #[test]
    fn test_value_float_construction() {
        let v = Value::Float(42.0);
        match v {
            Value::Float(f) => assert!((f - 42.0).abs() < f64::EPSILON),
            _ => panic!("expected Value::Float"),
        }
    }

    // 2. Float returns Some from as_float, others return None
    #[test]
    fn test_value_as_float_some() {
        let v = Value::Float(3.25);
        assert_eq!(v.as_float(), Some(3.25));

        // Non-float variants must return None
        assert_eq!(Value::Text(String::from("hi")).as_float(), None);
        assert_eq!(Value::Bytes(vec![1]).as_float(), None);
        assert_eq!(Value::Series(vec![1.0]).as_float(), None);
    }

    // 3. Text.as_float() returns None
    #[test]
    fn test_value_as_float_none_for_text() {
        let v = Value::Text(String::from("not a number"));
        assert_eq!(v.as_float(), None);
    }

    // 4. Text returns Some from as_text
    #[test]
    fn test_value_as_text_some() {
        let v = Value::Text(String::from("hello"));
        assert_eq!(v.as_text(), Some("hello"));
    }

    // 5. Float.as_text() returns None
    #[test]
    fn test_value_as_text_none_for_float() {
        let v = Value::Float(1.0);
        assert_eq!(v.as_text(), None);
    }

    // 6. Bytes returns Some from as_bytes
    #[test]
    fn test_value_as_bytes_some() {
        let v = Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(v.as_bytes(), Some(&[0xDE, 0xAD, 0xBE, 0xEF][..]));
    }

    // 7. Series returns Some from as_series
    #[test]
    fn test_value_as_series_some() {
        let v = Value::Series(vec![1.0, 2.0, 3.0]);
        assert_eq!(v.as_series(), Some(&[1.0, 2.0, 3.0][..]));
    }

    // 8. Float.kind() == PortKind::Float
    #[test]
    fn test_value_kind_float() {
        let v = Value::Float(0.0);
        assert_eq!(v.kind(), PortKind::Float);
    }

    // 9. kind() correct for all 4 variants
    #[test]
    fn test_value_kind_all_variants() {
        assert_eq!(Value::Float(1.0).kind(), PortKind::Float);
        assert_eq!(Value::Bytes(vec![]).kind(), PortKind::Bytes);
        assert_eq!(Value::Text(String::from("")).kind(), PortKind::Text);
        assert_eq!(Value::Series(vec![]).kind(), PortKind::Series);
    }

    // 10. PortDef::new has correct fields
    #[test]
    fn test_port_def_new() {
        let pd = PortDef::new("input", PortKind::Float);
        assert_eq!(pd.name, "input");
        assert_eq!(pd.kind, PortKind::Float);

        let pd2 = PortDef::new("data_out", PortKind::Bytes);
        assert_eq!(pd2.name, "data_out");
        assert_eq!(pd2.kind, PortKind::Bytes);
    }

    // 11. serde_json roundtrip for Float
    #[test]
    fn test_value_serde_roundtrip_float() {
        let original = Value::Float(99.5);
        let json = serde_json::to_string(&original).expect("serialize Float");
        let restored: Value = serde_json::from_str(&json).expect("deserialize Float");
        assert_eq!(original, restored);

        // Verify the tagged representation
        assert!(json.contains("\"type\":\"Float\""));
        assert!(json.contains("\"data\":99.5"));
    }

    // 12. serde_json roundtrip for Text, Bytes, Series
    #[test]
    fn test_value_serde_roundtrip_all_variants() {
        let cases = vec![
            Value::Text(String::from("round-trip")),
            Value::Bytes(vec![1, 2, 3, 255]),
            Value::Series(vec![-1.0, 0.0, 1.0, 2.5]),
        ];

        for original in &cases {
            let json = serde_json::to_string(original).expect("serialize");
            let restored: Value = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(original, &restored);
        }
    }

    // 13. PortKind serde roundtrip
    #[test]
    fn test_port_kind_serde_roundtrip() {
        let kinds = vec![
            PortKind::Float,
            PortKind::Bytes,
            PortKind::Text,
            PortKind::Series,
            PortKind::Any,
        ];

        for kind in &kinds {
            let json = serde_json::to_string(kind).expect("serialize PortKind");
            let restored: PortKind = serde_json::from_str(&json).expect("deserialize PortKind");
            assert_eq!(kind, &restored);
        }
    }
}
