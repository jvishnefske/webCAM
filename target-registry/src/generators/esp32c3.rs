//! ESP32-C3 target generator (RISC-V, esp-hal + embassy).

use std::fmt::Write;

use super::TargetGenerator;
use crate::binding::Binding;
use graph_model::GraphSnapshot;

pub struct Esp32c3Generator;

impl TargetGenerator for Esp32c3Generator {
    fn generate(
        &self,
        _snap: &GraphSnapshot,
        _binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String> {
        let cargo_toml = generate_cargo_toml();
        let cargo_config = generate_cargo_config();
        let main_rs = generate_main_rs(dt);

        Ok(vec![
            ("target-esp32c3/Cargo.toml".to_string(), cargo_toml),
            (
                "target-esp32c3/.cargo/config.toml".to_string(),
                cargo_config,
            ),
            ("target-esp32c3/src/main.rs".to_string(), main_rs),
        ])
    }
}

fn generate_cargo_toml() -> String {
    r#"[package]
name = "target-esp32c3"
version = "0.1.0"
edition = "2021"

[dependencies]
logic = { path = "../logic" }
dataflow-rt = { path = "../dataflow-rt", default-features = false }
esp-hal = { version = "1.0.0-beta.0", features = ["esp32c3"] }
esp-hal-embassy = { version = "0.7", features = ["esp32c3"] }
embassy-executor = { version = "0.7", features = ["task-arena-size-12288"] }
embassy-time = "0.4"
panic-halt = "1"

[profile.release]
opt-level = "z"
lto = true
"#
    .to_string()
}

fn generate_cargo_config() -> String {
    r#"[build]
target = "riscv32imc-unknown-none-elf"

[unstable]
build-std = ["core"]
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
    writeln!(
        out,
        "    let _peripherals = esp_hal::init(esp_hal::Config::default());"
    )
    .unwrap();
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

    out
}
