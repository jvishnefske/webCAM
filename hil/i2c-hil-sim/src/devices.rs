//! Built-in simulated I2C slave device implementations.
//!
//! These provide ready-to-use device models for common I2C peripheral
//! patterns.

pub mod eeprom;
pub mod i2c_switch;
pub mod register;
pub mod tca9555;
pub mod tmp1075;

pub use eeprom::{Eeprom, Eeprom256k, Eeprom2k};
pub use i2c_switch::builder::I2cSwitchBuilder;
pub use i2c_switch::{ChannelSet, I2cSwitch};
pub use register::RegisterDevice;
pub use tca9555::{SimPins, Tca9555, Tca9555Pins};
pub use tmp1075::Tmp1075;
