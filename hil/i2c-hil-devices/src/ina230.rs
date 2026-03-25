//! INA230 high-/low-side current/power monitor simulation.
//!
//! Models a TI INA230 with nine 16-bit registers accessed via the SMBus
//! word protocol (pointer byte + MSB + LSB):
//!
//! | Pointer | Register      | Access | Default  | Sim Behavior                       |
//! |---------|---------------|--------|----------|------------------------------------|
//! | 0x00    | Configuration | R/W    | 0x4127   | RST bit (D15) resets all to POR    |
//! | 0x01    | Shunt Voltage | R      | 0x0000   | Set via `set_shunt_voltage_raw()`  |
//! | 0x02    | Bus Voltage   | R      | 0x0000   | Set via `set_bus_voltage_raw()`    |
//! | 0x03    | Power         | R      | 0x0000   | Computed from current and bus voltage |
//! | 0x04    | Current       | R      | 0x0000   | Computed from shunt and calibration |
//! | 0x05    | Calibration   | R/W    | 0x0000   | D15 reserved, D14:D0 stored        |
//! | 0x06    | Mask/Enable   | R/W    | 0x0000   | Reading clears CVRF (D3)           |
//! | 0x07    | Alert Limit   | R/W    | 0x0000   | Full 16-bit threshold              |
//! | 0xFF    | Die ID        | R      | 0x2260   | Read-only identification           |
//!
//! # Current and Power Computation
//!
//! Current and power registers are not stored; they are computed on read:
//!
//! - **Current** = `(shunt_voltage * calibration) / 2048`
//! - **Power** = `(|current| * bus_voltage) / 20_000`
//!
//! Both return 0 when the calibration register is 0.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_devices::Ina230;
//! use embedded_hal::i2c::I2c;
//!
//! let mut bus = SimBusBuilder::new()
//!     .with_device(Ina230::new(Address::new(0x40).unwrap()))
//!     .build();
//!
//! // Read default configuration register
//! let mut buf = [0u8; 2];
//! bus.write_read(0x40, &[0x00], &mut buf).unwrap();
//! assert_eq!(buf, [0x41, 0x27]);
//! ```

use i2c_hil_sim::smbus::SmBusWordDevice;
use i2c_hil_sim::{Address, BusError};

/// Default configuration register value (POR).
const DEFAULT_CONFIG: u16 = 0x4127;

/// Default die ID for INA230.
const DEFAULT_DIE_ID: u16 = 0x2260;

/// RST bit position in the configuration register.
const RST_BIT: u16 = 1 << 15;

/// CVRF (Conversion Ready Flag) bit position in the mask/enable register.
const CVRF_BIT: u16 = 1 << 3;

/// Mask for the calibration register (D15 reserved, always 0).
const CALIBRATION_MASK: u16 = 0x7FFF;

/// Mask for bus voltage (15-bit unsigned, D15 always 0).
const BUS_VOLTAGE_MASK: u16 = 0x7FFF;

/// Simulated TI INA230 high-/low-side current/power monitor.
///
/// Stores writable registers and injected measurement values. Current
/// and power are computed on read from shunt voltage, bus voltage, and
/// the calibration register.
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_devices::Ina230;
///
/// let monitor = Ina230::new(Address::new(0x40).unwrap());
/// ```
pub struct Ina230 {
    address: Address,
    pointer: u8,
    config: u16,
    shunt_voltage: i16,
    bus_voltage: u16,
    calibration: u16,
    mask_enable: u16,
    alert_limit: u16,
    die_id: u16,
}

