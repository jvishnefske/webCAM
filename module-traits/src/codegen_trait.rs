//! The `Codegen` trait — custom code emission for embedded targets.

use alloc::string::String;

/// Custom code emission for embedded targets.
pub trait Codegen {
    /// Emit Rust source code for the given target family.
    ///
    /// `target` is the target family name (e.g. "host", "rp2040", "stm32f4", "esp32c3").
    fn emit_rust(&self, target: &str) -> Result<String, String>;
}

#[cfg(test)]
mod tests {
    use super::Codegen;
    use alloc::format;
    use alloc::string::String;

    struct MockCodegen;

    impl Codegen for MockCodegen {
        fn emit_rust(&self, target: &str) -> Result<String, String> {
            match target {
                "host" => Ok(String::from("fn tick() { /* host */ }")),
                "rp2040" => Ok(String::from("fn tick() { /* rp2040 */ }")),
                other => Err(format!("unsupported target: {other}")),
            }
        }
    }

    #[test]
    fn test_codegen_emit_rust_host() {
        let cg = MockCodegen;
        let result = cg.emit_rust("host");
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("host"), "expected host-specific code");
        assert!(code.contains("fn tick()"), "expected a tick function");
    }

    #[test]
    fn test_codegen_emit_rust_rp2040() {
        let cg = MockCodegen;
        let result = cg.emit_rust("rp2040");
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("rp2040"), "expected rp2040-specific code");
        assert_ne!(
            code,
            cg.emit_rust("host").unwrap(),
            "targets should produce different code"
        );
    }

    #[test]
    fn test_codegen_emit_rust_unsupported_target() {
        let cg = MockCodegen;
        let result = cg.emit_rust("avr");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("unsupported target"),
            "error should mention unsupported target"
        );
        assert!(err.contains("avr"), "error should mention the target name");
    }
}
