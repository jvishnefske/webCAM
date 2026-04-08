//! Simulated I2C bus for hardware-in-the-loop testing.
//!
//! This crate provides [`SimBus`], an implementation of
//! [`embedded_hal::i2c::I2c`] that routes transactions to simulated
//! slave devices by address. It is `no_std` compatible and uses no
//! `unsafe` code, heap allocation, or shared-ownership primitives.
//!
//! # Architecture
//!
//! Devices are stored in a type-level linked list `(Dev, (Dev, ()))`.
//! The [`SimBusBuilder`] constructs the list incrementally, with each
//! [`with_device`](SimBusBuilder::with_device) call wrapping the
//! existing list in a new tuple layer:
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_sim::devices::RegisterDevice;
//!
//! let mut bus = SimBusBuilder::new()
//!     .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 256]))
//!     .with_device(RegisterDevice::new(Address::new(0x68).unwrap(), [0u8; 16]))
//!     .build();
//! ```
//!
//! # I2C Switches
//!
//! [`I2cSwitch`](devices::I2cSwitch) models multiplexer/switch ICs (e.g.
//! TCA9543A, TCA9548A) with a configurable number of channels:
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_sim::devices::{I2cSwitchBuilder, Tmp1075};
//!
//! let mut mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
//!     .channel(Tmp1075::new(Address::new(0x48).unwrap()))
//!     .channel(Tmp1075::new(Address::new(0x49).unwrap()))
//!     .build();
//! ```
//!
//! # Multiple Busses
//!
//! Each [`SimBus`] is an independent bus. Create multiple for multi-bus
//! scenarios:
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_sim::devices::RegisterDevice;
//!
//! let mut bus0 = SimBusBuilder::new()
//!     .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 256]))
//!     .build();
//!
//! let mut bus1 = SimBusBuilder::new()
//!     .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 256]))
//!     .build();
//! // bus0 and bus1 are independent -- same address on different busses is fine
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_in_result)]

pub mod bus;
pub mod channel;
pub mod device;
pub mod device_set;
pub mod devices;
pub mod error;
pub mod pmbus;
pub mod runtime;
pub mod smbus;

pub use channel::{I2cResponse, I2cTransaction};
pub use bus::builder::SimBusBuilder;
pub use bus::shared::{BusHandle, SharedBus};
pub use bus::SimBus;
pub use device::{Address, I2cDevice};
pub use error::BusError;
pub use pmbus::{PmBusAccess, PmBusDevice, PmBusEngine, PmBusKind, PmBusRegDesc, PmBusValue};
pub use runtime::{RuntimeBus, RuntimeDevice};
pub use smbus::SmBusWordDevice;
