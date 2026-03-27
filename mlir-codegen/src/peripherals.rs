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
    writeln!(out, "// Maps the dataflow-rt Peripherals trait to C externs.").unwrap();
    writeln!(out, "//").unwrap();
    writeln!(out, "// Each target's main.rs provides #[no_mangle] extern \"C\"").unwrap();
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

/// Generate a Rust FFI module that declares the C tick function and State struct.
///
/// This is the Rust-side counterpart: `logic/src/ffi.rs` in the generated workspace.
pub fn generate_ffi_rs(state_fields: &[(String, &str)]) -> String {
    let mut out = String::with_capacity(512);

    writeln!(out, "//! FFI bridge to the MLIR-generated C tick function.").unwrap();
    writeln!(out).unwrap();

    // State struct — #[repr(C)] for ABI compatibility
    writeln!(out, "#[repr(C)]").unwrap();
    writeln!(out, "pub struct State {{").unwrap();
    for (name, ty) in state_fields {
        writeln!(out, "    pub {name}: {ty},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "impl Default for State {{").unwrap();
    writeln!(out, "    fn default() -> Self {{").unwrap();
    writeln!(out, "        // SAFETY: State is all-zero-initializable (f64 fields)").unwrap();
    writeln!(out, "        unsafe {{ core::mem::zeroed() }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "extern \"C\" {{").unwrap();
    writeln!(out, "    /// MLIR-generated tick function.").unwrap();
    writeln!(out, "    pub fn tick(state: *mut State);").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "/// Safe wrapper around the C tick function.").unwrap();
    writeln!(out, "pub fn tick_safe(state: &mut State) {{").unwrap();
    writeln!(out, "    // SAFETY: state is a valid mutable reference").unwrap();
    writeln!(out, "    unsafe {{ tick(state as *mut State) }}").unwrap();
    writeln!(out, "}}").unwrap();

    out
}

/// Return the list of `#[no_mangle] extern "C"` function stubs that a target
/// `main.rs` must provide. Each entry is `(fn_name, signature, default_body)`.
pub fn hw_extern_stubs() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("hw_adc_read", "extern \"C\" fn hw_adc_read(channel: u8) -> f32", "0.0"),
        ("hw_pwm_write", "extern \"C\" fn hw_pwm_write(channel: u8, duty: f32)", ""),
        ("hw_gpio_read", "extern \"C\" fn hw_gpio_read(pin: u8) -> bool", "false"),
        ("hw_gpio_write", "extern \"C\" fn hw_gpio_write(pin: u8, high: bool)", ""),
        ("hw_uart_write", "extern \"C\" fn hw_uart_write(port: u8, data: *const u8, len: usize)", ""),
        ("hw_uart_read", "extern \"C\" fn hw_uart_read(port: u8, buf: *mut u8, buf_len: usize) -> usize", "0"),
        ("hw_encoder_read", "extern \"C\" fn hw_encoder_read(channel: u8) -> i64", "0"),
        ("hw_display_write", "extern \"C\" fn hw_display_write(bus: u8, addr: u8, l1: *const core::ffi::c_char, l2: *const core::ffi::c_char)", ""),
        ("hw_stepper_move", "extern \"C\" fn hw_stepper_move(port: u8, target: i64)", ""),
        ("hw_stepper_position", "extern \"C\" fn hw_stepper_position(port: u8) -> i64", "0"),
        ("hw_stepper_enable", "extern \"C\" fn hw_stepper_enable(port: u8, enabled: bool)", ""),
        ("hw_stallguard_read", "extern \"C\" fn hw_stallguard_read(port: u8, addr: u8) -> u16", "0"),
    ]
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
        assert!(rs.contains("#[repr(C)]"));
        assert!(rs.contains("pub out_1_p0: f64"));
        assert!(rs.contains("pub fn tick_safe"));
    }

    #[test]
    fn hw_stubs_count() {
        assert_eq!(hw_extern_stubs().len(), 12);
    }
}
