//! Hardware peripheral configuration model for the browser → MCU workflow.
//!
//! Three-layer model:
//! 1. [`PeripheralRequirement`] — what the graph needs (extracted from blocks)
//! 2. [`TargetCapabilities`] — what the MCU can provide (static per target)
//! 3. [`HardwareConfig`] — user's mapping from logical channels to physical pins
//!
//! The frontend extracts requirements, shows available pins from capabilities,
//! the user assigns pins, and the config is sent to the MCU alongside the graph.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Layer 1: Peripheral requirements (extracted from graph)
// ---------------------------------------------------------------------------

/// A single peripheral requirement extracted from a block in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PeripheralRequirement {
    Adc {
        logical_channel: u8,
    },
    Pwm {
        logical_channel: u8,
    },
    GpioOutput {
        logical_pin: u8,
    },
    GpioInput {
        logical_pin: u8,
    },
    Uart {
        logical_port: u8,
        direction: UartDirection,
    },
    Encoder {
        logical_channel: u8,
    },
    I2c {
        logical_bus: u8,
        address: u8,
    },
    Stepper {
        logical_port: u8,
    },
    StallGuard {
        logical_port: u8,
        address: u8,
    },
}

/// UART direction — TX only, RX only, or bidirectional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UartDirection {
    Tx,
    Rx,
    Bidirectional,
}

/// All peripheral requirements for a graph, with their source block IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementSet {
    pub requirements: Vec<RequirementEntry>,
}

/// A requirement tied to the block that needs it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementEntry {
    /// The block ID that created this requirement.
    pub block_id: u32,
    /// The block's display name.
    pub block_name: String,
    /// What the block needs.
    pub requirement: PeripheralRequirement,
}

// ---------------------------------------------------------------------------
// Layer 2: Target capabilities (static per MCU family)
// ---------------------------------------------------------------------------

/// Description of a target MCU's peripheral capabilities.
///
/// Sent to the frontend so the pin configuration UI knows what's available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetCapabilities {
    /// Target family identifier.
    pub family: String,
    /// Human-readable name (e.g., "Raspberry Pi Pico (RP2040)").
    pub display_name: String,
    /// Available ADC-capable pins.
    pub adc_pins: Vec<AdcCapability>,
    /// Available PWM-capable pins.
    pub pwm_pins: Vec<PwmCapability>,
    /// Available GPIO pins (any direction).
    pub gpio_pins: Vec<GpioCapability>,
    /// Available UART peripherals.
    pub uart_peripherals: Vec<UartCapability>,
    /// Available encoder-capable timer/pin pairs.
    pub encoder_pins: Vec<EncoderCapability>,
    /// Available I2C peripherals.
    pub i2c_peripherals: Vec<I2cCapability>,
    /// Available stepper motor driver connections.
    pub stepper_slots: Vec<StepperCapability>,
}

/// An ADC-capable pin on the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcCapability {
    /// Physical pin name (e.g., "GP26", "PA0").
    pub pin: String,
    /// ADC peripheral name (e.g., "ADC0").
    pub peripheral: String,
    /// Hardware channel index on the ADC peripheral.
    pub hw_channel: u8,
}

/// A PWM-capable pin on the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwmCapability {
    /// Physical pin name.
    pub pin: String,
    /// Timer/slice name (e.g., "PWM_SLICE0", "TIM1").
    pub timer: String,
    /// Channel within the timer (e.g., "A", "B", or channel index).
    pub channel: String,
}

/// A GPIO-capable pin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioCapability {
    /// Physical pin name.
    pub pin: String,
    /// Whether the pin supports input.
    pub can_input: bool,
    /// Whether the pin supports output.
    pub can_output: bool,
    /// Whether the pin supports pull-up/pull-down.
    pub has_pull: bool,
}

/// A UART peripheral on the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UartCapability {
    /// Peripheral name (e.g., "UART0", "USART1").
    pub peripheral: String,
    /// Available TX pin options.
    pub tx_pins: Vec<String>,
    /// Available RX pin options.
    pub rx_pins: Vec<String>,
}

/// An encoder-capable pin pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderCapability {
    /// Timer peripheral used for encoder mode.
    pub timer: String,
    /// Pin A option.
    pub pin_a: String,
    /// Pin B option.
    pub pin_b: String,
}

/// An I2C peripheral on the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2cCapability {
    /// Peripheral name (e.g., "I2C0").
    pub peripheral: String,
    /// Available SDA pin options.
    pub sda_pins: Vec<String>,
    /// Available SCL pin options.
    pub scl_pins: Vec<String>,
}

