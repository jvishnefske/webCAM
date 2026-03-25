//! Dashboard UI components.
//!
//! Each submodule provides a Leptos component for a specific section of the
//! HIL dashboard: bus overview, temperature, power, fan control, I2C console,
//! and firmware update.

pub mod bus_overview;
pub mod fan;
pub mod firmware_update;
pub mod i2c_console;
pub mod power;
pub mod temperature;
