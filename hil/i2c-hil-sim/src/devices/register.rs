//! A register-mapped I2C slave device.
//!
//! [`RegisterDevice`] models an I2C device with a flat register space
//! addressed by a single-byte register pointer. This is the most common
//! I2C device pattern: the master writes a register address, then reads
//! or writes data bytes starting from that address.
//!
//! # Protocol
//!
//! - **Write**: First byte is the register address, subsequent bytes are
//!   written to consecutive registers starting from that address.
//! - **Read**: Returns bytes starting from the current register pointer,
//!   auto-incrementing after each byte.
//! - **Write then Read** (write_read): The write sets the register pointer,
//!   the read returns data starting from that pointer.

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// A simulated I2C device with a fixed-size register map.
///
/// The register space is `N` bytes, addressed by a single-byte pointer.
/// The device maintains an internal pointer that auto-increments on each
/// byte read or written, wrapping at `N`.
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_sim::devices::RegisterDevice;
///
/// let device = RegisterDevice::new(
///     Address::new(0x48).unwrap(),
///     [0u8; 256],
/// );
/// ```
pub struct RegisterDevice<const N: usize> {
    address: Address,
    registers: [u8; N],
    pointer: u8,
}

impl<const N: usize> RegisterDevice<N> {
    /// Creates a new register device at the given address with initial
    /// register contents.
    ///
    /// The register pointer is initialized to 0.
    pub fn new(address: Address, registers: [u8; N]) -> Self {
        Self {
            address,
            registers,
            pointer: 0,
        }
    }

    /// Returns a shared reference to the register map.
    ///
    /// Useful for verifying device state in tests.
    pub fn registers(&self) -> &[u8; N] {
        &self.registers
    }

    /// Returns the current register pointer position.
    pub fn pointer(&self) -> u8 {
        self.pointer
    }

    /// Wraps an index into the valid register range.
    fn wrap_index(&self, index: u8) -> usize {
        (index as usize) % N
    }
}

impl<const N: usize> I2cDevice for RegisterDevice<N> {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if let Some((&reg_addr, payload)) = data.split_first() {
                        self.pointer = reg_addr;
                        for &byte in payload {
                            let idx = self.wrap_index(self.pointer);
                            self.registers[idx] = byte;
                            self.pointer = self.pointer.wrapping_add(1);
                        }
                    }
                }
                Operation::Read(buf) => {
                    for byte in buf.iter_mut() {
                        let idx = self.wrap_index(self.pointer);
                        *byte = self.registers[idx];
                        self.pointer = self.pointer.wrapping_add(1);
                    }
                }
            }
        }
        Ok(())
    }
}
