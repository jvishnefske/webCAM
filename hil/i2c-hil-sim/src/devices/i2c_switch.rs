//! Generic I2C switch/mux simulation supporting any number of channels.
//!
//! [`I2cSwitch`] models I2C multiplexer/switch ICs such as the TI TCA9543A
//! (2-channel) or TCA9548A (8-channel). Unlike ordinary I2C devices, the
//! switch **implements [`embedded_hal::i2c::I2c`]** directly — it *is* the
//! bus that firmware talks to, routing downstream transactions to per-channel
//! device sets based on the control register.
//!
//! # Control Register
//!
//! The control register is a single byte where each bit enables the
//! corresponding channel. The register is automatically masked to
//! `(1 << channel_count) - 1`, so upper bits read as zero.
//!
//! When a transaction targets the switch's own address, it reads or writes
//! the control register. All other addresses are routed to the enabled
//! channel(s)' device sets.
//!
//! If multiple channels are enabled, channels are tried in order starting
//! from channel 0. If a channel returns [`NoDeviceAtAddress`](crate::BusError::NoDeviceAtAddress),
//! the next enabled channel is tried.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::Address;
//! use i2c_hil_sim::devices::{Tmp1075, I2cSwitchBuilder};
//!
//! let mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
//!     .channel(Tmp1075::new(Address::new(0x48).unwrap()))
//!     .channel(Tmp1075::new(Address::new(0x49).unwrap()))
//!     .build();
//! ```

pub mod builder;

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::device::Address;
use crate::device_set::DeviceSet;
use crate::error::BusError;

/// A type-level linked list of I2C switch channels.
///
/// Each channel is itself a [`DeviceSet`] containing the devices on that
/// downstream bus segment. The list is built as nested tuples `(Channel, Rest)`
/// terminated by `()`, where channels are ordered so that lower-numbered
/// channels appear deeper in the nesting (tail-first).
///
/// [`COUNT`](ChannelSet::COUNT) gives the total number of channels and is
/// used to compute the control register mask.
pub trait ChannelSet {
    /// The number of channels in this set.
    const COUNT: usize;

    /// Routes a transaction to the first enabled channel that has a device
    /// at `address`.
    ///
    /// `control` is the current control register value. `channel_index` is
    /// the index of the current head channel (counting from the tail).
    /// Channels are tried in ascending index order; if an enabled channel
    /// returns [`BusError::NoDeviceAtAddress`], the next enabled channel
    /// is tried.
    fn dispatch_enabled(
        &mut self,
        control: u8,
        channel_index: usize,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), BusError>;
}

impl ChannelSet for () {
    const COUNT: usize = 0;

    fn dispatch_enabled(
        &mut self,
        _control: u8,
        _channel_index: usize,
        address: u8,
        _operations: &mut [Operation<'_>],
    ) -> Result<(), BusError> {
        Err(BusError::NoDeviceAtAddress(address))
    }
}

impl<C: DeviceSet, R: ChannelSet> ChannelSet for (C, R) {
    const COUNT: usize = R::COUNT + 1;

    fn dispatch_enabled(
        &mut self,
        control: u8,
        channel_index: usize,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), BusError> {
        // Try the tail (lower-numbered channels) first
        let tail_result = self
            .1
            .dispatch_enabled(control, channel_index, address, operations);

        // If tail succeeded or tail has a non-NoDeviceAtAddress error, return it
        match tail_result {
            Ok(()) => return Ok(()),
            Err(BusError::NoDeviceAtAddress(_)) => {}
            Err(e) => return Err(e),
        }

        // Now try this channel (which is channel_index + R::COUNT, i.e. the
        // head is the highest-numbered channel)
        let my_index = channel_index + R::COUNT;
        let enabled = control & (1 << my_index) != 0;

        if enabled {
            let result = self.0.dispatch(address, operations);
            if result.is_ok() {
                return result;
            }
            // If NoDeviceAtAddress, fall through (return NoDeviceAtAddress)
            return result;
        }

        Err(BusError::NoDeviceAtAddress(address))
    }
}

/// Simulated I2C switch/mux with a variable number of channels.
///
/// Each channel holds a type-level device set of downstream devices.
/// Construction uses the consuming [`I2cSwitchBuilder`](builder::I2cSwitchBuilder).
///
/// The control register is a single byte, masked to `(1 << C::COUNT) - 1`.
/// For a 2-channel switch (TCA9543A), only bits 0–1 are valid. For an
/// 8-channel switch (TCA9548A), all 8 bits are valid.
pub struct I2cSwitch<C: ChannelSet> {
    address: Address,
    control: u8,
    channels: C,
}

impl<C: ChannelSet> I2cSwitch<C> {
    /// Returns the current control register value.
    pub fn control(&self) -> u8 {
        self.control
    }

    /// Returns the switch's own I2C address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Returns a shared reference to the channel set.
    pub fn channels(&self) -> &C {
        &self.channels
    }

    /// Computes the control register mask for this switch's channel count.
    fn control_mask() -> u8 {
        if C::COUNT >= 8 {
            0xFF
        } else {
            (1u8 << C::COUNT) - 1
        }
    }

    /// Returns a mutable reference to the channel set.
    pub(crate) fn channels_mut(&mut self) -> &mut C {
        &mut self.channels
    }

    /// Processes a control register transaction (read/write to own address).
    pub(crate) fn process_control(
        &mut self,
        operations: &mut [Operation<'_>],
    ) -> Result<(), BusError> {
        let mask = Self::control_mask();
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if let Some(&byte) = data.first() {
                        self.control = byte & mask;
                    }
                }
                Operation::Read(buf) => {
                    for byte in buf.iter_mut() {
                        *byte = self.control;
                    }
                }
            }
        }
        Ok(())
    }

    /// Routes a transaction to the enabled channel(s).
    fn route_to_channel(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), BusError> {
        if self.control == 0 {
            return Err(BusError::NoDeviceAtAddress(address));
        }

        self.channels
            .dispatch_enabled(self.control, 0, address, operations)
    }
}

impl<C: ChannelSet> ErrorType for I2cSwitch<C> {
    type Error = BusError;
}

impl<C: ChannelSet> I2c for I2cSwitch<C> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        if address == self.address.raw() {
            self.process_control(operations)
        } else {
            self.route_to_channel(address, operations)
        }
    }
}
