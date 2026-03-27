//! Target-specific code generators.

pub mod esp32c3;
pub mod host;
pub mod rp2040;
pub mod stm32f4;
pub mod stm32g0b1;

use std::fmt::Write;

use crate::dataflow::codegen::binding::Binding;
use crate::dataflow::codegen::target::TargetFamily;
use crate::dataflow::graph::GraphSnapshot;

/// Trait for target-specific firmware generators.
///
/// Each target generates its own `target-<name>/` subdirectory with
/// Cargo.toml, main.rs, and any target-specific files (memory.x, .cargo/config.toml).
pub trait TargetGenerator {
    fn generate(
        &self,
        snap: &GraphSnapshot,
        binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String>;
}

/// Dispatch to the appropriate target generator.
pub fn generator_for(family: TargetFamily) -> Box<dyn TargetGenerator> {
    match family {
        TargetFamily::Host => Box::new(host::HostGenerator),
        TargetFamily::Rp2040 => Box::new(rp2040::Rp2040Generator),
        TargetFamily::Stm32f4 => Box::new(stm32f4::Stm32f4Generator),
        TargetFamily::Esp32c3 => Box::new(esp32c3::Esp32c3Generator),
        TargetFamily::Stm32g0b1 => Box::new(stm32g0b1::Stm32g0b1Generator),
    }
}

/// Generate `#[no_mangle] extern "C"` hw_* function stubs for the MLIR C-FFI path.
///
/// These functions delegate to a static `HwPeripherals` instance. Each target
/// generator can append this to its `main.rs` when using the MLIR backend.
///
/// The `hw_var` parameter is the name of the global peripherals variable
/// (e.g., `"HW"` for `static mut HW: ...`).
pub fn generate_hw_ffi_stubs(hw_var: &str) -> String {
    let mut out = String::new();
    writeln!(out).unwrap();
    writeln!(out, "// ---------------------------------------------------------------------------").unwrap();
    writeln!(out, "// C-FFI peripheral stubs for MLIR EmitC backend").unwrap();
    writeln!(out, "// ---------------------------------------------------------------------------").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_adc_read(channel: u8) -> f32 {{").unwrap();
    writeln!(out, "    // SAFETY: single-threaded embedded context").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.adc_read(channel) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_pwm_write(channel: u8, duty: f32) {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.pwm_write(channel, duty) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_gpio_read(pin: u8) -> bool {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.gpio_read(pin) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_gpio_write(pin: u8, high: bool) {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.gpio_write(pin, high) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_uart_write(port: u8, data: *const u8, len: usize) {{").unwrap();
    writeln!(out, "    let slice = unsafe {{ core::slice::from_raw_parts(data, len) }};").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.uart_write(port, slice) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_uart_read(port: u8, buf: *mut u8, buf_len: usize) -> usize {{").unwrap();
    writeln!(out, "    let slice = unsafe {{ core::slice::from_raw_parts_mut(buf, buf_len) }};").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.uart_read(port, slice) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_encoder_read(channel: u8) -> i64 {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.encoder_read(channel) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_display_write(bus: u8, addr: u8, l1: *const core::ffi::c_char, l2: *const core::ffi::c_char) {{").unwrap();
    writeln!(out, "    let s1 = if l1.is_null() {{ \"\" }} else {{ unsafe {{ core::ffi::CStr::from_ptr(l1) }}.to_str().unwrap_or(\"\") }};").unwrap();
    writeln!(out, "    let s2 = if l2.is_null() {{ \"\" }} else {{ unsafe {{ core::ffi::CStr::from_ptr(l2) }}.to_str().unwrap_or(\"\") }};").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.display_write(bus, addr, s1, s2) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_stepper_move(port: u8, target: i64) {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.stepper_move(port, target) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_stepper_position(port: u8) -> i64 {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.stepper_position(port) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_stepper_enable(port: u8, enabled: bool) {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.stepper_enable(port, enabled) }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[no_mangle]").unwrap();
    writeln!(out, "pub unsafe extern \"C\" fn hw_stallguard_read(port: u8, addr: u8) -> u16 {{").unwrap();
    writeln!(out, "    unsafe {{ {hw_var}.stallguard_read(port, addr) }}").unwrap();
    writeln!(out, "}}").unwrap();

    out
}