/// A stepper motor driver slot (step/dir/enable + UART for TMC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepperCapability {
    /// Slot label (e.g., "X", "Y", "Z").
    pub label: String,
    /// Available step pin options.
    pub step_pins: Vec<String>,
    /// Available dir pin options.
    pub dir_pins: Vec<String>,
    /// Available enable pin options.
    pub enable_pins: Vec<String>,
    /// UART peripheral for TMC communication (if available).
    pub uart: Option<String>,
}

// ---------------------------------------------------------------------------
// Layer 3: Hardware configuration (user's pin assignments)
// ---------------------------------------------------------------------------

/// Complete hardware configuration: target + pin assignments.
///
/// This is the serializable object that travels from browser to MCU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    /// Target family identifier.
    pub family: String,
    /// Pin assignments mapping logical channels to physical pins.
    pub assignments: Vec<PinAssignment>,
}

/// A single pin assignment mapping a logical channel to physical hardware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PinAssignment {
    Adc {
        logical_channel: u8,
        pin: String,
        peripheral: String,
    },
    Pwm {
        logical_channel: u8,
        pin: String,
        timer: String,
    },
    Gpio {
        logical_pin: u8,
        pin: String,
        direction: GpioDirection,
    },
    Uart {
        logical_port: u8,
        tx_pin: String,
        rx_pin: String,
        peripheral: String,
    },
    Encoder {
        logical_channel: u8,
        pin_a: String,
        pin_b: String,
        timer: String,
    },
    I2c {
        logical_bus: u8,
        sda_pin: String,
        scl_pin: String,
        peripheral: String,
    },
    Stepper {
        logical_port: u8,
        step_pin: String,
        dir_pin: String,
        enable_pin: String,
        uart_peripheral: String,
    },
}

/// GPIO direction for a pin assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpioDirection {
    Input,
    Output,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Error found during hardware configuration validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigError {
    pub severity: ConfigSeverity,
    pub message: String,
    /// Block ID that caused the error, if applicable.
    pub block_id: Option<u32>,
}

/// Severity of a configuration error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigSeverity {
    Error,
    Warning,
}

