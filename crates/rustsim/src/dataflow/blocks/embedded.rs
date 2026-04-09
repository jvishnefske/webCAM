//! Embedded peripheral blocks: ADC, PWM, GPIO, UART, Encoder, Display, Stepper.
//!
//! In WASM these produce no output — real hardware access requires native execution
//! on an embedded target. The blocks participate in the graph so the topology
//! can be designed in the browser and later code-generated for a specific MCU.
//!
//! In simulation mode, blocks with `SimModel` impls interact with `SimPeripherals`.

use crate::dataflow::block::{Module, PortDef, PortKind, SimModel, SimPeripherals, Tick, Value};
use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

// ---------------------------------------------------------------------------
// ADC Source
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
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
/// Without simulation, outputs None.
pub struct AdcBlock {
    pub(crate) config: AdcConfig,
}

impl AdcBlock {
    pub fn from_config(config: AdcConfig) -> Self {
        Self { config }
    }
}

impl Module for AdcBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for AdcBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // In non-simulation mode, output 0.0 (no hardware)
        vec![Some(Value::Float(0.0))]
    }
}

impl SimModel for AdcBlock {
    fn sim_tick(
        &mut self,
        _inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let voltage = peripherals.adc_read(self.config.channel);
        vec![Some(Value::Float(voltage))]
    }
}

// ---------------------------------------------------------------------------
// PWM Sink
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
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
/// Without simulation, consumes input silently.
pub struct PwmBlock {
    pub(crate) config: PwmConfig,
}

impl PwmBlock {
    pub fn from_config(config: PwmConfig) -> Self {
        Self { config }
    }
}

impl Module for PwmBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for PwmBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Sink: consume input, produce no output
        vec![]
    }
}

impl SimModel for PwmBlock {
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        if let Some(duty) = inputs.first().and_then(|i| i.and_then(|v| v.as_float())) {
            peripherals.pwm_write(self.config.channel, duty);
        }
        vec![]
    }
}

// ---------------------------------------------------------------------------
// GPIO Out
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct GpioOutConfig {
    pub pin: u8,
}

impl Default for GpioOutConfig {
    fn default() -> Self {
        Self { pin: 13 }
    }
}

/// Sets a GPIO pin high (>0.5) or low (<=0.5).
/// Without simulation, consumes input silently.
pub struct GpioOutBlock {
    pub(crate) config: GpioOutConfig,
}

impl GpioOutBlock {
    pub fn from_config(config: GpioOutConfig) -> Self {
        Self { config }
    }
}

impl Module for GpioOutBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for GpioOutBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![]
    }
}

impl SimModel for GpioOutBlock {
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        if let Some(v) = inputs.first().and_then(|i| i.and_then(|v| v.as_float())) {
            peripherals.gpio_write(self.config.pin, v > 0.5);
        }
        vec![]
    }
}

// ---------------------------------------------------------------------------
// GPIO In
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct GpioInConfig {
    pub pin: u8,
}

impl Default for GpioInConfig {
    fn default() -> Self {
        Self { pin: 2 }
    }
}

/// Reads a GPIO pin state (0.0 or 1.0).
/// Without simulation, outputs None.
pub struct GpioInBlock {
    pub(crate) config: GpioInConfig,
}

impl GpioInBlock {
    pub fn from_config(config: GpioInConfig) -> Self {
        Self { config }
    }
}

impl Module for GpioInBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for GpioInBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![Some(Value::Float(0.0))]
    }
}

impl SimModel for GpioInBlock {
    fn sim_tick(
        &mut self,
        _inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let state = if peripherals.gpio_read(self.config.pin) {
            1.0
        } else {
            0.0
        };
        vec![Some(Value::Float(state))]
    }
}

// ---------------------------------------------------------------------------
// UART TX
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
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
/// Without simulation, consumes input silently.
pub struct UartTxBlock {
    pub(crate) config: UartTxConfig,
}

impl UartTxBlock {
    pub fn from_config(config: UartTxConfig) -> Self {
        Self { config }
    }
}

impl Module for UartTxBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for UartTxBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![]
    }
}

impl SimModel for UartTxBlock {
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        if let Some(data) = inputs.first().and_then(|i| i.and_then(|v| v.as_bytes())) {
            peripherals.uart_write(self.config.port, data);
        }
        vec![]
    }
}

// ---------------------------------------------------------------------------
// UART RX
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
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
/// Without simulation, outputs None.
pub struct UartRxBlock {
    pub(crate) config: UartRxConfig,
}

impl UartRxBlock {
    pub fn from_config(config: UartRxConfig) -> Self {
        Self { config }
    }
}

