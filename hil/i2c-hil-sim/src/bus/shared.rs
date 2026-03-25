//! Shared bus wrapper enabling multiple independent I2C handles.
//!
//! [`SharedBus`] wraps a [`SimBus`](crate::SimBus) in a `RefCell` and hands
//! out lightweight [`BusHandle`] references that each implement
//! [`embedded_hal::i2c::I2c`]. This lets multiple driver abstractions
//! (e.g., one for an EEPROM, one for a temperature sensor) each hold
//! their own `impl I2c` handle while sharing the same underlying bus.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, SharedBus, Address};
//! use i2c_hil_sim::devices::RegisterDevice;
//! use embedded_hal::i2c::I2c;
//!
//! let shared = SimBusBuilder::new()
//!     .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
//!     .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 8]))
//!     .build_shared();
//!
//! let mut h1 = shared.handle();
//! let mut h2 = shared.handle();
//!
//! let mut buf = [0u8; 2];
//! h1.read(0x48, &mut buf).unwrap();
//! h2.write(0x50, &[0x00, 0xFF]).unwrap();
//! ```

use core::cell::RefCell;

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::bus::SimBus;
use crate::device_set::DeviceSet;
use crate::error::BusError;

/// A shared I2C bus that allows multiple [`BusHandle`] references.
///
/// Internally wraps a [`SimBus`] in a [`RefCell`] for single-threaded
/// interior mutability. Each [`BusHandle`] borrows the bus only for the
/// duration of a single `transaction()` call, so handles can coexist
/// freely as long as transactions are not re-entrant.
pub struct SharedBus<D: DeviceSet> {
    inner: RefCell<SimBus<D>>,
}

impl<D: DeviceSet> SharedBus<D> {
    /// Creates a new `SharedBus` wrapping the given [`SimBus`].
    pub fn new(bus: SimBus<D>) -> Self {
        Self {
            inner: RefCell::new(bus),
        }
    }

    /// Returns a lightweight handle implementing [`I2c`].
    ///
    /// Multiple handles can coexist. Each handle borrows the underlying
    /// bus only for the duration of a single transaction.
    pub fn handle(&self) -> BusHandle<'_, D> {
        BusHandle { bus: &self.inner }
    }

    /// Returns a shared reference to the device set for test inspection.
    ///
    /// The returned [`core::cell::Ref`] holds the borrow for its
    /// lifetime. Access devices via tuple indexing (e.g.,
    /// `shared.devices().0`).
    pub fn devices(&self) -> core::cell::Ref<'_, D> {
        core::cell::Ref::map(self.inner.borrow(), |bus| bus.devices())
    }
}

/// A lightweight handle to a [`SharedBus`] that implements [`I2c`].
///
/// Created by [`SharedBus::handle()`]. Each transaction borrows the
/// underlying bus through a [`RefCell`], so multiple handles can
/// coexist as long as transactions do not overlap (which is guaranteed
/// in single-threaded code).
pub struct BusHandle<'a, D: DeviceSet> {
    bus: &'a RefCell<SimBus<D>>,
}

impl<D: DeviceSet> ErrorType for BusHandle<'_, D> {
    type Error = BusError;
}

impl<D: DeviceSet> I2c for BusHandle<'_, D> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.bus.borrow_mut().transaction(address, operations)
    }
}
