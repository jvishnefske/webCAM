//! Simulated I2C7 bus topology for board-support-pico.
//!
//! Defines a flat bus (no mux) with:
//!
//! - EMC2305 5-fan PWM controller at 0x2C
//! - TMP1075 temperature sensor at 0x48 (55 °C)
//! - TMP1075 temperature sensor at 0x49 (60 °C)

use i2c_hil_devices::Emc2305;
use i2c_hil_sim::devices::Tmp1075;
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C7 device set.
///
/// ```text
/// (Tmp1075, (Tmp1075, (Emc2305, ())))
/// ```
pub type DeviceSet = (Tmp1075, (Tmp1075, (Emc2305, ())));

/// Type alias for the complete I2C7 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C7 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing an EMC2305 fan controller and two
/// TMP1075 temperature sensors on a flat bus (no mux).
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    SimBusBuilder::new()
        .with_device(Emc2305::new(Address::new(0x2C).unwrap()))
        .with_device(Tmp1075::with_temperature(
            Address::new(0x48).unwrap(),
            Tmp1075::celsius_to_raw(55.0),
        ))
        .with_device(Tmp1075::with_temperature(
            Address::new(0x49).unwrap(),
            Tmp1075::celsius_to_raw(60.0),
        ))
        .build()
}
