//! Trait definition for simulated I2C slave devices.
//!
//! Implementors of [`I2cDevice`] represent individual I2C slave peripherals
//! that respond to transactions at a fixed 7-bit address.

use embedded_hal::i2c::Operation;

use crate::error::BusError;

/// A 7-bit I2C slave address in right-aligned form (`0x00..=0x7F`).
///
/// This newtype enforces that addresses are valid 7-bit values at
/// construction time, preventing invalid addresses from entering the
/// system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(u8);

impl Address {
    /// Creates a new 7-bit I2C address.
    ///
    /// Returns `None` if `raw` exceeds `0x7F`.
    pub const fn new(raw: u8) -> Option<Self> {
        if raw > 0x7F {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// Returns the raw 7-bit address value.
    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// A simulated I2C slave device that processes transactions.
///
/// Implementations model the register-level behavior of a specific I2C
/// peripheral. The simulator routes transactions to the correct device
/// by matching the transaction address against [`I2cDevice::address`].
///
/// # Contract
///
/// - [`address`](I2cDevice::address) must return the same value for the
///   lifetime of the device. It is called on every transaction for routing.
/// - [`process`](I2cDevice::process) receives the full `&mut [Operation]`
///   slice for a single `transaction()` call and must handle all operations
///   in sequence.
pub trait I2cDevice {
    /// Returns the 7-bit address this device responds to.
    fn address(&self) -> Address;

    /// Processes a sequence of I2C operations directed at this device.
    ///
    /// The operations slice matches exactly what was passed to
    /// [`embedded_hal::i2c::I2c::transaction`]. For a
    /// [`Operation::Write`] operation, the device should consume the written
    /// bytes. For a [`Operation::Read`] operation, the device should fill
    /// the buffer with response data.
    ///
    /// # Errors
    ///
    /// Returns [`BusError::DataNak`] if the device cannot process the
    /// operations (e.g., writing to a read-only register).
    /// Returns [`BusError::DeviceError`] for internal device failures.
    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError>;
}