impl Ina230 {
    /// Creates a new INA230 at the given address with power-on reset defaults.
    ///
    /// All measurement registers start at zero. Configuration defaults
    /// to `0x4127` per the datasheet.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            pointer: 0,
            config: DEFAULT_CONFIG,
            shunt_voltage: 0,
            bus_voltage: 0,
            calibration: 0,
            mask_enable: 0,
            alert_limit: 0,
            die_id: DEFAULT_DIE_ID,
        }
    }

    /// Creates a new INA230 with a custom die ID value.
    pub fn with_die_id(address: Address, die_id: u16) -> Self {
        let mut dev = Self::new(address);
        dev.die_id = die_id;
        dev
    }

    /// Injects a raw shunt voltage measurement (signed 16-bit).
    ///
    /// Each LSB = 2.5 uV. For example, a value of 8000 represents
    /// 20 mV across the shunt resistor.
    pub fn set_shunt_voltage_raw(&mut self, raw: i16) {
        self.shunt_voltage = raw;
    }

    /// Injects a raw bus voltage measurement (unsigned, 15-bit).
    ///
    /// The value is masked to 15 bits (D15 always 0). Each LSB = 1.25 mV.
    pub fn set_bus_voltage_raw(&mut self, raw: u16) {
        self.bus_voltage = raw & BUS_VOLTAGE_MASK;
    }

    /// Returns the raw shunt voltage value.
    pub fn shunt_voltage_raw(&self) -> i16 {
        self.shunt_voltage
    }

    /// Returns the raw bus voltage value.
    pub fn bus_voltage_raw(&self) -> u16 {
        self.bus_voltage
    }

    /// Returns the calibration register value.
    pub fn calibration(&self) -> u16 {
        self.calibration
    }

    /// Returns the configuration register value.
    pub fn config(&self) -> u16 {
        self.config
    }

    /// Computes the current register value from shunt voltage and calibration.
    ///
    /// Formula: `(shunt_voltage * calibration) / 2048`, truncated to i16.
    /// Returns 0 when calibration is 0.
    fn current(&self) -> i16 {
        if self.calibration == 0 {
            return 0;
        }
        let result = (self.shunt_voltage as i32 * self.calibration as i32) / 2048;
        result as i16
    }

    /// Computes the power register value from current and bus voltage.
    ///
    /// Formula: `(|current| * bus_voltage) / 20_000`, truncated to u16.
    /// Returns 0 when calibration is 0.
    fn power(&self) -> u16 {
        let current = self.current();
        if current == 0 && self.bus_voltage == 0 {
            return 0;
        }
        let abs_current = (current as i32).unsigned_abs();
        let result = (abs_current * self.bus_voltage as u32) / 20_000;
        result as u16
    }

    /// Resets all registers to power-on defaults.
    fn reset(&mut self) {
        let address = self.address;
        let die_id = self.die_id;
        *self = Self::new(address);
        self.die_id = die_id;
    }
}

impl SmBusWordDevice for Ina230 {
    fn address(&self) -> Address {
        self.address
    }

    fn pointer(&self) -> u8 {
        self.pointer
    }

    fn set_pointer(&mut self, ptr: u8) -> Result<(), BusError> {
        match ptr {
            0x00..=0x07 | 0xFF => {
                self.pointer = ptr;
                Ok(())
            }
            _ => Err(BusError::DataNak),
        }
    }

    fn read_register(&mut self, ptr: u8) -> u16 {
        match ptr {
            0x00 => self.config,
            0x01 => self.shunt_voltage as u16,
            0x02 => self.bus_voltage,
            0x03 => self.power(),
            0x04 => self.current() as u16,
            0x05 => self.calibration,
            0x06 => {
                let value = self.mask_enable;
                self.mask_enable &= !CVRF_BIT;
                value
            }
            0x07 => self.alert_limit,
            0xFF => self.die_id,
            _ => 0,
        }
    }

    fn write_register(&mut self, ptr: u8, value: u16) -> Result<(), BusError> {
        match ptr {
            0x00 => {
                if value & RST_BIT != 0 {
                    self.reset();
                } else {
                    self.config = value;
                }
                Ok(())
            }
            0x01..=0x04 | 0xFF => Err(BusError::DataNak),
            0x05 => {
                self.calibration = value & CALIBRATION_MASK;
                Ok(())
            }
            0x06 => {
                self.mask_enable = value;
                Ok(())
            }
            0x07 => {
                self.alert_limit = value;
                Ok(())
            }
            _ => Err(BusError::DataNak),
        }
    }
}
