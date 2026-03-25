//! Consuming builder for [`I2cSwitch`](super::I2cSwitch).
//!
//! The builder uses a type-changing pattern: each call to
//! [`channel`](I2cSwitchBuilder::channel) appends a new channel to the
//! type-level channel list.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::Address;
//! use i2c_hil_sim::devices::{I2cSwitchBuilder, Tmp1075};
//!
//! let mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
//!     .channel(Tmp1075::new(Address::new(0x48).unwrap()))
//!     .channel(Tmp1075::new(Address::new(0x49).unwrap()))
//!     .build();
//! ```

use crate::device::{Address, I2cDevice};
use crate::device_set::DeviceSet;

use super::{ChannelSet, I2cSwitch};

/// Builder for constructing an [`I2cSwitch`] with per-channel device sets.
///
/// Channels are added in order: the first `.channel()` call creates channel 0,
/// the second creates channel 1, and so on. The maximum supported channel
/// count is 8 (matching the TCA9548A).
///
/// # Panics
///
/// [`channel`](Self::channel) panics if a device with the same address already
/// exists on the channel being added. [`build`](Self::build) panics if more
/// than 8 channels have been added.
pub struct I2cSwitchBuilder<C: ChannelSet> {
    address: Address,
    channels: C,
}

impl I2cSwitchBuilder<()> {
    /// Creates a new builder with no channels.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            channels: (),
        }
    }
}

impl<C: ChannelSet> I2cSwitchBuilder<C> {
    /// Adds a single-device channel to the switch.
    ///
    /// This is the common case: one device per channel. The device is
    /// wrapped in a single-element [`DeviceSet`].
    ///
    /// # Panics
    ///
    /// Panics if the resulting switch would have more than 8 channels.
    pub fn channel<Dev: I2cDevice>(self, device: Dev) -> I2cSwitchBuilder<((Dev, ()), C)> {
        assert!(
            C::COUNT < 8,
            "I2cSwitch supports at most 8 channels, already have {}",
            C::COUNT,
        );
        I2cSwitchBuilder {
            address: self.address,
            channels: ((device, ()), self.channels),
        }
    }

    /// Adds a multi-device channel to the switch.
    ///
    /// Use this when a channel has multiple devices. The `devices` parameter
    /// should be a [`DeviceSet`] (e.g., built from nested tuples).
    ///
    /// # Panics
    ///
    /// Panics if the resulting switch would have more than 8 channels.
    pub fn channel_with_devices<D: DeviceSet>(self, devices: D) -> I2cSwitchBuilder<(D, C)> {
        assert!(
            C::COUNT < 8,
            "I2cSwitch supports at most 8 channels, already have {}",
            C::COUNT,
        );
        I2cSwitchBuilder {
            address: self.address,
            channels: (devices, self.channels),
        }
    }

    /// Adds an empty channel to the switch.
    ///
    /// Useful when a channel exists physically but has no devices attached.
    ///
    /// # Panics
    ///
    /// Panics if the resulting switch would have more than 8 channels.
    pub fn empty_channel(self) -> I2cSwitchBuilder<((), C)> {
        assert!(
            C::COUNT < 8,
            "I2cSwitch supports at most 8 channels, already have {}",
            C::COUNT,
        );
        I2cSwitchBuilder {
            address: self.address,
            channels: ((), self.channels),
        }
    }

    /// Builds the [`I2cSwitch`] with the configured channels.
    ///
    /// The control register is initialized to 0 (all channels disabled).
    pub fn build(self) -> I2cSwitch<C> {
        I2cSwitch {
            address: self.address,
            control: 0,
            channels: self.channels,
        }
    }
}
