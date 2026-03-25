//! Simulated I2C1 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - INA230 at 0x41 (current/power monitor)
//! - INA230 at 0x42 (current/power monitor)

use i2c_hil_devices::Ina230;
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C1 device set.
///
/// ```text
/// (Ina230, (Ina230, ()))
/// ```
pub type DeviceSet = (Ina230, (Ina230, ()));

/// Type alias for the complete I2C1 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C1 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing two INA230 current/power monitors
/// on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(Ina230::new(Address::new(0x41).unwrap()))
        .with_device(Ina230::new(Address::new(0x42).unwrap()))
        .build()
}
