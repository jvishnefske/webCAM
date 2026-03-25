//! Embedded peripheral blocks: ADC, PWM, GPIO, UART.
//!
//! In WASM these are stubbed — real hardware access requires native execution
//! on an embedded target. The blocks participate in the graph so the topology
//! can be designed in the browser and later code-generated for a specific MCU.

use crate::dataflow::block::{Block, PortDef, PortKind, Value};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ADC Source
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct AdcConfig {
    pub channel: u8,
    pub resolution_bits: u8,
}

impl Default for AdcConfig {
    fn default() -> Self {
        Self {
            channel: 0,
            resolution_bits: 12,
        }
    }
}

/// Reads an analog-to-digital converter channel.
/// Stubbed in WASM — always outputs None.
pub struct AdcBlock {
    config: AdcConfig,
}

impl AdcBlock {
    pub fn from_config(config: AdcConfig) -> Self {
        Self { config }
    }
}

impl Block for AdcBlock {
    fn name(&self) -> &str {
        "ADC Source"
    }
    fn block_type(&self) -> &str {
        "adc_source"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("value", PortKind::Float)]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual ADC in WASM.
        vec![None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// PWM Sink
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct PwmConfig {
    pub channel: u8,
    pub frequency_hz: u32,
}

impl Default for PwmConfig {
    fn default() -> Self {
        Self {
            channel: 0,
            frequency_hz: 1000,
        }
    }
}

/// Drives a PWM output channel with a duty cycle (0.0 to 1.0).
/// Stubbed in WASM.
pub struct PwmBlock {
    config: PwmConfig,
}

impl PwmBlock {
    pub fn from_config(config: PwmConfig) -> Self {
        Self { config }
    }
}

impl Block for PwmBlock {
    fn name(&self) -> &str {
        "PWM Sink"
    }
    fn block_type(&self) -> &str {
        "pwm_sink"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("duty", PortKind::Float)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual PWM in WASM.
        vec![]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// GPIO Out
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct GpioOutConfig {
    pub pin: u8,
}

impl Default for GpioOutConfig {
    fn default() -> Self {
        Self { pin: 13 }
    }
}

/// Sets a GPIO pin high (>0.5) or low (<=0.5).
/// Stubbed in WASM.
pub struct GpioOutBlock {
    config: GpioOutConfig,
}

impl GpioOutBlock {
    pub fn from_config(config: GpioOutConfig) -> Self {
        Self { config }
    }
}

impl Block for GpioOutBlock {
    fn name(&self) -> &str {
        "GPIO Out"
    }
    fn block_type(&self) -> &str {
        "gpio_out"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("state", PortKind::Float)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual GPIO in WASM.
        vec![]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// GPIO In
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct GpioInConfig {
    pub pin: u8,
}

impl Default for GpioInConfig {
    fn default() -> Self {
        Self { pin: 2 }
    }
}

/// Reads a GPIO pin state (0.0 or 1.0).
/// Stubbed in WASM — always outputs None.
pub struct GpioInBlock {
    config: GpioInConfig,
}

impl GpioInBlock {
    pub fn from_config(config: GpioInConfig) -> Self {
        Self { config }
    }
}

impl Block for GpioInBlock {
    fn name(&self) -> &str {
        "GPIO In"
    }
    fn block_type(&self) -> &str {
        "gpio_in"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("state", PortKind::Float)]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual GPIO in WASM.
        vec![None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// UART TX
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct UartTxConfig {
    pub port: u8,
    pub baud: u32,
}

impl Default for UartTxConfig {
    fn default() -> Self {
        Self {
            port: 0,
            baud: 115200,
        }
    }
}

/// Transmits bytes over a UART port.
/// Stubbed in WASM.
pub struct UartTxBlock {
    config: UartTxConfig,
}

impl UartTxBlock {
    pub fn from_config(config: UartTxConfig) -> Self {
        Self { config }
    }
}

impl Block for UartTxBlock {
    fn name(&self) -> &str {
        "UART TX"
    }
    fn block_type(&self) -> &str {
        "uart_tx"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("data", PortKind::Bytes)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UART in WASM.
        vec![]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// UART RX
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct UartRxConfig {
    pub port: u8,
    pub baud: u32,
}

impl Default for UartRxConfig {
    fn default() -> Self {
        Self {
            port: 0,
            baud: 115200,
        }
    }
}

/// Receives bytes from a UART port.
/// Stubbed in WASM — always outputs None.
pub struct UartRxBlock {
    config: UartRxConfig,
}

impl UartRxBlock {
    pub fn from_config(config: UartRxConfig) -> Self {
        Self { config }
    }
}

impl Block for UartRxBlock {
    fn name(&self) -> &str {
        "UART RX"
    }
    fn block_type(&self) -> &str {
        "uart_rx"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("data", PortKind::Bytes)]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UART in WASM.
        vec![None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct EncoderConfig {
    pub channel: u8,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self { channel: 0 }
    }
}

/// Reads a quadrature encoder channel.
/// Stubbed in WASM — always outputs None.
pub struct EncoderBlock {
    config: EncoderConfig,
}

impl EncoderBlock {
    pub fn from_config(config: EncoderConfig) -> Self {
        Self { config }
    }
}

impl Block for EncoderBlock {
    fn name(&self) -> &str {
        "Encoder"
    }
    fn block_type(&self) -> &str {
        "encoder"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef::new("position", PortKind::Float),
            PortDef::new("velocity", PortKind::Float),
        ]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual encoder in WASM.
        vec![None, None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// SSD1306 Display
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Ssd1306DisplayConfig {
    pub i2c_bus: u8,
    pub address: u8,
}

impl Default for Ssd1306DisplayConfig {
    fn default() -> Self {
        Self {
            i2c_bus: 0,
            address: 0x3C,
        }
    }
}

/// Writes two lines to an SSD1306 OLED display.
/// Stubbed in WASM.
pub struct Ssd1306DisplayBlock {
    config: Ssd1306DisplayConfig,
}

impl Ssd1306DisplayBlock {
    pub fn from_config(config: Ssd1306DisplayConfig) -> Self {
        Self { config }
    }
}

impl Block for Ssd1306DisplayBlock {
    fn name(&self) -> &str {
        "SSD1306 Display"
    }
    fn block_type(&self) -> &str {
        "ssd1306_display"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef::new("line1", PortKind::Text),
            PortDef::new("line2", PortKind::Text),
        ]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual display in WASM.
        vec![]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// TMC2209 Stepper
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Tmc2209StepperConfig {
    pub uart_port: u8,
    pub uart_addr: u8,
    pub steps_per_rev: u16,
    pub microsteps: u8,
}

impl Default for Tmc2209StepperConfig {
    fn default() -> Self {
        Self {
            uart_port: 0,
            uart_addr: 0,
            steps_per_rev: 200,
            microsteps: 16,
        }
    }
}

/// Controls a TMC2209 stepper driver.
/// Stubbed in WASM.
pub struct Tmc2209StepperBlock {
    config: Tmc2209StepperConfig,
}

impl Tmc2209StepperBlock {
    pub fn from_config(config: Tmc2209StepperConfig) -> Self {
        Self { config }
    }
}

impl Block for Tmc2209StepperBlock {
    fn name(&self) -> &str {
        "TMC2209 Stepper"
    }
    fn block_type(&self) -> &str {
        "tmc2209_stepper"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef::new("target_position", PortKind::Float),
            PortDef::new("enable", PortKind::Float),
        ]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("actual_position", PortKind::Float)]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual stepper in WASM.
        vec![None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// TMC2209 StallGuard
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Tmc2209StallGuardConfig {
    pub uart_port: u8,
    pub uart_addr: u8,
    pub threshold: u16,
}

impl Default for Tmc2209StallGuardConfig {
    fn default() -> Self {
        Self {
            uart_port: 0,
            uart_addr: 0,
            threshold: 50,
        }
    }
}

/// Reads TMC2209 StallGuard value for stall detection.
/// Stubbed in WASM.
pub struct Tmc2209StallGuardBlock {
    config: Tmc2209StallGuardConfig,
}

impl Tmc2209StallGuardBlock {
    pub fn from_config(config: Tmc2209StallGuardConfig) -> Self {
        Self { config }
    }
}

impl Block for Tmc2209StallGuardBlock {
    fn name(&self) -> &str {
        "TMC2209 StallGuard"
    }
    fn block_type(&self) -> &str {
        "tmc2209_stallguard"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef::new("sg_value", PortKind::Float),
            PortDef::new("stall_detected", PortKind::Float),
        ]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual StallGuard in WASM.
        vec![None, None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adc_source_outputs_none() {
        let mut block = AdcBlock::from_config(AdcConfig::default());
        let result = block.tick(&[], 0.01);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_none());
    }

    #[test]
    fn pwm_config_roundtrip() {
        let config = PwmConfig {
            channel: 3,
            frequency_hz: 5000,
        };
        let block = PwmBlock::from_config(config);
        let json = block.config_json();
        let parsed: PwmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.channel, 3);
        assert_eq!(parsed.frequency_hz, 5000);
    }

    #[test]
    fn gpio_out_accepts_float() {
        let mut block = GpioOutBlock::from_config(GpioOutConfig::default());
        let val = Value::Float(0.8);
        let result = block.tick(&[Some(&val)], 0.01);
        assert!(result.is_empty());
    }

    #[test]
    fn uart_rx_outputs_none() {
        let mut block = UartRxBlock::from_config(UartRxConfig::default());
        let result = block.tick(&[], 0.01);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_none());
    }

    #[test]
    fn encoder_outputs_none() {
        let mut block = EncoderBlock::from_config(EncoderConfig::default());
        let result = block.tick(&[], 0.01);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_none());
    }

    #[test]
    fn ssd1306_display_config_roundtrip() {
        let config = Ssd1306DisplayConfig { i2c_bus: 1, address: 0x3C };
        let block = Ssd1306DisplayBlock::from_config(config);
        let json = block.config_json();
        let parsed: Ssd1306DisplayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.i2c_bus, 1);
        assert_eq!(parsed.address, 0x3C);
    }

    #[test]
    fn tmc2209_stepper_config_roundtrip() {
        let config = Tmc2209StepperConfig { uart_port: 1, uart_addr: 2, steps_per_rev: 400, microsteps: 32 };
        let block = Tmc2209StepperBlock::from_config(config);
        let json = block.config_json();
        let parsed: Tmc2209StepperConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uart_port, 1);
        assert_eq!(parsed.steps_per_rev, 400);
    }

    #[test]
    fn tmc2209_stallguard_outputs_none() {
        let mut block = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
        let result = block.tick(&[], 0.01);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_none());
    }
}
