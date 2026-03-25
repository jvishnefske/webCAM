//! Simulated I2C5 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - ADM1272 hot swap controller at 0x10
//! - BMR4696001 dual-output PoL DC-DC converter at 0x20
//! - BMR491 DC-DC converter at 0x54

use i2c_hil_devices::{Adm1272, Bmr4696001, Bmr491};
use i2c_hil_sim::{Address, PmBusEngine, SimBus, SimBusBuilder};

/// Type alias for the I2C5 device set.
///
/// ```text
/// (PmBusEngine<Bmr491>, (PmBusEngine<Bmr4696001>, (PmBusEngine<Adm1272>, ())))
/// ```
pub type DeviceSet = (
    PmBusEngine<Bmr491>,
    (PmBusEngine<Bmr4696001>, (PmBusEngine<Adm1272>, ())),
);

/// Type alias for the complete I2C5 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C5 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing an ADM1272 hot swap controller,
/// a BMR4696001 dual-output PoL converter, and a BMR491 DC-DC converter
/// on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(PmBusEngine::new(Adm1272::new(Address::new(0x10).unwrap())))
        .with_device(PmBusEngine::new(Bmr4696001::new(
            Address::new(0x20).unwrap(),
        )))
        .with_device(PmBusEngine::new(Bmr491::new(Address::new(0x54).unwrap())))
        .build()
}
