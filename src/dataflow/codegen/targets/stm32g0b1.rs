//! STM32G0B1 Embassy target generator.

use std::fmt::Write;

use super::TargetGenerator;
use crate::dataflow::codegen::binding::Binding;
use crate::dataflow::graph::GraphSnapshot;

pub struct Stm32g0b1Generator;

impl TargetGenerator for Stm32g0b1Generator {
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
            ("target-stm32g0b1/Cargo.toml".to_string(), cargo_toml),
            (
                "target-stm32g0b1/.cargo/config.toml".to_string(),
                cargo_config,
            ),
            ("target-stm32g0b1/build.rs".to_string(), build_rs),
            ("target-stm32g0b1/src/main.rs".to_string(), main_rs),
        ])
    }
}

fn generate_cargo_toml() -> String {
    r#"[package]
name = "target-stm32g0b1"
version = "0.1.0"
edition = "2021"

[dependencies]
logic = { path = "../logic" }
dataflow-rt = { path = "../dataflow-rt", default-features = false }
embassy-executor = { version = "0.7", features = ["arch-cortex-m", "executor-thread"] }
embassy-stm32 = { version = "0.2", features = ["stm32g0b1cb", "time-driver-tim3"] }
embassy-time = "0.4"
cortex-m = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt = "0.7"
panic-halt = "1"
ssd1306 = "0.9"
embedded-graphics = "0.8"

[profile.release]
opt-level = "z"
lto = true
"#
    .to_string()
}

fn generate_cargo_config() -> String {
    r#"[build]
target = "thumbv6m-none-eabi"

[target.thumbv6m-none-eabi]
runner = "probe-rs run --chip STM32G0B1CBTx"
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
    writeln!(out, "    // TODO: Add peripheral handles").unwrap();
    writeln!(out, "    // encoder: TIM1 in encoder mode (PA8/PA9)").unwrap();
    writeln!(out, "    // i2c: I2C1 (PB7 SDA, PB6 SCL) for SSD1306").unwrap();
    writeln!(out, "    // uart: USART2 (PA2 TX, PA3 RX) for TMC2209").unwrap();
    writeln!(out, "    // step_pin: PA0, dir_pin: PA1, en_pin: PA4").unwrap();
    writeln!(out, "    _marker: (),").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl Peripherals for HwPeripherals {{").unwrap();
    writeln!(
        out,
        "    fn adc_read(&mut self, _ch: u8) -> f32 {{ 0.0 }}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn pwm_write(&mut self, _ch: u8, _duty: f32) {{}}"
    )
    .unwrap();
    writeln!(
        out,
        "    fn gpio_read(&self, _pin: u8) -> bool {{ false }}"
    )
    .unwrap();
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
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn encoder_read(&mut self, _ch: u8) -> i64 {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Read TIM1 counter in encoder mode"
    )
    .unwrap();
    writeln!(out, "        0").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn display_write(&mut self, _bus: u8, _addr: u8, _line1: &str, _line2: &str) {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Write to SSD1306 via I2C1"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn stepper_move(&mut self, _port: u8, _target: i64) {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Generate step pulses toward target"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn stepper_position(&self, _port: u8) -> i64 {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Return current step count"
    )
    .unwrap();
    writeln!(out, "        0").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn stepper_enable(&mut self, _port: u8, _enabled: bool) {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Set enable pin (active low)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn stallguard_read(&mut self, _port: u8, _addr: u8) -> u16 {{"
    )
    .unwrap();
    writeln!(
        out,
        "        // TODO: Read StallGuard via TMC2209 UART"
    )
    .unwrap();
    writeln!(out, "        0").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#[embassy_executor::main]").unwrap();
    writeln!(out, "async fn main(_spawner: Spawner) {{").unwrap();
    writeln!(
        out,
        "    let _p = embassy_stm32::init(Default::default());"
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