impl Module for UartRxBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for UartRxBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![None]
    }
}

impl SimModel for UartRxBlock {
    fn sim_tick(
        &mut self,
        _inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let mut buf = vec![0u8; 256];
        let n = peripherals.uart_read(self.config.port, &mut buf);
        if n > 0 {
            buf.truncate(n);
            vec![Some(Value::Bytes(buf))]
        } else {
            vec![None]
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct EncoderConfig {
    pub channel: u8,
}

/// Reads a quadrature encoder channel.
/// Without simulation, outputs zero position and velocity.
pub struct EncoderBlock {
    pub(crate) config: EncoderConfig,
    prev_position: i64,
}

impl EncoderBlock {
    pub fn from_config(config: EncoderConfig) -> Self {
        Self {
            config,
            prev_position: 0,
        }
    }
}

impl Module for EncoderBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for EncoderBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![Some(Value::Float(0.0)), Some(Value::Float(0.0))]
    }
}

impl SimModel for EncoderBlock {
    fn sim_tick(
        &mut self,
        _inputs: &[Option<&Value>],
        dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let position = peripherals.encoder_read(self.config.channel);
        let delta = position - self.prev_position;
        self.prev_position = position;
        let pos_f64 = position as f64;
        let velocity = if dt > 0.0 { delta as f64 / dt } else { 0.0 };
        vec![Some(Value::Float(pos_f64)), Some(Value::Float(velocity))]
    }
}

// ---------------------------------------------------------------------------
// SSD1306 Display
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(default)]
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
/// Without simulation, consumes input silently.
pub struct Ssd1306DisplayBlock {
    pub(crate) config: Ssd1306DisplayConfig,
}

impl Ssd1306DisplayBlock {
    pub fn from_config(config: Ssd1306DisplayConfig) -> Self {
        Self { config }
    }
}

impl Module for Ssd1306DisplayBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for Ssd1306DisplayBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![]
    }
}

impl SimModel for Ssd1306DisplayBlock {
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let line1 = inputs
            .first()
            .and_then(|i| i.and_then(|v| v.as_text()))
            .unwrap_or("");
        let line2 = inputs
            .get(1)
            .and_then(|i| i.and_then(|v| v.as_text()))
            .unwrap_or("");
        peripherals.display_write(self.config.i2c_bus, self.config.address, line1, line2);
        vec![]
    }
}

// ---------------------------------------------------------------------------
// TMC2209 Stepper
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(default)]
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
/// Without simulation, consumes input silently.
pub struct Tmc2209StepperBlock {
    pub(crate) config: Tmc2209StepperConfig,
}

impl Tmc2209StepperBlock {
    pub fn from_config(config: Tmc2209StepperConfig) -> Self {
        Self { config }
    }
}

impl Module for Tmc2209StepperBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for Tmc2209StepperBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![Some(Value::Float(0.0))]
    }
}

impl SimModel for Tmc2209StepperBlock {
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        _dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        let enabled = inputs
            .get(1)
            .and_then(|i| i.and_then(|v| v.as_float()))
            .unwrap_or(0.0)
            > 0.5;
        if enabled {
            if let Some(target) = inputs.first().and_then(|i| i.and_then(|v| v.as_float())) {
                peripherals.stepper_move(self.config.uart_port, target as i64);
            }
        }
        let pos = peripherals.stepper_position(self.config.uart_port);
        vec![Some(Value::Float(pos as f64))]
    }
}

// ---------------------------------------------------------------------------
// TMC2209 StallGuard
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Tsify, schemars::JsonSchema)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(default)]
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
/// Without simulation, outputs None.
pub struct Tmc2209StallGuardBlock {
    pub(crate) config: Tmc2209StallGuardConfig,
}

impl Tmc2209StallGuardBlock {
    pub fn from_config(config: Tmc2209StallGuardConfig) -> Self {
        Self { config }
    }
}

impl Module for Tmc2209StallGuardBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        Some(self)
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for Tmc2209StallGuardBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        vec![Some(Value::Float(0.0)), Some(Value::Float(0.0))]
    }
}

