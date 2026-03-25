//! Simulated I2C bus implementing [`embedded_hal::i2c::I2c`].
//!
//! [`SimBus`] owns a type-level device list and routes transactions to
//! the correct slave device by matching 7-bit addresses. Construction
//! uses the builder pattern via [`SimBusBuilder`](crate::SimBusBuilder).

pub mod builder;
pub mod shared;

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::device_set::DeviceSet;
use crate::error::BusError;

/// A simulated I2C bus that routes transactions to slave devices by address.
///
/// The bus owns all its devices through a type-level linked list `D`.
///
/// # Example
///
/// ```rust
/// use i2c_hil_sim::{SimBusBuilder, Address};
/// use i2c_hil_sim::devices::RegisterDevice;
///
/// let mut bus = SimBusBuilder::new()
///     .with_device(RegisterDevice::new(
///         Address::new(0x48).unwrap(),
///         [0x00u8; 256],
///     ))
///     .build();
/// ```
pub struct SimBus<D: DeviceSet> {
    devices: D,
}

impl<D: DeviceSet> SimBus<D> {
    /// Creates a new `SimBus` with the given device set.
    ///
    /// Prefer using [`SimBusBuilder`](crate::SimBusBuilder) for ergonomic
    /// construction.
    pub(crate) fn new(devices: D) -> Self {
        Self { devices }
    }

    /// Returns a shared reference to the device set.
    ///
    /// Useful for inspecting device state in tests via tuple access
    /// (e.g., `bus.devices().0`).
    pub fn devices(&self) -> &D {
        &self.devices
    }
}

impl<D: DeviceSet> ErrorType for SimBus<D> {
    type Error = BusError;
}

impl<D: DeviceSet> I2c for SimBus<D> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.devices.dispatch(address, operations)
    }
}
