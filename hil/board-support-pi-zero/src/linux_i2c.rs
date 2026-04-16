//! Linux `/dev/i2c-*` bus wrapper.
//!
//! [`LinuxI2cBus`] wraps a [`linux_embedded_hal::I2cdev`] device node,
//! translating `embedded_hal::i2c::I2c` operations into Linux ioctl
//! calls on the underlying `/dev/i2c-N` file.

use embedded_hal::i2c::I2c;
use linux_embedded_hal::I2cdev;

/// Error from a Linux I2C bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I2cError;

/// A real I2C bus backed by a Linux `/dev/i2c-N` device node.
///
/// Constructed via [`open`](Self::open), which maps to the kernel's
/// I2C character device interface. Each transaction sets the slave
/// address via ioctl before performing the transfer.
pub struct LinuxI2cBus {
    dev: I2cdev,
    #[allow(dead_code)]
    path: String,
}

impl LinuxI2cBus {
    /// Opens the I2C device at the given path (e.g. `/dev/i2c-1`).
    ///
    /// # Errors
    ///
    /// Returns a description string if the device cannot be opened.
    pub fn open(path: &str) -> Result<Self, String> {
        let dev = I2cdev::new(path).map_err(|e| format!("failed to open {path}: {e}"))?;
        Ok(Self {
            dev,
            path: path.to_string(),
        })
    }

    /// Returns the device path this bus was opened from.
    #[allow(dead_code)]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Performs a write-then-read I2C transaction.
    ///
    /// Writes the single-byte register address `reg`, then reads
    /// `buf.len()` bytes from the device at `addr`.
    ///
    /// # Errors
    ///
    /// Returns [`I2cError`] if the I2C transaction fails.
    pub fn i2c_read(&mut self, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), I2cError> {
        self.dev.write_read(addr, &[reg], buf).map_err(|_| I2cError)
    }

    /// Performs an I2C write transaction.
    ///
    /// The `data` slice typically begins with the register address
    /// followed by value bytes.
    ///
    /// # Errors
    ///
    /// Returns [`I2cError`] if the I2C transaction fails.
    pub fn i2c_write(&mut self, addr: u8, data: &[u8]) -> Result<(), I2cError> {
        self.dev.write(addr, data).map_err(|_| I2cError)
    }
}