impl SimModel for Tmc2209StallGuardBlock {
    fn sim_tick(
        &mut self,
        _inputs: &[Option<&Value>],
        _dt: f64,
        _peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>> {
        // StallGuard values would come from more sophisticated motor simulation.
        // For now, return zeros (no stall).
        vec![Some(Value::Float(0.0)), Some(Value::Float(0.0))]
    }
}

pub(crate) fn register(reg: &mut Vec<super::registry::BlockRegistration>) {
    reg.push(super::registry::BlockRegistration {
        block_type: "adc_source",
        display_name: "ADC Source",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: AdcConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(AdcBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "pwm_sink",
        display_name: "PWM Sink",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: PwmConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(PwmBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "gpio_out",
        display_name: "GPIO Out",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: GpioOutConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(GpioOutBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "gpio_in",
        display_name: "GPIO In",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: GpioInConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(GpioInBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "uart_tx",
        display_name: "UART TX",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: UartTxConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(UartTxBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "uart_rx",
        display_name: "UART RX",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: UartRxConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(UartRxBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "encoder",
        display_name: "Encoder",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: EncoderConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(EncoderBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "ssd1306_display",
        display_name: "SSD1306 Display",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: Ssd1306DisplayConfig =
                serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(Ssd1306DisplayBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "tmc2209_stepper",
        display_name: "TMC2209 Stepper",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: Tmc2209StepperConfig =
                serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(Tmc2209StepperBlock::from_config(cfg)))
        },
    });
    reg.push(super::registry::BlockRegistration {
        block_type: "tmc2209_stallguard",
        display_name: "TMC2209 StallGuard",
        category: "Embedded",
        create_from_json: |json| {
            let cfg: Tmc2209StallGuardConfig =
                serde_json::from_str(json).map_err(|e| e.to_string())?;
            Ok(Box::new(Tmc2209StallGuardBlock::from_config(cfg)))
        },
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::block::Module;

    #[test]
    fn adc_source_module_only() {
        let block = AdcBlock::from_config(AdcConfig::default());
        assert_eq!(block.output_ports().len(), 1);
        assert_eq!(block.block_type(), "adc_source");
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
    fn gpio_out_ports() {
        let block = GpioOutBlock::from_config(GpioOutConfig::default());
        assert_eq!(block.input_ports().len(), 1);
        assert!(block.output_ports().is_empty());
    }

    #[test]
    fn uart_rx_ports() {
        let block = UartRxBlock::from_config(UartRxConfig::default());
        assert_eq!(block.output_ports().len(), 1);
        assert!(block.input_ports().is_empty());
    }

    #[test]
    fn encoder_ports() {
        let block = EncoderBlock::from_config(EncoderConfig::default());
        assert_eq!(block.output_ports().len(), 2);
    }

    #[test]
    fn ssd1306_display_config_roundtrip() {
        let config = Ssd1306DisplayConfig {
            i2c_bus: 1,
            address: 0x3C,
        };
        let block = Ssd1306DisplayBlock::from_config(config);
        let json = block.config_json();
        let parsed: Ssd1306DisplayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.i2c_bus, 1);
        assert_eq!(parsed.address, 0x3C);
    }

    #[test]
    fn tmc2209_stepper_config_roundtrip() {
        let config = Tmc2209StepperConfig {
            uart_port: 1,
            uart_addr: 2,
            steps_per_rev: 400,
            microsteps: 32,
        };
        let block = Tmc2209StepperBlock::from_config(config);
        let json = block.config_json();
        let parsed: Tmc2209StepperConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uart_port, 1);
        assert_eq!(parsed.steps_per_rev, 400);
    }

    #[test]
    fn tmc2209_stallguard_ports() {
        let block = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
        assert_eq!(block.output_ports().len(), 2);
        assert!(block.input_ports().is_empty());
    }

    // --- SimModel tests ---

    #[test]
    fn adc_sim_reads_configured_voltage() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = AdcBlock::from_config(AdcConfig {
            channel: 2,
            resolution_bits: 12,
        });
        let mut peripherals = WasmSimPeripherals::new();
        peripherals.set_adc_voltage(2, 3.3);

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert_eq!(out[0], Some(Value::Float(3.3)));
    }

    #[test]
    fn pwm_sim_writes_duty() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = PwmBlock::from_config(PwmConfig {
            channel: 1,
            frequency_hz: 1000,
        });
        let mut peripherals = WasmSimPeripherals::new();
        let duty = Value::Float(0.75);

        let sim = block.as_sim_model().unwrap();
        sim.sim_tick(&[Some(&duty)], 0.01, &mut peripherals);
        assert_eq!(peripherals.get_pwm_duty(1), 0.75);
    }

    #[test]
    fn gpio_sim_roundtrip() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut peripherals = WasmSimPeripherals::new();
        peripherals.set_gpio_state(5, true);

        let mut in_block = GpioInBlock::from_config(GpioInConfig { pin: 5 });
        let sim = in_block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert_eq!(out[0], Some(Value::Float(1.0)));

        let mut out_block = GpioOutBlock::from_config(GpioOutConfig { pin: 7 });
        let val = Value::Float(1.0);
        let sim = out_block.as_sim_model().unwrap();
        sim.sim_tick(&[Some(&val)], 0.01, &mut peripherals);
        assert!(peripherals.get_gpio_state(7));
    }

    // ---------------------------------------------------------------
    // Full Module trait coverage tests
    // ---------------------------------------------------------------

    /// Helper: exercise every Module trait method on a block, including
    /// the default-returning as_analysis, as_codegen, as_tick.
    fn assert_module_basics(block: &mut dyn Module, name: &str, block_type: &str) {
        assert_eq!(block.name(), name);
        assert_eq!(block.block_type(), block_type);
        // input_ports / output_ports just need to not panic
        let _ = block.input_ports();
        let _ = block.output_ports();
        let _ = block.config_json();
        // default trait impls return None for these
        assert!(block.as_analysis().is_none());
        assert!(block.as_codegen().is_none());
        assert!(block.as_tick().is_some());
    }

    #[test]
    fn adc_full_module_trait() {
        let mut b = AdcBlock::from_config(AdcConfig::default());
        assert_module_basics(&mut b, "ADC Source", "adc_source");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn pwm_full_module_trait() {
        let mut b = PwmBlock::from_config(PwmConfig::default());
        assert_module_basics(&mut b, "PWM Sink", "pwm_sink");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn gpio_out_full_module_trait() {
        let mut b = GpioOutBlock::from_config(GpioOutConfig::default());
        assert_module_basics(&mut b, "GPIO Out", "gpio_out");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn gpio_in_full_module_trait() {
        let mut b = GpioInBlock::from_config(GpioInConfig::default());
        assert_module_basics(&mut b, "GPIO In", "gpio_in");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn uart_tx_full_module_trait() {
        let mut b = UartTxBlock::from_config(UartTxConfig::default());
        assert_module_basics(&mut b, "UART TX", "uart_tx");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn uart_rx_full_module_trait() {
        let mut b = UartRxBlock::from_config(UartRxConfig::default());
        assert_module_basics(&mut b, "UART RX", "uart_rx");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn encoder_full_module_trait() {
        let mut b = EncoderBlock::from_config(EncoderConfig::default());
        assert_module_basics(&mut b, "Encoder", "encoder");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn ssd1306_full_module_trait() {
        let mut b = Ssd1306DisplayBlock::from_config(Ssd1306DisplayConfig::default());
        assert_module_basics(&mut b, "SSD1306 Display", "ssd1306_display");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn tmc2209_stepper_full_module_trait() {
        let mut b = Tmc2209StepperBlock::from_config(Tmc2209StepperConfig::default());
        assert_module_basics(&mut b, "TMC2209 Stepper", "tmc2209_stepper");
        assert!(b.as_sim_model().is_some());
    }

    #[test]
    fn tmc2209_stallguard_full_module_trait() {
        let mut b = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
        assert_module_basics(&mut b, "TMC2209 StallGuard", "tmc2209_stallguard");
        assert!(b.as_sim_model().is_some());
    }

    // ---------------------------------------------------------------
    // SimModel::sim_tick coverage for blocks not yet tested
    // ---------------------------------------------------------------

    #[test]
    fn uart_tx_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = UartTxBlock::from_config(UartTxConfig::default());
        let mut peripherals = WasmSimPeripherals::new();
        let data = Value::Bytes(vec![0x41, 0x42]);

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[Some(&data)], 0.01, &mut peripherals);
        assert!(out.is_empty());
    }

    #[test]
    fn uart_tx_sim_tick_no_input() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = UartTxBlock::from_config(UartTxConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert!(out.is_empty());
    }

    #[test]
    fn uart_rx_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = UartRxBlock::from_config(UartRxConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        // No data queued, so output should be None
        assert_eq!(out.len(), 1);
        assert!(out[0].is_none());
    }

    #[test]
    fn encoder_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = EncoderBlock::from_config(EncoderConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert_eq!(out.len(), 2);
        // position and velocity both present
        assert!(out[0].is_some());
        assert!(out[1].is_some());
    }

    #[test]
    fn ssd1306_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = Ssd1306DisplayBlock::from_config(Ssd1306DisplayConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let line1 = Value::Text("hello".into());
        let line2 = Value::Text("world".into());
        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[Some(&line1), Some(&line2)], 0.01, &mut peripherals);
        assert!(out.is_empty());
    }

    #[test]
    fn ssd1306_sim_tick_no_input() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = Ssd1306DisplayBlock::from_config(Ssd1306DisplayConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert!(out.is_empty());
    }

    #[test]
    fn tmc2209_stepper_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = Tmc2209StepperBlock::from_config(Tmc2209StepperConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let target = Value::Float(100.0);
        let enable = Value::Float(1.0);
        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[Some(&target), Some(&enable)], 0.01, &mut peripherals);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_some());
    }

    #[test]
    fn tmc2209_stepper_sim_tick_disabled() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = Tmc2209StepperBlock::from_config(Tmc2209StepperConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let target = Value::Float(100.0);
        let enable = Value::Float(0.0);
        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[Some(&target), Some(&enable)], 0.01, &mut peripherals);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn tmc2209_stallguard_sim_tick() {
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut block = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
        let mut peripherals = WasmSimPeripherals::new();

        let sim = block.as_sim_model().unwrap();
        let out = sim.sim_tick(&[], 0.01, &mut peripherals);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], Some(Value::Float(0.0)));
        assert_eq!(out[1], Some(Value::Float(0.0)));
    }

    #[test]
    fn sim_mode_graph_adc_to_gain_to_pwm() {
        use crate::dataflow::blocks::function::FunctionBlock;
        use crate::dataflow::graph::DataflowGraph;
        use crate::dataflow::sim_peripherals::WasmSimPeripherals;

        let mut g = DataflowGraph::new();
        g.set_simulation_mode(true);
        let mut peripherals = WasmSimPeripherals::new();
        peripherals.set_adc_voltage(0, 2.5);
        g.set_sim_peripherals(peripherals);

        let adc = g.add_block(Box::new(AdcBlock::from_config(AdcConfig::default())));
        let gain = g.add_block(Box::new(FunctionBlock::gain(0.4)));
        let pwm = g.add_block(Box::new(PwmBlock::from_config(PwmConfig::default())));

        g.connect(adc, 0, gain, 0).unwrap();
        g.connect(gain, 0, pwm, 0).unwrap();

        // Tick 1: ADC reads 2.5
        g.tick(0.01);
        // Tick 2: Gain receives 2.5, outputs 1.0
        g.tick(0.01);
        // Tick 3: PWM receives 1.0
        g.tick(0.01);

        assert_eq!(g.get_sim_pwm(0), 1.0);
    }

    // --- Tick (normal mode) tests ---

    #[test]
    fn adc_tick_returns_zero() {
        let mut block = AdcBlock::from_config(AdcConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out, vec![Some(Value::Float(0.0))]);
    }

    #[test]
    fn pwm_tick_consumes_input() {
        let mut block = PwmBlock::from_config(PwmConfig::default());
        let duty = Value::Float(0.5);
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[Some(&duty)], 0.01);
        assert!(out.is_empty());
    }

    #[test]
    fn gpio_in_tick_returns_zero() {
        let mut block = GpioInBlock::from_config(GpioInConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out, vec![Some(Value::Float(0.0))]);
    }

    #[test]
    fn gpio_out_tick_consumes_input() {
        let mut block = GpioOutBlock::from_config(GpioOutConfig::default());
        let val = Value::Float(1.0);
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[Some(&val)], 0.01);
        assert!(out.is_empty());
    }

    #[test]
    fn uart_rx_tick_returns_none() {
        let mut block = UartRxBlock::from_config(UartRxConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out, vec![None]);
    }

    #[test]
    fn uart_tx_tick_consumes_input() {
        let mut block = UartTxBlock::from_config(UartTxConfig::default());
        let data = Value::Bytes(vec![0x48, 0x49]);
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[Some(&data)], 0.01);
        assert!(out.is_empty());
    }

    #[test]
    fn encoder_tick_returns_zeros() {
        let mut block = EncoderBlock::from_config(EncoderConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], Some(Value::Float(0.0)));
        assert_eq!(out[1], Some(Value::Float(0.0)));
    }

    #[test]
    fn display_tick_consumes_input() {
        let mut block = Ssd1306DisplayBlock::from_config(Ssd1306DisplayConfig::default());
        let l1 = Value::Text("Hello".into());
        let l2 = Value::Text("World".into());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[Some(&l1), Some(&l2)], 0.01);
        assert!(out.is_empty());
    }

    #[test]
    fn stepper_tick_returns_zero_position() {
        let mut block = Tmc2209StepperBlock::from_config(Tmc2209StepperConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out, vec![Some(Value::Float(0.0))]);
    }

    #[test]
    fn stallguard_tick_returns_zeros() {
        let mut block = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
        let tick = block.as_tick().unwrap();
        let out = tick.tick(&[], 0.01);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], Some(Value::Float(0.0)));
        assert_eq!(out[1], Some(Value::Float(0.0)));
    }
}
