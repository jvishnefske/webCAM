//! Builder for constructing [`SimBus`](crate::SimBus) instances with devices.
//!
//! The builder uses a type-changing pattern: each call to
//! [`with_device`](SimBusBuilder::with_device) wraps the existing device
//! list in a new tuple layer, producing a new builder type.

use crate::bus::shared::SharedBus;
use crate::bus::SimBus;
use crate::device::I2cDevice;
use crate::device_set::DeviceSet;
use crate::devices::i2c_switch::{ChannelSet, I2cSwitch};

/// Builder for constructing a [`SimBus`](crate::SimBus) with a set of slave
/// devices.
///
/// Each call to [`with_device`](Self::with_device) prepends a device to the
/// type-level linked list. The builder type parameter changes with each
/// addition.
///
/// # Example
///
/// ```rust
/// use i2c_hil_sim::{SimBusBuilder, Address};
/// use i2c_hil_sim::devices::RegisterDevice;
///
/// let mut bus = SimBusBuilder::new()
///     .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 8]))
///     .with_device(RegisterDevice::new(Address::new(0x68).unwrap(), [0u8; 4]))
///     .build();
/// ```
pub struct SimBusBuilder<D: DeviceSet> {
    devices: D,
}

impl SimBusBuilder<()> {
    /// Creates a new builder with an empty device set.
    pub fn new() -> Self {
        Self { devices: () }
    }
}

impl<D: DeviceSet> SimBusBuilder<D> {
    /// Adds a device to the bus under construction.
    ///
    /// The device's address must not already be present in the device set.
    /// This is checked at runtime and will panic if a duplicate is detected.
    /// Panicking here is acceptable because bus construction is a setup-time
    /// operation, not a runtime transaction.
    ///
    /// # Panics
    ///
    /// Panics if a device with the same address is already registered on
    /// this bus.
    pub fn with_device<Dev: I2cDevice>(self, device: Dev) -> SimBusBuilder<(Dev, D)> {
        assert!(
            !self.devices.contains_address(device.address().raw()),
            "duplicate I2C address 0x{:02x} on bus",
            device.address().raw(),
        );
        SimBusBuilder {
            devices: (device, self.devices),
        }
    }

    /// Adds an I2C switch to the bus under construction.
    ///
    /// The switch's own address must not already be present in the device
    /// set. Downstream devices behind the switch are isolated and do not
    /// conflict with bus-level addresses.
    ///
    /// # Panics
    ///
    /// Panics if the switch's address is already registered on this bus.
    pub fn with_switch<C: ChannelSet>(
        self,
        switch: I2cSwitch<C>,
    ) -> SimBusBuilder<(I2cSwitch<C>, D)> {
        assert!(
            !self.devices.contains_address(switch.address().raw()),
            "duplicate I2C address 0x{:02x} on bus",
            switch.address().raw(),
        );
        SimBusBuilder {
            devices: (switch, self.devices),
        }
    }

    /// Finalizes the builder and returns the configured [`SimBus`](crate::SimBus).
    ///
    /// The returned bus owns all registered devices and is ready to
    /// process transactions.
    pub fn build(self) -> SimBus<D> {
        SimBus::new(self.devices)
    }

    /// Finalizes the builder and returns a [`SharedBus`] that supports
    /// multiple [`BusHandle`](crate::BusHandle) references.
    ///
    /// This is a convenience shortcut for `SharedBus::new(builder.build())`.
    pub fn build_shared(self) -> SharedBus<D> {
        SharedBus::new(self.build())
    }
}

impl Default for SimBusBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}
