//! MLIR dialect ops, type strings, and attribute formatting.
//!
//! Math operations use the standard MLIR `arith` dialect.
//! I/O operations use `func.call` to named channel functions.
//! Only truly hardware-specific ops remain in a minimal `builtin` namespace.

/// MLIR namespace for hardware-specific ops not covered by standard dialects.
pub const DIALECT: &str = "builtin";

// -- Standard MLIR dialect ops (arith, func) --------------------------------

/// `arith.constant` — replaces the old `dataflow.constant`.
pub const OP_CONSTANT: &str = "arith.constant";
/// `arith.mulf` — gain is mul by a constant factor.
pub const OP_GAIN: &str = "arith.mulf";
/// `arith.addf`
pub const OP_ADD: &str = "arith.addf";
/// `arith.mulf` (two-input multiply)
pub const OP_MUL: &str = "arith.mulf";
/// `arith.subf`
pub const OP_SUB: &str = "arith.subf";
/// Clamp is compound: `arith.maximumf(min, arith.minimumf(max, x))`
pub const OP_CLAMP: &str = "arith.maximumf";

// -- I/O: modeled as func.call to named channel functions -------------------

/// `func.call @adc_read_{ch}` — read from a typed ADC channel.
pub const OP_ADC_READ: &str = "func.call";
/// `func.call @pwm_write_{ch}` — write to a typed PWM channel.
pub const OP_PWM_WRITE: &str = "func.call";
/// `func.call @gpio_read_{pin}` — read GPIO pin.
pub const OP_GPIO_READ: &str = "func.call";
/// `func.call @gpio_write_{pin}` — write GPIO pin.
pub const OP_GPIO_WRITE: &str = "func.call";
/// `func.call @uart_tx_{port}` — transmit on UART.
pub const OP_UART_TX: &str = "func.call";
/// `func.call @uart_rx_{port}` — receive from UART.
pub const OP_UART_RX: &str = "func.call";
/// `func.call @encoder_read_{ch}` — read encoder position.
pub const OP_ENCODER_READ: &str = "func.call";
/// `func.call @display_write_{bus}_{addr}` — write to display.
pub const OP_DISPLAY_WRITE: &str = "func.call";
/// `func.call @stepper_move_{port}` — move stepper motor.
pub const OP_STEPPER_MOVE: &str = "func.call";
/// `func.call @stepper_enable_{port}` — enable stepper.
pub const OP_STEPPER_ENABLE: &str = "func.call";
/// `func.call @stepper_position_{port}` — read stepper position.
pub const OP_STEPPER_POSITION: &str = "func.call";
/// `func.call @stallguard_read_{port}` — read stallguard value.
pub const OP_STALLGUARD_READ: &str = "func.call";
/// `func.call @subscribe_{topic}` — subscribe to pub/sub topic.
pub const OP_SUBSCRIBE: &str = "func.call";
/// `func.call @publish_{topic}` — publish to pub/sub topic.
pub const OP_PUBLISH: &str = "func.call";
/// `scf.execute_region` — state machine uses structured control flow.
pub const OP_STATE_MACHINE: &str = "scf.execute_region";

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
