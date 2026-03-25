//! Simulated I2C device models for hardware-in-the-loop testing.
//!
//! This crate provides device simulations that implement
//! [`i2c_hil_sim::SmBusWordDevice`] or [`i2c_hil_sim::I2cDevice`],
//! modelling real I2C peripherals for use with [`i2c_hil_sim::SimBus`].
//!
//! # Available Devices
//!
//! - [`Ina230`] -- TI INA230 high-/low-side power/current monitor
//! - [`Ltc4287`] -- Analog Devices LTC4287 high power positive hot swap controller
//! - [`Emc2305`] -- Microchip EMC2305 5-fan PWM controller
//! - [`Adm1272`] -- Analog Devices ADM1272 hot swap controller (PMBus engine)
//! - [`Tps546b24a`] -- TI TPS546B24A buck converter (PMBus engine)
//! - [`Bmr491`] -- Flex BMR491 DC-DC converter (PMBus engine)
//! - [`Bmr4696001`] -- Flex BMR4696001 dual-output PoL DC-DC converter (PMBus engine)
//! - [`Isl68224`] -- Renesas ISL68224 triple-output PWM controller (PMBus engine)
//! - [`Raa228926`] -- Renesas RAA228926 dual-output PWM controller (PMBus engine)

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_in_result)]

pub mod adm1272;
pub mod bmr4696001;
pub mod bmr491;
pub mod emc2305;
pub mod ina230;
pub mod isl68224;
pub mod ltc4287;
pub mod raa228926;
pub mod tps546b24a;

pub use adm1272::Adm1272;
pub use bmr4696001::Bmr4696001;
pub use bmr491::Bmr491;
pub use emc2305::Emc2305;
pub use ina230::Ina230;
pub use isl68224::Isl68224;
pub use ltc4287::Ltc4287;
pub use raa228926::Raa228926;
pub use tps546b24a::Tps546b24a;