/// Validate a hardware config against requirements and capabilities.
///
/// Returns `Ok(())` if the config is valid, or a list of errors/warnings.
pub fn validate_config(
    requirements: &RequirementSet,
    capabilities: &TargetCapabilities,
    config: &HardwareConfig,
) -> Result<(), Vec<ConfigError>> {
    let mut errors = Vec::new();

    // Check every requirement has a matching assignment
    for entry in &requirements.requirements {
        if !has_assignment_for(&entry.requirement, &config.assignments) {
            errors.push(ConfigError {
                severity: ConfigSeverity::Error,
                message: alloc::format!(
                    "block '{}' (id={}) requires {:?} but no pin is assigned",
                    entry.block_name,
                    entry.block_id,
                    entry.requirement
                ),
                block_id: Some(entry.block_id),
            });
        }
    }

    // Check for pin conflicts (same physical pin used twice)
    let all_pins = collect_all_pins(&config.assignments);
    for i in 0..all_pins.len() {
        for j in (i + 1)..all_pins.len() {
            if all_pins[i] == all_pins[j] {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!(
                        "pin '{}' is assigned to multiple peripherals",
                        all_pins[i]
                    ),
                    block_id: None,
                });
            }
        }
    }

    // Check assignments reference valid pins in capabilities
    for assignment in &config.assignments {
        validate_assignment_against_capabilities(assignment, capabilities, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn has_assignment_for(req: &PeripheralRequirement, assignments: &[PinAssignment]) -> bool {
    assignments.iter().any(|a| matches_requirement(a, req))
}

fn matches_requirement(assignment: &PinAssignment, req: &PeripheralRequirement) -> bool {
    match (assignment, req) {
        (
            PinAssignment::Adc {
                logical_channel: a, ..
            },
            PeripheralRequirement::Adc { logical_channel: r },
        ) => a == r,
        (
            PinAssignment::Pwm {
                logical_channel: a, ..
            },
            PeripheralRequirement::Pwm { logical_channel: r },
        ) => a == r,
        (
            PinAssignment::Gpio {
                logical_pin: a,
                direction: GpioDirection::Output,
                ..
            },
            PeripheralRequirement::GpioOutput { logical_pin: r },
        ) => a == r,
        (
            PinAssignment::Gpio {
                logical_pin: a,
                direction: GpioDirection::Input,
                ..
            },
            PeripheralRequirement::GpioInput { logical_pin: r },
        ) => a == r,
        (
            PinAssignment::Uart {
                logical_port: a, ..
            },
            PeripheralRequirement::Uart {
                logical_port: r, ..
            },
        ) => a == r,
        (
            PinAssignment::Encoder {
                logical_channel: a, ..
            },
            PeripheralRequirement::Encoder { logical_channel: r },
        ) => a == r,
        (
            PinAssignment::I2c { logical_bus: a, .. },
            PeripheralRequirement::I2c { logical_bus: r, .. },
        ) => a == r,
        (
            PinAssignment::Stepper {
                logical_port: a, ..
            },
            PeripheralRequirement::Stepper { logical_port: r },
        ) => a == r,
        (
            PinAssignment::Stepper {
                logical_port: a, ..
            },
            PeripheralRequirement::StallGuard {
                logical_port: r, ..
            },
        ) => a == r,
        _ => false,
    }
}

fn collect_all_pins(assignments: &[PinAssignment]) -> Vec<&str> {
    let mut pins = Vec::new();
    for a in assignments {
        match a {
            PinAssignment::Adc { pin, .. } => pins.push(pin.as_str()),
            PinAssignment::Pwm { pin, .. } => pins.push(pin.as_str()),
            PinAssignment::Gpio { pin, .. } => pins.push(pin.as_str()),
            PinAssignment::Uart { tx_pin, rx_pin, .. } => {
                pins.push(tx_pin.as_str());
                pins.push(rx_pin.as_str());
            }
            PinAssignment::Encoder { pin_a, pin_b, .. } => {
                pins.push(pin_a.as_str());
                pins.push(pin_b.as_str());
            }
            PinAssignment::I2c {
                sda_pin, scl_pin, ..
            } => {
                pins.push(sda_pin.as_str());
                pins.push(scl_pin.as_str());
            }
            PinAssignment::Stepper {
                step_pin,
                dir_pin,
                enable_pin,
                ..
            } => {
                pins.push(step_pin.as_str());
                pins.push(dir_pin.as_str());
                pins.push(enable_pin.as_str());
            }
        }
    }
    pins
}

fn validate_assignment_against_capabilities(
    assignment: &PinAssignment,
    capabilities: &TargetCapabilities,
    errors: &mut Vec<ConfigError>,
) {
    match assignment {
        PinAssignment::Adc { pin, .. } => {
            if !capabilities.adc_pins.iter().any(|c| c.pin == *pin) {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!("pin '{}' is not ADC-capable on this target", pin),
                    block_id: None,
                });
            }
        }
        PinAssignment::Pwm { pin, .. } => {
            if !capabilities.pwm_pins.iter().any(|c| c.pin == *pin) {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!("pin '{}' is not PWM-capable on this target", pin),
                    block_id: None,
                });
            }
        }
        PinAssignment::Gpio { pin, direction, .. } => {
            let cap = capabilities.gpio_pins.iter().find(|c| c.pin == *pin);
            match cap {
                None => {
                    errors.push(ConfigError {
                        severity: ConfigSeverity::Error,
                        message: alloc::format!("pin '{}' is not available on this target", pin),
                        block_id: None,
                    });
                }
                Some(c) => {
                    if *direction == GpioDirection::Input && !c.can_input {
                        errors.push(ConfigError {
                            severity: ConfigSeverity::Error,
                            message: alloc::format!("pin '{}' does not support input", pin),
                            block_id: None,
                        });
                    }
                    if *direction == GpioDirection::Output && !c.can_output {
                        errors.push(ConfigError {
                            severity: ConfigSeverity::Error,
                            message: alloc::format!("pin '{}' does not support output", pin),
                            block_id: None,
                        });
                    }
                }
            }
        }
        PinAssignment::Uart { peripheral, .. } => {
            if !capabilities
                .uart_peripherals
                .iter()
                .any(|c| c.peripheral == *peripheral)
            {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!(
                        "UART '{}' is not available on this target",
                        peripheral
                    ),
                    block_id: None,
                });
            }
        }
        PinAssignment::Encoder { timer, .. } => {
            if !capabilities.encoder_pins.iter().any(|c| c.timer == *timer) {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!(
                        "encoder timer '{}' is not available on this target",
                        timer
                    ),
                    block_id: None,
                });
            }
        }
        PinAssignment::I2c { peripheral, .. } => {
            if !capabilities
                .i2c_peripherals
                .iter()
                .any(|c| c.peripheral == *peripheral)
            {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: alloc::format!("I2C '{}' is not available on this target", peripheral),
                    block_id: None,
                });
            }
        }
        PinAssignment::Stepper { .. } => {
            if capabilities.stepper_slots.is_empty() {
                errors.push(ConfigError {
                    severity: ConfigSeverity::Error,
                    message: "target has no stepper driver support".into(),
                    block_id: None,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Requirement extraction
// ---------------------------------------------------------------------------

/// Extract peripheral requirements from a list of blocks.
///
/// Each block's `block_type` and `config` are inspected to determine what
/// hardware peripherals it needs.
pub fn extract_requirements(blocks: &[(u32, &str, &str, &serde_json::Value)]) -> RequirementSet {
    let mut requirements = Vec::new();

    for &(block_id, block_name, block_type, config) in blocks {
        match block_type {
            "adc_source" => {
                let ch = config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Adc {
                        logical_channel: ch,
                    },
                });
            }
            "pwm_sink" => {
                let ch = config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Pwm {
                        logical_channel: ch,
                    },
                });
            }
            "gpio_out" => {
                let pin = config.get("pin").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::GpioOutput { logical_pin: pin },
                });
            }
            "gpio_in" => {
                let pin = config.get("pin").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::GpioInput { logical_pin: pin },
                });
            }
            "uart_tx" => {
                let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Uart {
                        logical_port: port,
                        direction: UartDirection::Tx,
                    },
                });
            }
            "uart_rx" => {
                let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Uart {
                        logical_port: port,
                        direction: UartDirection::Rx,
                    },
                });
            }
            "encoder" => {
                let ch = config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Encoder {
                        logical_channel: ch,
                    },
                });
            }
            "ssd1306_display" => {
                let bus = config.get("i2c_bus").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                let addr = config
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0x3C) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::I2c {
                        logical_bus: bus,
                        address: addr,
                    },
                });
            }
            "tmc2209_stepper" => {
                let port = config
                    .get("uart_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::Stepper { logical_port: port },
                });
            }
            "tmc2209_stallguard" => {
                let port = config
                    .get("uart_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;
                let addr = config
                    .get("uart_addr")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;
                requirements.push(RequirementEntry {
                    block_id,
                    block_name: String::from(block_name),
                    requirement: PeripheralRequirement::StallGuard {
                        logical_port: port,
                        address: addr,
                    },
                });
            }
            _ => {} // Non-peripheral blocks have no hardware requirements
        }
    }

    RequirementSet { requirements }
}

