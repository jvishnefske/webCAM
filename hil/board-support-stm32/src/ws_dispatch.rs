//! Chip-specific I2C bus set type aliases.
//!
//! Each STM32 chip variant gets a bus set sized to fit its available
//! SRAM:
//!
//! | Chip | Buses | Devices/bus | Approx RAM |
//! |------|-------|-------------|------------|
//! | STM32F401CC | 2 | 4 | ~2.3 KB |
//! | STM32F411CE | 4 | 8 | ~9.4 KB |
//! | STM32H743VI | 10 | 8 | ~23 KB |

/// Chip-specific bus set type alias for STM32F401CC (64 KB SRAM).
#[cfg(feature = "stm32f401cc")]
pub type StmBusSet = hil_firmware_support::runtime_buses::RuntimeBusSet<2, 4>;

/// Chip-specific bus set type alias for STM32F411CE (128 KB SRAM).
#[cfg(feature = "stm32f411ce")]
pub type StmBusSet = hil_firmware_support::runtime_buses::RuntimeBusSet<4, 8>;

/// Chip-specific bus set type alias for STM32H743VI (1 MB SRAM).
#[cfg(feature = "stm32h743vi")]
pub type StmBusSet = hil_firmware_support::runtime_buses::RuntimeBusSet<10, 8>;
