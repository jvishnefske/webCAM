//! STM32F4 Embassy target generator.

use std::fmt::Write;

use super::TargetGenerator;
use crate::dataflow::codegen::binding::Binding;
use crate::dataflow::graph::GraphSnapshot;

pub struct Stm32f4Generator;

impl TargetGenerator for Stm32f4Generator {
    fn generate(
        &self,
        _snap: &GraphSnapshot,
        _binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String> {
        let cargo_toml = generate_cargo_toml();
        let cargo_config = generate_cargo_config();
        let build_rs = generate_build_rs();
        let main_rs = generate_main_rs(dt);

        Ok(vec![
            ("target-stm32f4/Cargo.toml".to_string(), cargo_toml),
            (
                "target-stm32f4/.cargo/config.toml".to_string(),
                cargo_config,
            ),
            ("target-stm32f4/build.rs".to_string(), build_rs),
            ("target-stm32f4/src/main.rs".to_string(), main_rs),
        ])
    }
}

fn generate_cargo_toml() -> String {
    r#"[package]
name = "target-stm32f4"
version = "0.1.0"
edition = "2021"

[dependencies]
logic = { path = "../logic" }
dataflow-rt = { path = "../dataflow-rt", default-features = false }
embassy-executor = { version = "0.7", features = ["arch-cortex-m", "executor-thread"] }
embassy-stm32 = { version = "0.2", features = ["stm32f401cc", "time-driver-tim2"] }
embassy-time = "0.4"
cortex-m = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt = "0.7"
panic-halt = "1"

[profile.release]
opt-level = "z"
lto = true
"#
    .to_string()
}

fn generate_cargo_config() -> String {
    r#"[build]
target = "thumbv7em-none-eabihf"

[target.thumbv7em-none-eabihf]
runner = "probe-rs run --chip STM32F401CCUx"
"#
    .to_string()
}

fn generate_build_rs() -> String {
    r#"fn main() {
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
"#
    .to_string()
}

fn generate_main_rs(dt: f64) -> String {
    let dt_ms = (dt * 1000.0) as u64;

    let mut out = String::new();
    writeln!(out, "#![no_std]").unwrap();
    writeln!(out, "#![no_main]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use embassy_executor::Spawner;").unwrap();
    writeln!(out, "use embassy_time::{{Duration, Ticker}};").unwrap();
    writeln!(out, "use panic_halt as _;").unwrap();
    writeln!(out, "use dataflow_rt::Peripherals;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "struct HwPeripherals {{").unwrap();
    writeln!(out, "    _marker: (),").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl Peripherals for HwPeripherals {{").unwrap();
    writeln!(out, "    fn adc_read(&mut self, _ch: u8) -> f32 {{ 0.0 }}").unwrap();
    writeln!(out, "    fn pwm_write(&mut self, _ch: u8, _duty: f32) {{}}").unwrap();
    writeln!(out, "    fn gpio_read(&self, _pin: u8) -> bool {{ false }}").unwrap();
    writeln!(
        out,
        "    fn gpio_write(&mut self, _pin: u8, _high: bool) {{}}"
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
    writeln!(out, "#[embassy_executor::main]").unwrap();
    writeln!(out, "async fn main(_spawner: Spawner) {{").unwrap();
    writeln!(out, "    let _p = embassy_stm32::init(Default::default());").unwrap();
    writeln!(out, "    let mut hw = HwPeripherals {{ _marker: () }};").unwrap();
    writeln!(out, "    let mut state = logic::State::default();").unwrap();
    writeln!(
        out,
        "    let mut ticker = Ticker::every(Duration::from_millis({dt_ms}));"
    )
    .unwrap();
    writeln!(out, "    loop {{").unwrap();
    writeln!(out, "        logic::tick(&mut hw, &mut state);").unwrap();
    writeln!(out, "        ticker.next().await;").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    // Append C-FFI hw_* stubs for MLIR backend
    writeln!(out).unwrap();
    writeln!(
        out,
        "static mut HW: HwPeripherals = HwPeripherals {{ _marker: () }};"
    )
    .unwrap();
    out.push_str(&super::generate_hw_ffi_stubs("HW"));

    out
}
