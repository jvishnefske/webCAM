//! Map dataflow PortKind to C type strings for EmitC output.

use module_traits::value::PortKind;

/// Return the C type name for a port kind.
///
/// On embedded targets all non-Float types collapse to `double` since
/// the generated C code operates in a flat numeric domain.
pub fn c_type(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "double",
        // Embedded: no heap, so Bytes/Text/Series/Any → double
        PortKind::Bytes => "double",
        PortKind::Text => "double",
        PortKind::Series => "double",
        PortKind::Any => "double",
        PortKind::Message(_) => "double",
    }
}

/// Return a C default-value expression for a port kind.
pub fn c_default(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "0.0",
        PortKind::Bytes => "0.0",
        PortKind::Text => "0.0",
        PortKind::Series => "0.0",
        PortKind::Any => "0.0",
        PortKind::Message(_) => "0.0",
    }
}

/// Return the MLIR type string for a port kind.
pub fn mlir_type(kind: &PortKind) -> &'static str {
    match kind {
        PortKind::Float => "f64",
        PortKind::Bytes => "f64",
        PortKind::Text => "f64",
        PortKind::Series => "f64",
        PortKind::Any => "f64",
        PortKind::Message(_) => "f64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_maps() {
        assert_eq!(c_type(&PortKind::Float), "double");
        assert_eq!(c_default(&PortKind::Float), "0.0");
        assert_eq!(mlir_type(&PortKind::Float), "f64");
    }

    #[test]
    fn non_float_collapse_to_double() {
        for kind in &[
            PortKind::Bytes,
            PortKind::Text,
            PortKind::Series,
            PortKind::Any,
        ] {
            assert_eq!(c_type(kind), "double");
            assert_eq!(c_default(kind), "0.0");
            assert_eq!(mlir_type(kind), "f64");
        }
    }
}
