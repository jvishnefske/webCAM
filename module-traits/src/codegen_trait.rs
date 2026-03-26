//! The `Codegen` trait — custom code emission for embedded targets.

use alloc::string::String;

/// Custom code emission for embedded targets.
pub trait Codegen {
    /// Emit Rust source code for the given target family.
    ///
    /// `target` is the target family name (e.g. "host", "rp2040", "stm32f4", "esp32c3").
    fn emit_rust(&self, target: &str) -> Result<String, String>;
}
