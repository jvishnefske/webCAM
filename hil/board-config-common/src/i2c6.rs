//! Simulated I2C6 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - ISL68224 triple PWM controller at 0x60
//! - RAA228926 dual PWM controller at 0x61

use i2c_hil_devices::{Isl68224, Raa228926};
use i2c_hil_sim::{Address, PmBusEngine, SimBus, SimBusBuilder};

/// Type alias for the I2C6 device set.
///
/// ```text
/// (PmBusEngine<Raa228926>, (PmBusEngine<Isl68224>, ()))
/// ```
pub type DeviceSet = (PmBusEngine<Raa228926>, (PmBusEngine<Isl68224>, ()));

/// Type alias for the complete I2C6 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C6 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing an ISL68224 triple PWM controller and
/// a RAA228926 dual PWM controller on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(PmBusEngine::new(Isl68224::new(Address::new(0x60).unwrap())))
        .with_device(PmBusEngine::new(Raa228926::new(
            Address::new(0x61).unwrap(),
        )))
        .build()
}
