//! TMP1075 digital temperature sensor simulation.
//!
//! Models a TI TMP1075 with four 16-bit registers accessed via a pointer
//! byte protocol:
//!
//! | Pointer | Register | Access | Default  |
//! |---------|----------|--------|----------|
//! | 0x00    | Temp     | R      | 0x0000   |
//! | 0x01    | Config   | R/W    | 0x00FF   |
//! | 0x02    | T_LOW    | R/W    | 0x4B00   |
//! | 0x03    | T_HIGH   | R/W    | 0x5000   |
//!
//! # Protocol
//!
//! - **Write 1 byte**: Sets the register pointer (0–3).
//! - **Write 3 bytes**: Sets pointer, then writes 2 data bytes (MSB, LSB)
//!   to the selected register.
//! - **Read**: Returns the 16-bit register at the current pointer as MSB,
//!   LSB, repeating for as many bytes as requested.
//!
//! # Temperature Encoding
//!
//! The temperature register stores a 12-bit two's-complement value in
//! bits \[15:4\]. One LSB = 0.0625 °C. Use [`celsius_to_raw`] to convert
//! from floating-point Celsius.

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// Number of registers in the TMP1075.
const REGISTER_COUNT: usize = 4;

/// Register index for the temperature result (read-only).
const REG_TEMP: u8 = 0x00;

/// Default value for the configuration register.
const DEFAULT_CONFIG: u16 = 0x00FF;

/// Default value for the T_LOW limit register (75 °C).
const DEFAULT_T_LOW: u16 = 0x4B00;

/// Default value for the T_HIGH limit register (80 °C).
const DEFAULT_T_HIGH: u16 = 0x5000;

/// Converts a temperature in degrees Celsius to the TMP1075 raw 16-bit
/// encoding.
///
/// The result is a 12-bit two's-complement value left-shifted by 4.
/// Values outside the sensor's range (−128 to +127.9375 °C) are clamped.
///
/// # Examples
///
/// ```
/// use i2c_hil_sim::devices::Tmp1075;
///
/// assert_eq!(Tmp1075::celsius_to_raw(25.0), 0x1900);
/// assert_eq!(Tmp1075::celsius_to_raw(0.0), 0x0000);
/// assert_eq!(Tmp1075::celsius_to_raw(-25.0), 0xE700);
/// ```
pub fn celsius_to_raw(celsius: f32) -> u16 {
    let clamped = celsius.clamp(-128.0_f32, 127.9375_f32);
    let counts = (clamped / 0.0625) as i16;
    ((counts << 4) as u16) & 0xFFF0
}

/// Simulated TI TMP1075 digital temperature sensor.
///
/// Holds four 16-bit registers and a pointer byte. The temperature
/// register (pointer 0) is read-only; writes to it return
/// [`BusError::DataNak`].
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_sim::devices::Tmp1075;
///
/// let sensor = Tmp1075::new(Address::new(0x48).unwrap());
/// ```
pub struct Tmp1075 {
    address: Address,
    registers: [u16; REGISTER_COUNT],
    pointer: u8,
}

impl Tmp1075 {
    /// Creates a new TMP1075 at the given address with default register
    /// values.
    ///
    /// Defaults: Temp=0x0000, Config=0x00FF, T_LOW=0x4B00, T_HIGH=0x5000.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            registers: [0x0000, DEFAULT_CONFIG, DEFAULT_T_LOW, DEFAULT_T_HIGH],
            pointer: 0,
        }
    }

    /// Creates a TMP1075 with a pre-loaded temperature value.
    ///
    /// The `raw` value is written directly to register 0. Use
    /// [`celsius_to_raw`] to convert from degrees Celsius.
    pub fn with_temperature(address: Address, raw: u16) -> Self {
        let mut dev = Self::new(address);
        dev.registers[REG_TEMP as usize] = raw;
        dev
    }

    /// Converts a temperature in degrees Celsius to the TMP1075 raw
    /// 16-bit encoding.
    ///
    /// This is a convenience alias for the free function [`celsius_to_raw`].
    pub fn celsius_to_raw(celsius: f32) -> u16 {
        celsius_to_raw(celsius)
    }

    /// Sets the temperature register to a raw 16-bit value.
    ///
    /// Use [`celsius_to_raw`] to convert from degrees Celsius.
    pub fn set_temperature_raw(&mut self, raw: u16) {
        self.registers[REG_TEMP as usize] = raw;
    }

    /// Returns a shared reference to the four 16-bit registers.
    pub fn registers(&self) -> &[u16; REGISTER_COUNT] {
        &self.registers
    }

    /// Returns the current pointer value.
    pub fn pointer(&self) -> u8 {
        self.pointer
    }
}

impl I2cDevice for Tmp1075 {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if data.is_empty() {
                        continue;
                    }
                    let ptr = data[0];
                    if ptr >= REGISTER_COUNT as u8 {
                        return Err(BusError::DataNak);
                    }
                    self.pointer = ptr;

                    if data.len() >= 3 {
                        // Writing 2 data bytes to the selected register.
                        if self.pointer == REG_TEMP {
                            return Err(BusError::DataNak);
                        }
                        let msb = data[1];
                        let lsb = data[2];
                        self.registers[self.pointer as usize] = u16::from_be_bytes([msb, lsb]);
                    }
                }
                Operation::Read(buf) => {
                    let reg_val = self.registers[self.pointer as usize];
                    let bytes = reg_val.to_be_bytes();
                    for (i, byte) in buf.iter_mut().enumerate() {
                        *byte = bytes[i % 2];
                    }
                }
            }
        }
        Ok(())
    }
}
