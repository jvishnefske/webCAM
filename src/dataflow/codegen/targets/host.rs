//! Host simulation target generator.

use std::fmt::Write;

use super::TargetGenerator;
use crate::dataflow::codegen::binding::Binding;
use crate::dataflow::graph::GraphSnapshot;

pub struct HostGenerator;

impl TargetGenerator for HostGenerator {
    fn generate(
        &self,
        _snap: &GraphSnapshot,
        _binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String> {
        let cargo_toml = generate_cargo_toml();
        let main_rs = generate_main_rs(dt);
        Ok(vec![
            ("target-host/Cargo.toml".to_string(), cargo_toml),
            ("target-host/src/main.rs".to_string(), main_rs),
        ])
    }
}

fn generate_cargo_toml() -> String {
    r#"[package]
name = "target-host"
version = "0.1.0"
edition = "2021"

[dependencies]
logic = { path = "../logic" }
dataflow-rt = { path = "../dataflow-rt", features = ["std"] }
"#
    .to_string()
}

fn generate_main_rs(dt: f64) -> String {
    let mut out = String::new();
    writeln!(out, "//! Generated host simulation target.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use dataflow_rt::Peripherals;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#[derive(Default)]").unwrap();
    writeln!(out, "struct SimPeripherals {{").unwrap();
    writeln!(out, "    adc: [f32; 16],").unwrap();
    writeln!(out, "    pwm: [f32; 16],").unwrap();
    writeln!(out, "    gpio: [bool; 32],").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl Peripherals for SimPeripherals {{").unwrap();
    writeln!(
        out,
        "    fn adc_read(&mut self, ch: u8) -> f32 {{ self.adc[ch as usize] }}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn pwm_write(&mut self, ch: u8, duty: f32) {{ self.pwm[ch as usize] = duty; }}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn gpio_read(&self, pin: u8) -> bool {{ self.gpio[pin as usize] }}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn gpio_write(&mut self, pin: u8, high: bool) {{ self.gpio[pin as usize] = high; }}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn uart_write(&mut self, _port: u8, _data: &[u8]) {{}}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn uart_read(&mut self, _port: u8, _buf: &mut [u8]) -> usize {{ 0 }}"
    )
    .unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Global peripherals instance for C-FFI path
    writeln!(out, "static mut HW: SimPeripherals = SimPeripherals {{").unwrap();
    writeln!(out, "    adc: [0.0; 16],").unwrap();
    writeln!(out, "    pwm: [0.0; 16],").unwrap();
    writeln!(out, "    gpio: [false; 32],").unwrap();
    writeln!(out, "}};").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "fn main() {{").unwrap();
    writeln!(out, "    let mut hw = SimPeripherals::default();").unwrap();
    writeln!(out, "    let mut state = logic::State::default();").unwrap();
    writeln!(out, "    loop {{").unwrap();
    writeln!(out, "        logic::tick(&mut hw, &mut state);").unwrap();
    writeln!(
        out,
        "        std::thread::sleep(std::time::Duration::from_secs_f64({dt}));"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    // Append C-FFI hw_* stubs
    out.push_str(&super::generate_hw_ffi_stubs("HW"));

    out
}
