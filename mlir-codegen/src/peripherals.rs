//! Generate `peripherals.h` C header from the `Peripherals` trait.
//!
//! Each trait method becomes an `extern` C function declaration. Target
//! `main.rs` files provide `#[no_mangle] extern "C"` implementations that
//! delegate to the actual HAL.

use std::fmt::Write;

/// C function signature for a peripheral extern declaration.
struct PeripheralFn {
    name: &'static str,
    return_type: &'static str,
    params: &'static str,
}

const PERIPHERAL_FNS: &[PeripheralFn] = &[
    PeripheralFn {
        name: "hw_adc_read",
        return_type: "float",
        params: "uint8_t channel",
    },
    PeripheralFn {
        name: "hw_pwm_write",
        return_type: "void",
        params: "uint8_t channel, float duty",
    },
    PeripheralFn {
        name: "hw_gpio_read",
        return_type: "bool",
        params: "uint8_t pin",
    },
    PeripheralFn {
        name: "hw_gpio_write",
        return_type: "void",
        params: "uint8_t pin, bool high",
    },
    PeripheralFn {
        name: "hw_uart_write",
        return_type: "void",
        params: "uint8_t port, const uint8_t* data, size_t len",
    },
    PeripheralFn {
        name: "hw_uart_read",
        return_type: "size_t",
        params: "uint8_t port, uint8_t* buf, size_t buf_len",
    },
    PeripheralFn {
        name: "hw_encoder_read",
        return_type: "int64_t",
        params: "uint8_t channel",
    },
    PeripheralFn {
        name: "hw_display_write",
        return_type: "void",
        params: "uint8_t bus, uint8_t addr, const char* l1, const char* l2",
    },
    PeripheralFn {
        name: "hw_stepper_move",
        return_type: "void",
        params: "uint8_t port, int64_t target",
    },
    PeripheralFn {
        name: "hw_stepper_position",
        return_type: "int64_t",
        params: "uint8_t port",
    },
    PeripheralFn {
        name: "hw_stepper_enable",
        return_type: "void",
        params: "uint8_t port, bool enabled",
    },
    PeripheralFn {
        name: "hw_stallguard_read",
        return_type: "uint16_t",
        params: "uint8_t port, uint8_t addr",
    },
];

/// Generate the `peripherals.h` C header content.
pub fn generate_peripherals_h() -> String {
    let mut out = String::with_capacity(1024);

    writeln!(out, "// Auto-generated peripheral interface header.").unwrap();
    writeln!(
        out,
        "// Maps the dataflow-rt Peripherals trait to C externs."
    )
    .unwrap();
    writeln!(out, "//").unwrap();
    writeln!(
        out,
        "// Each target's main.rs provides #[no_mangle] extern \"C\""
    )
    .unwrap();
    writeln!(out, "// implementations that delegate to the HAL.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#ifndef PERIPHERALS_H").unwrap();
    writeln!(out, "#define PERIPHERALS_H").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#include <stdint.h>").unwrap();
    writeln!(out, "#include <stddef.h>").unwrap();
    writeln!(out, "#include <stdbool.h>").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#ifdef __cplusplus").unwrap();
    writeln!(out, "extern \"C\" {{").unwrap();
    writeln!(out, "#endif").unwrap();
    writeln!(out).unwrap();

    for pf in PERIPHERAL_FNS {
        writeln!(out, "extern {} {}({});", pf.return_type, pf.name, pf.params).unwrap();
    }

    writeln!(out).unwrap();
    writeln!(out, "#ifdef __cplusplus").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out, "#endif").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#endif // PERIPHERALS_H").unwrap();

    out
}

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
    fn peripherals_h_has_all_functions() {
        let h = generate_peripherals_h();
        assert!(h.contains("#ifndef PERIPHERALS_H"));
        assert!(h.contains("hw_adc_read"));
        assert!(h.contains("hw_pwm_write"));
        assert!(h.contains("hw_gpio_read"));
        assert!(h.contains("hw_gpio_write"));
        assert!(h.contains("hw_uart_write"));
        assert!(h.contains("hw_uart_read"));
        assert!(h.contains("hw_encoder_read"));
        assert!(h.contains("hw_display_write"));
        assert!(h.contains("hw_stepper_move"));
        assert!(h.contains("hw_stepper_position"));
        assert!(h.contains("hw_stepper_enable"));
        assert!(h.contains("hw_stallguard_read"));
    }

    #[test]
    fn peripherals_h_has_12_externs() {
        let h = generate_peripherals_h();
        let count = h.matches("extern ").count();
        // 12 function externs + 1 extern "C" block = 13
        assert!(count >= 12, "expected >=12 externs, got {count}");
    }

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
