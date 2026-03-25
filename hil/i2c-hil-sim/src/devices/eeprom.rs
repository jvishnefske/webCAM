//! Generic I2C-compatible EEPROM simulation.
//!
//! Models serial EEPROMs with a 16-bit byte-address protocol and page write
//! boundaries. The memory size is a compile-time constant parameter `N`,
//! which must be a power of two between 256 and 65,536 bytes.
//!
//! # Protocol
//!
//! - **Write**: First two bytes are the memory address (MSB, LSB). Subsequent
//!   bytes are written starting from that address with the lower bits of
//!   the address auto-incrementing within the page boundary. Writes
//!   that reach the end of a page wrap to the beginning of the same page.
//! - **Read**: Returns bytes starting from the current address pointer,
//!   auto-incrementing after each byte. The pointer wraps at the end of the
//!   memory space.
//! - **Write then Read** (random read): The write sets the address pointer,
//!   the read returns data from that address.
//!
//! # Address Validation
//!
//! The 16-bit address is masked to fit the memory size. Writes with fewer
//! than 2 bytes return [`BusError::DataNak`].
//!
//! # Page Size
//!
//! Page size is derived from memory size: `N / 512`, clamped to a minimum
//! of 8 bytes. This matches common EEPROM families (e.g. 256-byte devices
//! use 8-byte pages, 32 KB devices use 64-byte pages).

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// Computes the page size for a given memory size.
///
/// Uses `N / 512` with a minimum of 8, matching common EEPROM page sizes:
/// - 256 B → 8-byte pages
/// - 2 KB → 8-byte pages
/// - 8 KB → 16-byte pages
/// - 32 KB → 64-byte pages
/// - 64 KB → 128-byte pages
const fn page_size(n: usize) -> u16 {
    let computed = n / 512;
    if computed < 8 {
        8
    } else {
        computed as u16
    }
}

/// Simulated I2C EEPROM with compile-time memory size.
///
/// Holds `N` bytes of memory and a 16-bit address pointer. The default
/// erased state is `0xFF`, matching real EEPROM hardware.
///
/// `N` must be a power of two. This is enforced at compile time via a
/// const assertion in [`Eeprom::new`].
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_sim::devices::Eeprom256k;
///
/// let eeprom = Eeprom256k::new(Address::new(0x50).unwrap());
/// ```
pub struct Eeprom<const N: usize> {
    address: Address,
    memory: [u8; N],
    pointer: u16,
}

impl<const N: usize> Eeprom<N> {
    /// Address mask for the memory space.
    const ADDR_MASK: u16 = (N as u16).wrapping_sub(1);

    /// Page size in bytes for page write wrapping.
    const PAGE_SIZE: u16 = page_size(N);

    /// Mask for the page offset bits.
    const PAGE_OFFSET_MASK: u16 = Self::PAGE_SIZE - 1;

    /// Creates a new EEPROM at the given address with all bytes set to
    /// `0xFF` (erased state).
    ///
    /// The address pointer is initialized to 0.
    ///
    /// # Compile-time Requirements
    ///
    /// `N` must be a power of two. A compile-time assertion ensures this.
    pub fn new(address: Address) -> Self {
        const { assert!(N.is_power_of_two(), "EEPROM size N must be a power of two") };
        const { assert!(N >= 256, "EEPROM size N must be at least 256 bytes") };
        const { assert!(N <= 65_536, "EEPROM size N must be at most 65536 bytes") };
        Self {
            address,
            memory: [0xFF; N],
            pointer: 0,
        }
    }

    /// Creates a new EEPROM at the given address pre-loaded with the
    /// given memory contents.
    ///
    /// The address pointer is initialized to 0.
    pub fn with_data(address: Address, memory: [u8; N]) -> Self {
        const { assert!(N.is_power_of_two(), "EEPROM size N must be a power of two") };
        const { assert!(N >= 256, "EEPROM size N must be at least 256 bytes") };
        const { assert!(N <= 65_536, "EEPROM size N must be at most 65536 bytes") };
        Self {
            address,
            memory,
            pointer: 0,
        }
    }

    /// Returns a shared reference to the full memory array.
    ///
    /// Useful for verifying device state in tests.
    pub fn memory(&self) -> &[u8; N] {
        &self.memory
    }

    /// Returns the current 16-bit address pointer position.
    pub fn pointer(&self) -> u16 {
        self.pointer
    }
}

impl<const N: usize> I2cDevice for Eeprom<N> {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if data.len() < 2 {
                        return Err(BusError::DataNak);
                    }
                    self.pointer = u16::from_be_bytes([data[0], data[1]]) & Self::ADDR_MASK;

                    for &byte in &data[2..] {
                        self.memory[self.pointer as usize] = byte;
                        let page_base = self.pointer & !Self::PAGE_OFFSET_MASK;
                        let next_offset = (self.pointer & Self::PAGE_OFFSET_MASK) + 1;
                        self.pointer = page_base | (next_offset & Self::PAGE_OFFSET_MASK);
                    }
                }
                Operation::Read(buf) => {
                    for byte in buf.iter_mut() {
                        *byte = self.memory[self.pointer as usize];
                        self.pointer = (self.pointer + 1) & Self::ADDR_MASK;
                    }
                }
            }
        }
        Ok(())
    }
}

/// 256-Kbit (32,768-byte) EEPROM, e.g. AT24C256.
///
/// Uses 64-byte page writes and 15-bit addressing.
pub type Eeprom256k = Eeprom<32_768>;

/// 2-Kbit (256-byte) EEPROM, e.g. AT24C02.
///
/// Uses 8-byte page writes and 8-bit addressing.
pub type Eeprom2k = Eeprom<256>;
