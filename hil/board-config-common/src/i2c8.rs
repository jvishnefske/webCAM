//! Simulated I2C8 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - LTC4287 hot swap controller at 0x44
//! - INA230 current/power monitor at 0x45

use i2c_hil_devices::{Ina230, Ltc4287};
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C8 device set.
///
/// ```text
/// (Ina230, (Ltc4287, ()))
/// ```
pub type DeviceSet = (Ina230, (Ltc4287, ()));

/// Type alias for the complete I2C8 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C8 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing an LTC4287 hot swap controller and
/// an INA230 current/power monitor on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(Ltc4287::new(Address::new(0x44).unwrap()))
        .with_device(Ina230::new(Address::new(0x45).unwrap()))
        .build()
}
