//! Simulated I2C4 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - TPS546B24A buck converter at 0x10
//! - INA230 current/power monitor at 0x40

use i2c_hil_devices::{Ina230, Tps546b24a};
use i2c_hil_sim::{Address, PmBusEngine, SimBus, SimBusBuilder};

/// Type alias for the I2C4 device set.
///
/// ```text
/// (Ina230, (PmBusEngine<Tps546b24a>, ()))
/// ```
pub type DeviceSet = (Ina230, (PmBusEngine<Tps546b24a>, ()));

/// Type alias for the complete I2C4 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C4 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing a TPS546B24A buck converter and an
/// INA230 current/power monitor on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(PmBusEngine::new(Tps546b24a::new(
            Address::new(0x10).unwrap(),
        )))
        .with_device(Ina230::new(Address::new(0x40).unwrap()))
        .build()
}
