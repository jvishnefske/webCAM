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
}
