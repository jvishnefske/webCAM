//! Generate safe Rust modules for the MLIR logic crate.
//!
//! Produces `ffi.rs` with a plain State struct — no `repr(C)`, no
//! `extern "C"`, no `unsafe`.

use std::fmt::Write;

/// Generate a pure-Rust state module (no C FFI).
///
/// Generates `logic/src/ffi.rs` with a plain State struct and Default impl.
/// No `#[repr(C)]`, no `extern "C"`, no `unsafe`.
pub fn generate_ffi_rs(state_fields: &[(String, &str)]) -> String {
    let mut out = String::with_capacity(512);

    writeln!(out, "//! State struct for the dataflow tick function.").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[derive(Default)]").unwrap();
    writeln!(out, "pub struct State {{").unwrap();
    for (name, ty) in state_fields {
        writeln!(out, "    pub {name}: {ty},").unwrap();
    }
    writeln!(out, "}}").unwrap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_rs_has_state_struct() {
        let fields = vec![
            ("out_1_p0".to_string(), "f64"),
            ("out_2_p0".to_string(), "f64"),
        ];
        let rs = generate_ffi_rs(&fields);
        assert!(rs.contains("#[derive(Default)]"));
        assert!(rs.contains("pub out_1_p0: f64"));
        assert!(!rs.contains("unsafe"), "ffi.rs must not contain unsafe");
        assert!(!rs.contains("repr(C)"), "ffi.rs must not use repr(C)");
        assert!(!rs.contains("extern"), "ffi.rs must not use extern");
    }
}
