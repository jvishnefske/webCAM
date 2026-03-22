//! Pin/peripheral binding model for target-specific code generation.

use serde::{Deserialize, Serialize};

use super::target::TargetFamily;

/// A complete set of pin bindings for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub target: TargetFamily,
    pub pins: Vec<PinBinding>,
}

/// A single pin/peripheral binding.
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
}
