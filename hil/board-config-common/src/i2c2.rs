//! Simulated I2C2 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - TMP1075 at 0x48 (35 °C)
//! - TMP1075 at 0x49 (40 °C)
//! - Eeprom256k at 0x50

use i2c_hil_sim::devices::{Eeprom256k, Tmp1075};
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C2 device set.
///
/// ```text
/// (Eeprom256k, (Tmp1075, (Tmp1075, ())))
/// ```
pub type DeviceSet = (Eeprom256k, (Tmp1075, (Tmp1075, ())));

/// Type alias for the complete I2C2 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C2 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing two TMP1075 temperature sensors and
/// one 256-Kbit EEPROM on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(Tmp1075::with_temperature(
            Address::new(0x48).unwrap(),
            Tmp1075::celsius_to_raw(35.0),
        ))
        .with_device(Tmp1075::with_temperature(
            Address::new(0x49).unwrap(),
            Tmp1075::celsius_to_raw(40.0),
        ))
        .with_device(Eeprom256k::new(Address::new(0x50).unwrap()))
        .build()
}