// ---------------------------------------------------------------------------
// Target capability definitions
// ---------------------------------------------------------------------------

/// Get capabilities for a known target family.
pub fn capabilities_for(family: &str) -> Option<TargetCapabilities> {
    match family {
        "Host" => Some(host_capabilities()),
        "Rp2040" => Some(rp2040_capabilities()),
        "Stm32f4" => Some(stm32f4_capabilities()),
        "Esp32c3" => Some(esp32c3_capabilities()),
        "Stm32g0b1" => Some(stm32g0b1_capabilities()),
        _ => None,
    }
}

fn host_capabilities() -> TargetCapabilities {
    TargetCapabilities {
        family: String::from("Host"),
        display_name: String::from("Host Simulation"),
        adc_pins: (0..16)
            .map(|i| AdcCapability {
                pin: alloc::format!("SIM_ADC{i}"),
                peripheral: String::from("ADC"),
                hw_channel: i,
            })
            .collect(),
        pwm_pins: (0..16)
            .map(|i| PwmCapability {
                pin: alloc::format!("SIM_PWM{i}"),
                timer: String::from("SIM"),
                channel: alloc::format!("{i}"),
            })
            .collect(),
        gpio_pins: (0..32)
            .map(|i| GpioCapability {
                pin: alloc::format!("SIM_GPIO{i}"),
                can_input: true,
                can_output: true,
                has_pull: false,
            })
            .collect(),
        uart_peripherals: (0..4)
            .map(|i| UartCapability {
                peripheral: alloc::format!("SIM_UART{i}"),
                tx_pins: alloc::vec![alloc::format!("SIM_TX{i}")],
                rx_pins: alloc::vec![alloc::format!("SIM_RX{i}")],
            })
            .collect(),
        encoder_pins: Vec::new(),
        i2c_peripherals: Vec::new(),
        stepper_slots: Vec::new(),
    }
}

