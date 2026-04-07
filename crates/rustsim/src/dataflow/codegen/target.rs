//! Target definitions for multi-target code generation.

use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

/// Target MCU family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
pub enum TargetFamily {
    Host,
    Rp2040,
    Stm32f4,
    Esp32c3,
    Stm32g0b1,
}

/// Counts of available peripherals on a target.
#[derive(Debug, Clone)]
pub struct PeripheralSet {
    pub adc_channels: u8,
    pub pwm_channels: u8,
    pub gpio_pins: u8,
    pub uart_ports: u8,
}

/// A specific target board/chip definition.
#[derive(Debug, Clone)]
pub struct TargetDef {
    pub family: TargetFamily,
    pub name: &'static str,
    pub rust_target: &'static str,
    pub embassy_chip: &'static str,
    pub peripherals: PeripheralSet,
}

/// All supported targets, hardcoded to keep WASM deps minimal.
pub fn all_targets() -> Vec<TargetDef> {
    vec![
        TargetDef {
            family: TargetFamily::Host,
            name: "host-sim",
            rust_target: "",
            embassy_chip: "",
            peripherals: PeripheralSet {
                adc_channels: 16,
                pwm_channels: 16,
                gpio_pins: 32,
                uart_ports: 4,
            },
        },
        TargetDef {
            family: TargetFamily::Rp2040,
            name: "rp2040-pico",
            rust_target: "thumbv6m-none-eabi",
            embassy_chip: "rp",
            peripherals: PeripheralSet {
                adc_channels: 4,
                pwm_channels: 16,
                gpio_pins: 30,
                uart_ports: 2,
            },
        },
        TargetDef {
            family: TargetFamily::Stm32f4,
            name: "stm32f401cc",
            rust_target: "thumbv7em-none-eabihf",
            embassy_chip: "stm32f401cc",
            peripherals: PeripheralSet {
                adc_channels: 10,
                pwm_channels: 12,
                gpio_pins: 50,
                uart_ports: 3,
            },
        },
        TargetDef {
            family: TargetFamily::Esp32c3,
            name: "esp32c3",
            rust_target: "riscv32imc-unknown-none-elf",
            embassy_chip: "esp32c3",
            peripherals: PeripheralSet {
                adc_channels: 6,
                pwm_channels: 6,
                gpio_pins: 22,
                uart_ports: 2,
            },
        },
        TargetDef {
            family: TargetFamily::Stm32g0b1,
            name: "stm32g0b1cb",
            rust_target: "thumbv6m-none-eabi",
            embassy_chip: "stm32g0b1cb",
            peripherals: PeripheralSet {
                adc_channels: 16,
                pwm_channels: 12,
                gpio_pins: 64,
                uart_ports: 4,
            },
        },
    ]
}

/// Look up the target definition for a family.
pub fn target_for(family: TargetFamily) -> Option<TargetDef> {
    all_targets().into_iter().find(|t| t.family == family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_targets_has_five() {
        assert_eq!(all_targets().len(), 5);
    }

    #[test]
    fn target_for_rp2040() {
        let t = target_for(TargetFamily::Rp2040).unwrap();
        assert_eq!(t.name, "rp2040-pico");
        assert_eq!(t.rust_target, "thumbv6m-none-eabi");
    }

    #[test]
    fn target_for_host() {
        let t = target_for(TargetFamily::Host).unwrap();
        assert_eq!(t.rust_target, "");
    }

    #[test]
    fn serde_roundtrip() {
        let json = serde_json::to_string(&TargetFamily::Esp32c3).unwrap();
        let parsed: TargetFamily = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TargetFamily::Esp32c3);
    }
}
