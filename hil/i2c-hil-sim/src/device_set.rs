//! Type-level linked list for heterogeneous device storage.
//!
//! [`DeviceSet`] is a trait implemented by nested tuples `(Device, Rest)`
//! terminated by `()`. This provides compile-time device composition
//! with zero heap allocation and no dynamic dispatch overhead for
//! address routing.
//!
//! In addition to regular [`I2cDevice`] nodes, an [`I2cSwitch`] can be
//! placed on a bus via the `(I2cSwitch<C>, R)` impl. The switch's own
//! address handles the control register; all other addresses are routed
//! through the switch's enabled channels before falling through to the
//! rest of the bus.

use embedded_hal::i2c::Operation;

use crate::device::I2cDevice;
use crate::devices::i2c_switch::{ChannelSet, I2cSwitch};
use crate::error::BusError;

/// A collection of [`I2cDevice`] implementors that supports address-based
/// dispatch.
///
/// Implemented for `()` (empty set), `(D, R)` where `D: I2cDevice`
/// and `R: DeviceSet` (recursive cons cell), and `(I2cSwitch<C>, R)`
/// for placing switches on a bus alongside regular devices.
pub trait DeviceSet {
    /// Routes a transaction to the device at `address`.
    ///
    /// Walks the type-level list and delegates to the first device whose
    /// address matches. Returns [`BusError::NoDeviceAtAddress`] if no
    /// device in the set responds to the given address.
    fn dispatch(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), BusError>;

    /// Returns `true` if any device in the set has the given address.
    ///
    /// Used by builders to detect duplicate address registration.
    fn contains_address(&self, address: u8) -> bool;
}

impl DeviceSet for () {
    fn dispatch(&mut self, address: u8, _operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        Err(BusError::NoDeviceAtAddress(address))
    }

    fn contains_address(&self, _address: u8) -> bool {
        false
    }
}

impl<D: I2cDevice, R: DeviceSet> DeviceSet for (D, R) {
    fn dispatch(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        if self.0.address().raw() == address {
            self.0.process(operations)
        } else {
            self.1.dispatch(address, operations)
        }
    }

    fn contains_address(&self, address: u8) -> bool {
        self.0.address().raw() == address || self.1.contains_address(address)
    }
}

impl<C: ChannelSet, R: DeviceSet> DeviceSet for (I2cSwitch<C>, R) {
    fn dispatch(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        if address == self.0.address().raw() {
            // Control register access on the switch itself
            self.0.process_control(operations)
        } else {
            // Try routing through enabled channels first
            let control = self.0.control();
            if control != 0 {
                let result = self
                    .0
                    .channels_mut()
                    .dispatch_enabled(control, 0, address, operations);
                match result {
                    Ok(()) => return Ok(()),
                    Err(BusError::NoDeviceAtAddress(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            // Fall through to rest of bus
            self.1.dispatch(address, operations)
        }
    }

    fn contains_address(&self, address: u8) -> bool {
        // Only reports the switch's own address; downstream devices are
        // isolated behind the mux and not directly on this bus segment.
        self.0.address().raw() == address || self.1.contains_address(address)
    }
}