fn rp2040_capabilities() -> TargetCapabilities {
    TargetCapabilities {
        family: String::from("Rp2040"),
        display_name: String::from("Raspberry Pi Pico (RP2040)"),
        adc_pins: alloc::vec![
            AdcCapability {
                pin: String::from("GP26"),
                peripheral: String::from("ADC"),
                hw_channel: 0
            },
            AdcCapability {
                pin: String::from("GP27"),
                peripheral: String::from("ADC"),
                hw_channel: 1
            },
            AdcCapability {
                pin: String::from("GP28"),
                peripheral: String::from("ADC"),
                hw_channel: 2
            },
            AdcCapability {
                pin: String::from("GP29"),
                peripheral: String::from("ADC"),
                hw_channel: 3
            },
        ],
        pwm_pins: (0..16)
            .map(|i| {
                let slice = i / 2;
                let ch = if i % 2 == 0 { "A" } else { "B" };
                PwmCapability {
                    pin: alloc::format!("GP{i}"),
                    timer: alloc::format!("PWM_SLICE{slice}"),
                    channel: String::from(ch),
                }
            })
            .collect(),
        gpio_pins: (0..30)
            .map(|i| GpioCapability {
                pin: alloc::format!("GP{i}"),
                can_input: true,
                can_output: true,
                has_pull: true,
            })
            .collect(),
        uart_peripherals: alloc::vec![
            UartCapability {
                peripheral: String::from("UART0"),
                tx_pins: alloc::vec![
                    String::from("GP0"),
                    String::from("GP12"),
                    String::from("GP16")
                ],
                rx_pins: alloc::vec![
                    String::from("GP1"),
                    String::from("GP13"),
                    String::from("GP17")
                ],
            },
            UartCapability {
                peripheral: String::from("UART1"),
                tx_pins: alloc::vec![String::from("GP4"), String::from("GP8")],
                rx_pins: alloc::vec![String::from("GP5"), String::from("GP9")],
            },
        ],
        encoder_pins: Vec::new(),
        i2c_peripherals: alloc::vec![
            I2cCapability {
                peripheral: String::from("I2C0"),
                sda_pins: alloc::vec![
                    String::from("GP0"),
                    String::from("GP4"),
                    String::from("GP8"),
                    String::from("GP12"),
                    String::from("GP16"),
                    String::from("GP20")
                ],
                scl_pins: alloc::vec![
                    String::from("GP1"),
                    String::from("GP5"),
                    String::from("GP9"),
                    String::from("GP13"),
                    String::from("GP17"),
                    String::from("GP21")
                ],
            },
            I2cCapability {
                peripheral: String::from("I2C1"),
                sda_pins: alloc::vec![
                    String::from("GP2"),
                    String::from("GP6"),
                    String::from("GP10"),
                    String::from("GP14"),
                    String::from("GP18"),
                    String::from("GP26")
                ],
                scl_pins: alloc::vec![
                    String::from("GP3"),
                    String::from("GP7"),
                    String::from("GP11"),
                    String::from("GP15"),
                    String::from("GP19"),
                    String::from("GP27")
                ],
            },
        ],
        stepper_slots: Vec::new(),
    }
}

fn stm32f4_capabilities() -> TargetCapabilities {
    TargetCapabilities {
        family: String::from("Stm32f4"),
        display_name: String::from("STM32F401CC (Cortex-M4)"),
        adc_pins: alloc::vec![
            AdcCapability {
                pin: String::from("PA0"),
                peripheral: String::from("ADC1"),
                hw_channel: 0
            },
            AdcCapability {
                pin: String::from("PA1"),
                peripheral: String::from("ADC1"),
                hw_channel: 1
            },
            AdcCapability {
                pin: String::from("PA4"),
                peripheral: String::from("ADC1"),
                hw_channel: 4
            },
            AdcCapability {
                pin: String::from("PA5"),
                peripheral: String::from("ADC1"),
                hw_channel: 5
            },
            AdcCapability {
                pin: String::from("PA6"),
                peripheral: String::from("ADC1"),
                hw_channel: 6
            },
            AdcCapability {
                pin: String::from("PA7"),
                peripheral: String::from("ADC1"),
                hw_channel: 7
            },
            AdcCapability {
                pin: String::from("PB0"),
                peripheral: String::from("ADC1"),
                hw_channel: 8
            },
            AdcCapability {
                pin: String::from("PB1"),
                peripheral: String::from("ADC1"),
                hw_channel: 9
            },
        ],
        pwm_pins: alloc::vec![
            PwmCapability {
                pin: String::from("PA8"),
                timer: String::from("TIM1"),
                channel: String::from("1")
            },
            PwmCapability {
                pin: String::from("PA9"),
                timer: String::from("TIM1"),
                channel: String::from("2")
            },
            PwmCapability {
                pin: String::from("PA10"),
                timer: String::from("TIM1"),
                channel: String::from("3")
            },
            PwmCapability {
                pin: String::from("PA11"),
                timer: String::from("TIM1"),
                channel: String::from("4")
            },
            PwmCapability {
                pin: String::from("PA0"),
                timer: String::from("TIM2"),
                channel: String::from("1")
            },
            PwmCapability {
                pin: String::from("PA1"),
                timer: String::from("TIM2"),
                channel: String::from("2")
            },
        ],
        gpio_pins: {
            let mut pins = Vec::new();
            for port in ['A', 'B', 'C'] {
                let count = match port {
                    'A' => 16,
                    'B' => 16,
                    _ => 14,
                };
                for i in 0..count {
                    pins.push(GpioCapability {
                        pin: alloc::format!("P{port}{i}"),
                        can_input: true,
                        can_output: true,
                        has_pull: true,
                    });
                }
            }
            pins
        },
        uart_peripherals: alloc::vec![
            UartCapability {
                peripheral: String::from("USART1"),
                tx_pins: alloc::vec![String::from("PA9"), String::from("PB6")],
                rx_pins: alloc::vec![String::from("PA10"), String::from("PB7")],
            },
            UartCapability {
                peripheral: String::from("USART2"),
                tx_pins: alloc::vec![String::from("PA2")],
                rx_pins: alloc::vec![String::from("PA3")],
            },
            UartCapability {
                peripheral: String::from("USART6"),
                tx_pins: alloc::vec![String::from("PA11")],
                rx_pins: alloc::vec![String::from("PA12")],
            },
        ],
        encoder_pins: alloc::vec![
            EncoderCapability {
                timer: String::from("TIM2"),
                pin_a: String::from("PA0"),
                pin_b: String::from("PA1")
            },
            EncoderCapability {
                timer: String::from("TIM3"),
                pin_a: String::from("PA6"),
                pin_b: String::from("PA7")
            },
        ],
        i2c_peripherals: alloc::vec![I2cCapability {
            peripheral: String::from("I2C1"),
            sda_pins: alloc::vec![String::from("PB7"), String::from("PB9")],
            scl_pins: alloc::vec![String::from("PB6"), String::from("PB8")],
        },],
        stepper_slots: Vec::new(),
    }
}

