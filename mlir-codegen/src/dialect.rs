//! Dataflow MLIR dialect: op names, type strings, and attribute formatting.

/// MLIR dialect namespace prefix.
pub const DIALECT: &str = "dataflow";

// -- Op names ---------------------------------------------------------------

pub const OP_CONSTANT: &str = "dataflow.constant";
pub const OP_GAIN: &str = "dataflow.gain";
pub const OP_ADD: &str = "dataflow.add";
pub const OP_MUL: &str = "dataflow.mul";
pub const OP_SUB: &str = "dataflow.subtract";
pub const OP_CLAMP: &str = "dataflow.clamp";
pub const OP_ADC_READ: &str = "dataflow.adc_read";
pub const OP_PWM_WRITE: &str = "dataflow.pwm_write";
pub const OP_GPIO_READ: &str = "dataflow.gpio_read";
pub const OP_GPIO_WRITE: &str = "dataflow.gpio_write";
pub const OP_UART_TX: &str = "dataflow.uart_tx";
pub const OP_UART_RX: &str = "dataflow.uart_rx";
pub const OP_ENCODER_READ: &str = "dataflow.encoder_read";
pub const OP_DISPLAY_WRITE: &str = "dataflow.display_write";
pub const OP_STEPPER_MOVE: &str = "dataflow.stepper_move";
pub const OP_STEPPER_ENABLE: &str = "dataflow.stepper_enable";
pub const OP_STEPPER_POSITION: &str = "dataflow.stepper_position";
pub const OP_STALLGUARD_READ: &str = "dataflow.stallguard_read";
pub const OP_SUBSCRIBE: &str = "dataflow.subscribe";
pub const OP_PUBLISH: &str = "dataflow.publish";
pub const OP_STATE_MACHINE: &str = "dataflow.state_machine";

// -- MLIR type strings ------------------------------------------------------

pub const MLIR_F64: &str = "f64";
pub const MLIR_F32: &str = "f32";
pub const MLIR_I32: &str = "i32";
pub const MLIR_I64: &str = "i64";
pub const MLIR_I1: &str = "i1";
pub const MLIR_INDEX: &str = "index";
pub const MLIR_MEMREF_F64: &str = "memref<?xf64>";

// -- Attribute formatting ---------------------------------------------------

/// Format a float attribute: `42.0 : f64`.
pub fn float_attr(value: f64) -> String {
    // Ensure we always have a decimal point for MLIR float literals.
    let s = format!("{value}");
    if s.contains('.') || s.contains('e') || s.contains('E') {
        format!("{s} : {MLIR_F64}")
    } else {
        format!("{s}.0 : {MLIR_F64}")
    }
}

/// Format an integer attribute: `3 : i32`.
pub fn i32_attr(value: i32) -> String {
    format!("{value} : {MLIR_I32}")
}

/// Format an integer attribute: `3 : i64`.
pub fn i64_attr(value: i64) -> String {
    format!("{value} : {MLIR_I64}")
}

/// Format a string attribute: `"topic_name"`.
pub fn string_attr(value: &str) -> String {
    format!("\"{value}\"")
}

/// Format an SSA value name: `%v{id}_p{port}`.
pub fn ssa_name(block_id: u32, port: usize) -> String {
    format!("%v{block_id}_p{port}")
}

/// Format a state memref argument: `%state`.
pub fn state_arg() -> &'static str {
    "%state"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_attr_decimal() {
        assert_eq!(float_attr(42.0), "42.0 : f64");
    }

    #[test]
    fn float_attr_integer_value() {
        // Rust formats 3.0 as "3" in some cases — we must add ".0"
        let result = float_attr(3.0);
        assert!(result.contains('.'), "must contain decimal: {result}");
        assert!(result.ends_with(": f64"));
    }

    #[test]
    fn i32_attr_format() {
        assert_eq!(i32_attr(7), "7 : i32");
    }

    #[test]
    fn i64_attr_format() {
        assert_eq!(i64_attr(42), "42 : i64");
        assert_eq!(i64_attr(-1), "-1 : i64");
    }

    #[test]
    fn string_attr_format() {
        assert_eq!(string_attr("hello"), "\"hello\"");
    }

    #[test]
    fn state_arg_returns_static_str() {
        assert_eq!(state_arg(), "%state");
    }

    #[test]
    fn ssa_name_format() {
        assert_eq!(ssa_name(5, 0), "%v5_p0");
        assert_eq!(ssa_name(12, 2), "%v12_p2");
    }
}
