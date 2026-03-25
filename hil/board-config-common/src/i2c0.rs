//! Simulated I2C0 bus topology for board-support-pico.
//!
//! Defines the device set type and provides a builder function that
//! constructs the complete bus with:
//!
//! - TCA9543A 2-channel mux at 0x70
//!   - Channel 0: TMP1075 at 0x48 (25 °C), TCA9555 at 0x20
//!   - Channel 1: TMP1075 at 0x48 (30 °C), TCA9555 at 0x20

use i2c_hil_sim::devices::{I2cSwitchBuilder, Tca9555, Tmp1075};
use i2c_hil_sim::{Address, SimBus, SimBusBuilder};

/// Type alias for the I2C0 device set.
///
/// ```text
/// (I2cSwitch<((Tmp1075, (Tca9555, ())), ((Tmp1075, (Tca9555, ())), ()))>, ())
/// ```
pub type DeviceSet = (
    i2c_hil_sim::devices::I2cSwitch<((Tmp1075, (Tca9555, ())), ((Tmp1075, (Tca9555, ())), ()))>,
    (),
);

/// Type alias for the complete I2C0 bus.
pub type Bus = SimBus<DeviceSet>;

/// Builds the I2C0 simulated bus with the standard board topology.
///
/// Returns a [`SimBus`] containing a TCA9543A mux at 0x70 with two
/// channels, each hosting a TMP1075 temperature sensor and a TCA9555
/// I/O expander.
///
/// # Panics
///
/// Panics if any hardcoded I2C address is outside the valid 7-bit range.
/// All addresses in this module are compile-time constants, so this
/// cannot occur in practice.
pub fn build() -> Bus {
    let mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel_with_devices((
            Tmp1075::with_temperature(Address::new(0x48).unwrap(), Tmp1075::celsius_to_raw(25.0)),
            (Tca9555::new(Address::new(0x20).unwrap()), ()),
        ))
        .channel_with_devices((
            Tmp1075::with_temperature(Address::new(0x48).unwrap(), Tmp1075::celsius_to_raw(30.0)),
            (Tca9555::new(Address::new(0x20).unwrap()), ()),
        ))
        .build();
    SimBusBuilder::new().with_switch(mux).build()
}
