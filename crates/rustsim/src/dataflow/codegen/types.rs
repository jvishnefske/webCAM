//! Map dataflow port kinds to Rust type strings.

use crate::dataflow::block::PortKind;

/// Return the Rust type name corresponding to a port kind.
pub fn rust_type(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "f64",
        PortKind::Bytes => "Vec<u8>",
        PortKind::Text => "String",
        PortKind::Series => "Vec<f64>",
        PortKind::Any => "serde_json::Value",
    }
}

/// Return a Rust default-value expression for a port kind.
pub fn rust_default(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "0.0_f64",
        PortKind::Bytes => "Vec::new()",
        PortKind::Text => "String::new()",
        PortKind::Series => "Vec::new()",
        PortKind::Any => "serde_json::Value::Null",
    }
}

/// Return the Rust type name for no_std contexts.
pub fn rust_type_no_std(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "f64",
        // no_std: no Vec/String, use fixed-size alternatives
        PortKind::Bytes => "f64",
        PortKind::Text => "f64",
        PortKind::Series => "f64",
        PortKind::Any => "f64",
    }
}

/// Return a Rust default-value expression for no_std contexts.
pub fn rust_default_no_std(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "0.0_f64",
        PortKind::Bytes => "0.0_f64",
        PortKind::Text => "0.0_f64",
        PortKind::Series => "0.0_f64",
        PortKind::Any => "0.0_f64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_type_and_default() {
        assert_eq!(rust_type(&PortKind::Float), "f64");
        assert_eq!(rust_default(&PortKind::Float), "0.0_f64");
    }

    #[test]
    fn bytes_type_and_default() {
        assert_eq!(rust_type(&PortKind::Bytes), "Vec<u8>");
        assert_eq!(rust_default(&PortKind::Bytes), "Vec::new()");
    }

    #[test]
    fn text_type_and_default() {
        assert_eq!(rust_type(&PortKind::Text), "String");
        assert_eq!(rust_default(&PortKind::Text), "String::new()");
    }

    #[test]
    fn series_type_and_default() {
        assert_eq!(rust_type(&PortKind::Series), "Vec<f64>");
        assert_eq!(rust_default(&PortKind::Series), "Vec::new()");
    }

    #[test]
    fn any_type_and_default() {
        assert_eq!(rust_type(&PortKind::Any), "serde_json::Value");
        assert_eq!(rust_default(&PortKind::Any), "serde_json::Value::Null");
    }
}
