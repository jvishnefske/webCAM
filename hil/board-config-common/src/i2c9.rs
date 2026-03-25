//! Simulated I2C9 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - Eeprom256k at 0x50
//! - RegisterDevice<16> at 0x30

use i2c_hil_sim::devices::{Eeprom256k, RegisterDevice};
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C9 device set.
///
/// ```text
/// (RegisterDevice<16>, (Eeprom256k, ()))
/// ```
pub type DeviceSet = (RegisterDevice<16>, (Eeprom256k, ()));

/// Type alias for the complete I2C9 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C9 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing a 256-Kbit EEPROM and a 16-byte
/// register device on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(Eeprom256k::new(Address::new(0x50).unwrap()))
        .with_device(RegisterDevice::new(Address::new(0x30).unwrap(), [0u8; 16]))
        .build()
}