fn esp32c3_capabilities() -> TargetCapabilities {
    TargetCapabilities {
        family: String::from("Esp32c3"),
        display_name: String::from("ESP32-C3 (RISC-V)"),
        adc_pins: alloc::vec![
            AdcCapability {
                pin: String::from("GPIO0"),
                peripheral: String::from("ADC1"),
                hw_channel: 0
            },
            AdcCapability {
                pin: String::from("GPIO1"),
                peripheral: String::from("ADC1"),
                hw_channel: 1
            },
            AdcCapability {
                pin: String::from("GPIO2"),
                peripheral: String::from("ADC1"),
                hw_channel: 2
            },
            AdcCapability {
                pin: String::from("GPIO3"),
                peripheral: String::from("ADC1"),
                hw_channel: 3
            },
            AdcCapability {
                pin: String::from("GPIO4"),
                peripheral: String::from("ADC1"),
                hw_channel: 4
            },
        ],
        pwm_pins: (0..6)
            .map(|i| PwmCapability {
                pin: alloc::format!("GPIO{i}"),
                timer: String::from("LEDC"),
                channel: alloc::format!("{i}"),
            })
            .collect(),
        gpio_pins: (0..22)
            .map(|i| GpioCapability {
                pin: alloc::format!("GPIO{i}"),
                can_input: true,
                can_output: true,
                has_pull: true,
            })
            .collect(),
        uart_peripherals: alloc::vec![
            UartCapability {
                peripheral: String::from("UART0"),
                tx_pins: alloc::vec![String::from("GPIO21")],
                rx_pins: alloc::vec![String::from("GPIO20")],
            },
            UartCapability {
                peripheral: String::from("UART1"),
                tx_pins: alloc::vec![String::from("GPIO0"), String::from("GPIO1")],
                rx_pins: alloc::vec![String::from("GPIO2"), String::from("GPIO3")],
            },
        ],
        encoder_pins: Vec::new(),
        i2c_peripherals: alloc::vec![I2cCapability {
            peripheral: String::from("I2C0"),
            sda_pins: alloc::vec![
                String::from("GPIO1"),
                String::from("GPIO3"),
                String::from("GPIO5")
            ],
            scl_pins: alloc::vec![
                String::from("GPIO0"),
                String::from("GPIO2"),
                String::from("GPIO4")
            ],
        },],
        stepper_slots: Vec::new(),
    }
}

