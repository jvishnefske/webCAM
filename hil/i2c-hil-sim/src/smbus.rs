//! SMBus 16-bit word register device abstraction.
//!
//! Many I2C devices use a common protocol: a pointer byte selects a 16-bit
//! register, reads return MSB then LSB, and writes send pointer + MSB + LSB.
//! The [`SmBusWordDevice`] trait captures this pattern and provides a blanket
//! [`I2cDevice`] implementation that handles I2C operation parsing.
//!
//! # Protocol
//!
//! - **Write 1 byte**: Sets the register pointer via [`set_pointer`](SmBusWordDevice::set_pointer).
//! - **Write 3+ bytes**: Sets pointer, then writes the 16-bit value (big-endian)
//!   via [`write_register`](SmBusWordDevice::write_register).
//! - **Read**: Returns the register at the current pointer as MSB, LSB,
//!   repeating for longer reads.
//! - **Empty operations**: Silently ignored (no-op).
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::smbus::SmBusWordDevice;
//! use i2c_hil_sim::{Address, BusError};
//!
//! struct MyDevice {
//!     address: Address,
//!     pointer: u8,
//!     registers: [u16; 4],
//! }
//!
//! impl SmBusWordDevice for MyDevice {
//!     fn address(&self) -> Address { self.address }
//!     fn pointer(&self) -> u8 { self.pointer }
//!     fn set_pointer(&mut self, ptr: u8) -> Result<(), BusError> {
//!         if ptr < 4 {
//!             self.pointer = ptr;
//!             Ok(())
//!         } else {
//!             Err(BusError::DataNak)
//!         }
//!     }
//!     fn read_register(&mut self, ptr: u8) -> u16 {
//!         self.registers[ptr as usize]
//!     }
//!     fn write_register(&mut self, ptr: u8, value: u16) -> Result<(), BusError> {
//!         self.registers[ptr as usize] = value;
//!         Ok(())
//!     }
//! }
//! ```

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// Trait for I2C devices that use 16-bit (word) registers accessed via a
/// pointer byte.
///
/// Implementors define register validation, read behavior, and write
/// behavior. The blanket [`I2cDevice`] implementation handles parsing
/// raw I2C operations into pointer-set, register-read, and register-write
/// calls.
///
/// This protocol is used by many sensor and power-monitoring ICs including
/// TMP1075, INA230, INA226, and similar devices.
pub trait SmBusWordDevice {
    /// Returns the 7-bit address this device responds to.
    fn address(&self) -> Address;

    /// Returns the current register pointer value.
    fn pointer(&self) -> u8;

    /// Validates and sets the register pointer.
    ///
    /// # Errors
    ///
    /// Returns [`BusError::DataNak`] if `ptr` is not a valid register
    /// address for this device.
    fn set_pointer(&mut self, ptr: u8) -> Result<(), BusError>;

    /// Reads the 16-bit register at the given pointer.
    ///
    /// Takes `&mut self` to allow side effects such as clearing status
    /// flags on read.
    fn read_register(&mut self, ptr: u8) -> u16;

    /// Writes a 16-bit value to the register at the given pointer.
    ///
    /// # Errors
    ///
    /// Returns [`BusError::DataNak`] if the register is read-only or
    /// the value is rejected.
    fn write_register(&mut self, ptr: u8, value: u16) -> Result<(), BusError>;
}

impl<T: SmBusWordDevice> I2cDevice for T {
    fn address(&self) -> Address {
        SmBusWordDevice::address(self)
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if data.is_empty() {
                        continue;
                    }
                    self.set_pointer(data[0])?;

                    if data.len() >= 3 {
                        let value = u16::from_be_bytes([data[1], data[2]]);
                        self.write_register(self.pointer(), value)?;
                    }
                }
                Operation::Read(buf) => {
                    let value = self.read_register(self.pointer());
                    let bytes = value.to_be_bytes();
                    for (i, byte) in buf.iter_mut().enumerate() {
                        *byte = bytes[i % 2];
                    }
                }
            }
        }
        Ok(())
    }
}
