//! Pin/peripheral binding model for target-specific code generation.
//!
//! The canonical hardware configuration model lives in [`module_traits::hardware`].
//! This module provides codegen-specific types and conversions.

use serde::{Deserialize, Serialize};

use super::target::TargetFamily;

// Re-export the canonical types from module-traits for frontend/runtime use.
pub use module_traits::hardware::{
    capabilities_for, extract_requirements, validate_config, ConfigError, ConfigSeverity,
    GpioDirection, HardwareConfig, PeripheralRequirement, PinAssignment, RequirementEntry,
    RequirementSet, TargetCapabilities,
};

/// A complete set of pin bindings for a target (codegen representation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub target: TargetFamily,
    pub pins: Vec<PinBinding>,
}

/// A single pin/peripheral binding (codegen representation).
///
/// This type is used by target generators to emit hardware initialization code.
/// For the canonical serializable model, see [`PinAssignment`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PinBinding {
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
    I2cDisplay {
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
        uart_tx: String,
        uart_rx: String,
        peripheral: String,
    },
}

/// A target with its binding, used as input to workspace generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetWithBinding {
    pub target: TargetFamily,
    pub binding: Binding,
}

impl Binding {
    /// Generate a simulated binding for the host target.
    pub fn host_default() -> Self {
        Self {
            target: TargetFamily::Host,
            pins: vec![],
        }
    }

    /// Convert a [`HardwareConfig`] to a codegen [`Binding`].
    pub fn from_hardware_config(config: &HardwareConfig, target: TargetFamily) -> Self {
        let pins = config
            .assignments
            .iter()
            .map(PinBinding::from_assignment)
            .collect();
        Self { target, pins }
    }
}

impl PinBinding {
    /// Convert a [`PinAssignment`] to a codegen [`PinBinding`].
    fn from_assignment(assignment: &PinAssignment) -> Self {
        match assignment {
            PinAssignment::Adc {
                logical_channel,
                pin,
                peripheral,
            } => PinBinding::Adc {
                logical_channel: *logical_channel,
                pin: pin.clone(),
                peripheral: peripheral.clone(),
            },
            PinAssignment::Pwm {
                logical_channel,
                pin,
                timer,
            } => PinBinding::Pwm {
                logical_channel: *logical_channel,
                pin: pin.clone(),
                timer: timer.clone(),
            },
            PinAssignment::Gpio {
                logical_pin, pin, ..
            } => PinBinding::Gpio {
                logical_pin: *logical_pin,
                pin: pin.clone(),
            },
            PinAssignment::Uart {
                logical_port,
                tx_pin,
                rx_pin,
                peripheral,
            } => PinBinding::Uart {
                logical_port: *logical_port,
                tx_pin: tx_pin.clone(),
                rx_pin: rx_pin.clone(),
                peripheral: peripheral.clone(),
            },
            PinAssignment::Encoder {
                logical_channel,
                pin_a,
                pin_b,
                timer,
            } => PinBinding::Encoder {
                logical_channel: *logical_channel,
                pin_a: pin_a.clone(),
                pin_b: pin_b.clone(),
                timer: timer.clone(),
            },
            PinAssignment::I2c {
                logical_bus,
                sda_pin,
                scl_pin,
                peripheral,
            } => PinBinding::I2cDisplay {
                logical_bus: *logical_bus,
                sda_pin: sda_pin.clone(),
                scl_pin: scl_pin.clone(),
                peripheral: peripheral.clone(),
            },
            PinAssignment::Stepper {
                logical_port,
                step_pin,
                dir_pin,
                enable_pin,
                uart_peripheral,
            } => PinBinding::Stepper {
                logical_port: *logical_port,
                step_pin: step_pin.clone(),
                dir_pin: dir_pin.clone(),
                enable_pin: enable_pin.clone(),
                uart_tx: String::new(),
                uart_rx: String::new(),
                peripheral: uart_peripheral.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_default_is_empty() {
        let b = Binding::host_default();
        assert_eq!(b.target, TargetFamily::Host);
        assert!(b.pins.is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let b = Binding {
            target: TargetFamily::Rp2040,
            pins: vec![
                PinBinding::Adc {
                    logical_channel: 0,
                    pin: "PIN_26".to_string(),
                    peripheral: "ADC".to_string(),
                },
                PinBinding::Pwm {
                    logical_channel: 0,
                    pin: "PIN_16".to_string(),
                    timer: "PWM_SLICE0".to_string(),
                },
                PinBinding::Gpio {
                    logical_pin: 0,
                    pin: "PIN_25".to_string(),
                },
                PinBinding::Uart {
                    logical_port: 0,
                    tx_pin: "PIN_0".to_string(),
                    rx_pin: "PIN_1".to_string(),
                    peripheral: "UART0".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&b).unwrap();
        let parsed: Binding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.target, TargetFamily::Rp2040);
        assert_eq!(parsed.pins.len(), 4);
    }

    #[test]
    fn from_hardware_config_converts() {
        let hw_config = HardwareConfig {
            family: "Rp2040".to_string(),
            assignments: vec![
                PinAssignment::Adc {
                    logical_channel: 0,
                    pin: "GP26".to_string(),
                    peripheral: "ADC".to_string(),
                },
                PinAssignment::Gpio {
                    logical_pin: 13,
                    pin: "GP13".to_string(),
                    direction: GpioDirection::Output,
                },
            ],
        };
        let binding = Binding::from_hardware_config(&hw_config, TargetFamily::Rp2040);
        assert_eq!(binding.target, TargetFamily::Rp2040);
        assert_eq!(binding.pins.len(), 2);
        assert!(matches!(
            binding.pins[0],
            PinBinding::Adc {
                logical_channel: 0,
                ..
            }
        ));
        assert!(matches!(
            binding.pins[1],
            PinBinding::Gpio {
                logical_pin: 13,
                ..
            }
        ));
    }
}
