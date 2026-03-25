//! Shared I2C bus topologies and network configuration for HIL firmware.
//!
//! Defines the simulated I2C bus topologies used by HIL board firmwares.
//! Each bus module exports a `Bus` type alias and a `build()` constructor.
//! Network constants (MAC addresses, IP addresses) are also shared.
//!
//! # Modules
//!
//! - [`i2c0`] — Simulated I2C0 bus with TCA9543A mux, TMP1075, and TCA9555 devices
//! - [`i2c1`] — Simulated I2C1 flat bus with 2× INA230
//! - [`i2c2`] — Simulated I2C2 flat bus with 2× TMP1075 and Eeprom256k
//! - [`i2c3`] — Simulated I2C3 flat bus with 2× TMP1075 and Eeprom256k
//! - [`i2c4`] — Simulated I2C4 flat bus with TPS546B24A and INA230
//! - [`i2c5`] — Simulated I2C5 flat bus with ADM1272 and BMR491
//! - [`i2c6`] — Simulated I2C6 flat bus with ISL68224 and RAA228926
//! - [`i2c7`] — Simulated I2C7 flat bus with EMC2305 and 2× TMP1075
//! - [`i2c8`] — Simulated I2C8 flat bus with LTC4287 and INA230
//! - [`i2c9`] — Simulated I2C9 flat bus with Eeprom256k and RegisterDevice
//! - [`network`] — MAC addresses and IP configuration constants

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_in_result)]

pub mod i2c0;
pub mod i2c1;
pub mod i2c2;
pub mod i2c3;
pub mod i2c4;
pub mod i2c5;
pub mod i2c6;
pub mod i2c7;
pub mod i2c8;
pub mod i2c9;
pub mod network;
