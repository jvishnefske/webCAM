//! Error types for the simulated I2C bus.
//!
//! [`BusError`] implements [`embedded_hal::i2c::Error`] so it can be used
//! as the associated `Error` type for the [`SimBus`](crate::SimBus) I2C
//! implementation.

use embedded_hal::i2c::{self, ErrorKind, NoAcknowledgeSource};

/// Errors that can occur during a simulated I2C transaction.
///
/// Each variant maps to a specific [`ErrorKind`] through the
/// [`embedded_hal::i2c::Error`] trait implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusError {
    /// No device responded at the given 7-bit address.
    ///
    /// Maps to [`ErrorKind::NoAcknowledge`] with
    /// [`NoAcknowledgeSource::Address`].
    NoDeviceAtAddress(u8),

    /// A device rejected a data byte during a write operation.
    ///
    /// Maps to [`ErrorKind::NoAcknowledge`] with
    /// [`NoAcknowledgeSource::Data`].
    DataNak,

    /// A device-specific error occurred during operation processing.
    ///
    /// Maps to [`ErrorKind::Other`].
    DeviceError,
}

impl i2c::Error for BusError {
    fn kind(&self) -> ErrorKind {
        match self {
            BusError::NoDeviceAtAddress(_) => {
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            }
            BusError::DataNak => ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data),
            BusError::DeviceError => ErrorKind::Other,
        }
    }
}

impl core::fmt::Display for BusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BusError::NoDeviceAtAddress(addr) => {
                write!(f, "no device at address 0x{addr:02x}")
            }
            BusError::DataNak => write!(f, "data not acknowledged"),
            BusError::DeviceError => write!(f, "device processing error"),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::format;
    use embedded_hal::i2c::Error;

    #[test]
    fn display_no_device() {
        let err = BusError::NoDeviceAtAddress(0x48);
        assert_eq!(format!("{err}"), "no device at address 0x48");
    }

    #[test]
    fn display_data_nak() {
        assert_eq!(format!("{}", BusError::DataNak), "data not acknowledged");
    }

    #[test]
    fn display_device_error() {
        assert_eq!(format!("{}", BusError::DeviceError), "device processing error");
    }

    #[test]
    fn error_kind_mapping() {
        assert!(matches!(
            BusError::NoDeviceAtAddress(0).kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert!(matches!(
            BusError::DataNak.kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
        ));
        assert!(matches!(
            BusError::DeviceError.kind(),
            ErrorKind::Other
        ));
    }
}