fn stm32g0b1_capabilities() -> TargetCapabilities {
    TargetCapabilities {
        family: String::from("Stm32g0b1"),
        display_name: String::from("STM32G0B1CB (Cortex-M0+)"),
        adc_pins: alloc::vec![
            AdcCapability {
                pin: String::from("PA0"),
                peripheral: String::from("ADC1"),
                hw_channel: 0
            },
            AdcCapability {
                pin: String::from("PA1"),
                peripheral: String::from("ADC1"),
                hw_channel: 1
            },
            AdcCapability {
                pin: String::from("PA2"),
                peripheral: String::from("ADC1"),
                hw_channel: 2
            },
            AdcCapability {
                pin: String::from("PA3"),
                peripheral: String::from("ADC1"),
                hw_channel: 3
            },
            AdcCapability {
                pin: String::from("PA4"),
                peripheral: String::from("ADC1"),
                hw_channel: 4
            },
            AdcCapability {
                pin: String::from("PA5"),
                peripheral: String::from("ADC1"),
                hw_channel: 5
            },
            AdcCapability {
                pin: String::from("PA6"),
                peripheral: String::from("ADC1"),
                hw_channel: 6
            },
            AdcCapability {
                pin: String::from("PA7"),
                peripheral: String::from("ADC1"),
                hw_channel: 7
            },
            AdcCapability {
                pin: String::from("PB0"),
                peripheral: String::from("ADC1"),
                hw_channel: 8
            },
            AdcCapability {
                pin: String::from("PB1"),
                peripheral: String::from("ADC1"),
                hw_channel: 9
            },
        ],
        pwm_pins: alloc::vec![
            PwmCapability {
                pin: String::from("PA8"),
                timer: String::from("TIM1"),
                channel: String::from("1")
            },
            PwmCapability {
                pin: String::from("PA9"),
                timer: String::from("TIM1"),
                channel: String::from("2")
            },
            PwmCapability {
                pin: String::from("PA10"),
                timer: String::from("TIM1"),
                channel: String::from("3")
            },
            PwmCapability {
                pin: String::from("PA11"),
                timer: String::from("TIM1"),
                channel: String::from("4")
            },
        ],
        gpio_pins: {
            let mut pins = Vec::new();
            for port in ['A', 'B', 'C', 'D'] {
                let count = match port {
                    'A' => 16,
                    'B' => 16,
                    'C' => 16,
                    _ => 16,
                };
                for i in 0..count {
                    pins.push(GpioCapability {
                        pin: alloc::format!("P{port}{i}"),
                        can_input: true,
                        can_output: true,
                        has_pull: true,
                    });
                }
            }
            pins
        },
        uart_peripherals: alloc::vec![
            UartCapability {
                peripheral: String::from("USART1"),
                tx_pins: alloc::vec![String::from("PA9"), String::from("PB6")],
                rx_pins: alloc::vec![String::from("PA10"), String::from("PB7")],
            },
            UartCapability {
                peripheral: String::from("USART2"),
                tx_pins: alloc::vec![String::from("PA2"), String::from("PA14")],
                rx_pins: alloc::vec![String::from("PA3"), String::from("PA15")],
            },
            UartCapability {
                peripheral: String::from("USART3"),
                tx_pins: alloc::vec![
                    String::from("PB8"),
                    String::from("PB10"),
                    String::from("PC4")
                ],
                rx_pins: alloc::vec![
                    String::from("PB9"),
                    String::from("PB11"),
                    String::from("PC5")
                ],
            },
            UartCapability {
                peripheral: String::from("USART4"),
                tx_pins: alloc::vec![String::from("PA0"), String::from("PC10")],
                rx_pins: alloc::vec![String::from("PA1"), String::from("PC11")],
            },
        ],
        encoder_pins: alloc::vec![
            EncoderCapability {
                timer: String::from("TIM2"),
                pin_a: String::from("PA0"),
                pin_b: String::from("PA1")
            },
            EncoderCapability {
                timer: String::from("TIM3"),
                pin_a: String::from("PA6"),
                pin_b: String::from("PA7")
            },
        ],
        i2c_peripherals: alloc::vec![
            I2cCapability {
                peripheral: String::from("I2C1"),
                sda_pins: alloc::vec![String::from("PB7"), String::from("PB9")],
                scl_pins: alloc::vec![String::from("PB6"), String::from("PB8")],
            },
            I2cCapability {
                peripheral: String::from("I2C2"),
                sda_pins: alloc::vec![
                    String::from("PA12"),
                    String::from("PB11"),
                    String::from("PB14")
                ],
                scl_pins: alloc::vec![
                    String::from("PA11"),
                    String::from("PB10"),
                    String::from("PB13")
                ],
            },
        ],
        stepper_slots: alloc::vec![StepperCapability {
            label: String::from("X"),
            step_pins: alloc::vec![String::from("PA0"), String::from("PB0")],
            dir_pins: alloc::vec![String::from("PA1"), String::from("PB1")],
            enable_pins: alloc::vec![String::from("PA2"), String::from("PB2")],
            uart: Some(String::from("USART3")),
        },],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn extract_adc_requirement() {
        let config = serde_json::json!({"channel": 2});
        let blocks = [(1, "adc_block", "adc_source", &config)];
        let reqs = extract_requirements(&blocks);
        assert_eq!(reqs.requirements.len(), 1);
        assert_eq!(
            reqs.requirements[0].requirement,
            PeripheralRequirement::Adc { logical_channel: 2 }
        );
    }

    #[test]
    fn extract_ignores_non_peripheral_blocks() {
        let config = serde_json::json!({"value": 42.0});
        let blocks = [(1, "const", "constant", &config)];
        let reqs = extract_requirements(&blocks);
        assert!(reqs.requirements.is_empty());
    }

    #[test]
    fn validate_missing_assignment() {
        let reqs = RequirementSet {
            requirements: alloc::vec![RequirementEntry {
                block_id: 1,
                block_name: String::from("adc"),
                requirement: PeripheralRequirement::Adc { logical_channel: 0 },
            }],
        };
        let caps = rp2040_capabilities();
        let config = HardwareConfig {
            family: String::from("Rp2040"),
            assignments: alloc::vec![],
        };
        let result = validate_config(&reqs, &caps, &config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("no pin is assigned"));
    }

    #[test]
    fn validate_valid_adc_assignment() {
        let reqs = RequirementSet {
            requirements: alloc::vec![RequirementEntry {
                block_id: 1,
                block_name: String::from("adc"),
                requirement: PeripheralRequirement::Adc { logical_channel: 0 },
            }],
        };
        let caps = rp2040_capabilities();
        let config = HardwareConfig {
            family: String::from("Rp2040"),
            assignments: alloc::vec![PinAssignment::Adc {
                logical_channel: 0,
                pin: String::from("GP26"),
                peripheral: String::from("ADC"),
            }],
        };
        assert!(validate_config(&reqs, &caps, &config).is_ok());
    }

    #[test]
    fn validate_invalid_pin() {
        let reqs = RequirementSet {
            requirements: alloc::vec![],
        };
        let caps = rp2040_capabilities();
        let config = HardwareConfig {
            family: String::from("Rp2040"),
            assignments: alloc::vec![PinAssignment::Adc {
                logical_channel: 0,
                pin: String::from("GP0"), // GP0 is not ADC-capable on RP2040
                peripheral: String::from("ADC"),
            }],
        };
        let result = validate_config(&reqs, &caps, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].message.contains("not ADC-capable"));
    }

    #[test]
    fn validate_pin_conflict() {
        let reqs = RequirementSet {
            requirements: alloc::vec![],
        };
        let caps = rp2040_capabilities();
        let config = HardwareConfig {
            family: String::from("Rp2040"),
            assignments: alloc::vec![
                PinAssignment::Gpio {
                    logical_pin: 0,
                    pin: String::from("GP0"),
                    direction: GpioDirection::Output,
                },
                PinAssignment::Pwm {
                    logical_channel: 0,
                    pin: String::from("GP0"), // conflict!
                    timer: String::from("PWM_SLICE0"),
                },
            ],
        };
        let result = validate_config(&reqs, &caps, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.message.contains("multiple peripherals")));
    }

    #[test]
    fn capabilities_for_known_targets() {
        assert!(capabilities_for("Rp2040").is_some());
        assert!(capabilities_for("Stm32f4").is_some());
        assert!(capabilities_for("Esp32c3").is_some());
        assert!(capabilities_for("Stm32g0b1").is_some());
        assert!(capabilities_for("Host").is_some());
        assert!(capabilities_for("Unknown").is_none());
    }

    #[test]
    fn rp2040_has_4_adc_pins() {
        let caps = rp2040_capabilities();
        assert_eq!(caps.adc_pins.len(), 4);
        assert_eq!(caps.adc_pins[0].pin, "GP26");
    }

    #[test]
    fn serde_roundtrip_hardware_config() {
        let config = HardwareConfig {
            family: String::from("Rp2040"),
            assignments: alloc::vec![
                PinAssignment::Adc {
                    logical_channel: 0,
                    pin: String::from("GP26"),
                    peripheral: String::from("ADC"),
                },
                PinAssignment::Pwm {
                    logical_channel: 0,
                    pin: String::from("GP0"),
                    timer: String::from("PWM_SLICE0"),
                },
                PinAssignment::Gpio {
                    logical_pin: 13,
                    pin: String::from("GP13"),
                    direction: GpioDirection::Output,
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: HardwareConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.family, "Rp2040");
        assert_eq!(parsed.assignments.len(), 3);
    }

    #[test]
    fn serde_roundtrip_capabilities() {
        let caps = rp2040_capabilities();
        let json = serde_json::to_string(&caps).unwrap();
        let parsed: TargetCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.adc_pins.len(), 4);
    }
}
