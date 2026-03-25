//! EMC2305 5-fan PWM controller simulation.
//!
//! Models a Microchip EMC2305 fan controller with SMBus byte protocol,
//! five independent fan channels with PWM duty registers, and tachometer
//! readings computed on read via a linear transfer function.
//!
//! # Linear Transfer Function
//!
//! Each fan channel maps PWM duty to RPM and then to a tach count:
//!
//! ```text
//! rpm = (pwm_duty * max_rpm) / 255
//! tach_count = 7_864_320 / rpm       (when rpm > 0)
//! tach_raw   = tach_count << 3       (lower 3 bits unused)
//! ```
//!
//! When `pwm_duty` is zero the fan is stalled and tach reads `0xFFE0`.
//!
//! # Protocol
//!
//! The EMC2305 uses SMBus byte/word protocol: a write sets the command
//! (register pointer) byte, followed by one data byte for byte registers
//! or two bytes for the tach target word register.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_devices::Emc2305;
//! use embedded_hal::i2c::I2c;
//!
//! let dev = Emc2305::new(Address::new(0x2E).unwrap());
//! let mut bus = SimBusBuilder::new().with_device(dev).build();
//!
//! // Read Product ID
//! let mut buf = [0u8; 1];
//! bus.write_read(0x2E, &[0xFD], &mut buf).unwrap();
//! assert_eq!(buf[0], 0x34);
//!
//! // Set fan 0 PWM to 128
//! bus.write(0x2E, &[0x30, 128]).unwrap();
//!
//! // Read tach high byte for fan 0
//! let mut tach = [0u8; 1];
//! bus.write_read(0x2E, &[0x3E], &mut tach).unwrap();
//! ```

use embedded_hal::i2c::Operation;
use i2c_hil_sim::{Address, BusError, I2cDevice};

// --- Global register addresses ---

const REG_CONFIGURATION: u8 = 0x00;
const REG_FAN_STATUS: u8 = 0x24;
const REG_DRIVE_FAIL_STATUS: u8 = 0x27;
const REG_PWM_POLARITY: u8 = 0x2A;
const REG_PWM_OUTPUT_CONFIG: u8 = 0x2B;
const REG_PRODUCT_ID: u8 = 0xFD;
const REG_MANUFACTURER_ID: u8 = 0xFE;
const REG_REVISION: u8 = 0xFF;

// --- Per-fan register offsets (from base) ---

const FAN_OFFSET_SETTING: u8 = 0x00;
const FAN_OFFSET_CONFIG1: u8 = 0x01;
const FAN_OFFSET_CONFIG2: u8 = 0x02;
const FAN_OFFSET_MIN_DRIVE: u8 = 0x05;
const FAN_OFFSET_TACH_TARGET_LOW: u8 = 0x0C;
const FAN_OFFSET_TACH_TARGET_HIGH: u8 = 0x0D;
const FAN_OFFSET_TACH_READING_HIGH: u8 = 0x0E;
const FAN_OFFSET_TACH_READING_LOW: u8 = 0x0F;

// --- Fan register bases ---

const FAN_BASE: [u8; 5] = [0x30, 0x40, 0x50, 0x60, 0x70];

// --- POR defaults ---

const POR_CONFIGURATION: u8 = 0x40;
const POR_FAN_CONFIG1: u8 = 0x2B;
const POR_FAN_MIN_DRIVE: u8 = 0x66;
const POR_TACH_TARGET: u16 = 0xFFFF;

// --- Identification constants ---

const PRODUCT_ID: u8 = 0x34;
const MANUFACTURER_ID: u8 = 0x5D;
const REVISION_ID: u8 = 0x01;

// --- Stall tach value ---

const TACH_STALLED_HIGH: u8 = 0xFF;
const TACH_STALLED_LOW: u8 = 0xE0;

// --- Default max RPM ---

const DEFAULT_MAX_RPM: u32 = 10_000;

// --- Tach numerator constant (3_932_160 * 2) ---

const TACH_NUMERATOR: u32 = 7_864_320;

/// Per-fan register state.
#[derive(Debug, Clone)]
struct FanState {
    /// PWM duty cycle (0–255).
    setting: u8,
    /// Fan configuration register 1.
    config1: u8,
    /// Fan configuration register 2.
    config2: u8,
    /// Minimum PWM drive floor.
    min_drive: u8,
    /// Tach target (16-bit, little-endian in register pair).
    tach_target: u16,
}

impl FanState {
    /// Creates a fan state with power-on reset defaults.
    fn new() -> Self {
        Self {
            setting: 0x00,
            config1: POR_FAN_CONFIG1,
            config2: 0x00,
            min_drive: POR_FAN_MIN_DRIVE,
            tach_target: POR_TACH_TARGET,
        }
    }
}

/// Simulated Microchip EMC2305 5-fan PWM controller.
///
/// Provides five independent fan channels, each with a PWM setting register
/// and computed tachometer readings. The tach value is derived on read from
/// the PWM duty via a configurable linear transfer function.
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_devices::Emc2305;
///
/// // Default: all fans at 10000 max RPM
/// let dev = Emc2305::new(Address::new(0x2E).unwrap());
///
/// // Custom max RPM per fan
/// let dev = Emc2305::with_max_rpm(
///     Address::new(0x2E).unwrap(),
///     [8000, 8000, 10000, 12000, 12000],
/// );
/// ```
pub struct Emc2305 {
    address: Address,
    command: u8,
    config: u8,
    fan_status: u8,
    drive_fail_status: u8,
    pwm_polarity: u8,
    pwm_output_config: u8,
    fans: [FanState; 5],
    max_rpm: [u32; 5],
}

impl Emc2305 {
    /// Creates a new EMC2305 at the given address with all fans at 10 000 max RPM.
    ///
    /// All registers are initialized to power-on reset defaults per the
    /// datasheet.
    pub fn new(address: Address) -> Self {
        Self::with_max_rpm(address, [DEFAULT_MAX_RPM; 5])
    }

    /// Creates a new EMC2305 with per-fan maximum RPM for the linear transfer
    /// function.
    ///
    /// The `max_rpm` array maps fan index 0–4 to the RPM produced at PWM
    /// duty 255. The tach reading is computed as:
    ///
    /// ```text
    /// rpm = (pwm_duty * max_rpm[fan]) / 255
    /// ```
    pub fn with_max_rpm(address: Address, max_rpm: [u32; 5]) -> Self {
        Self {
            address,
            command: 0,
            config: POR_CONFIGURATION,
            fan_status: 0x00,
            drive_fail_status: 0x00,
            pwm_polarity: 0x00,
            pwm_output_config: 0x00,
            fans: [
                FanState::new(),
                FanState::new(),
                FanState::new(),
                FanState::new(),
                FanState::new(),
            ],
            max_rpm,
        }
    }

    /// Returns the current PWM duty setting for the given fan (0–4).
    ///
    /// # Panics
    ///
    /// Panics if `fan >= 5`.
    pub fn fan_setting(&self, fan: usize) -> u8 {
        self.fans[fan].setting
    }

    /// Computes the RPM for the given fan from the linear transfer function.
    ///
    /// Returns `(pwm_duty * max_rpm) / 255`, or 0 when PWM is zero.
    ///
    /// # Panics
    ///
    /// Panics if `fan >= 5`.
    pub fn fan_rpm(&self, fan: usize) -> u32 {
        let duty = self.fans[fan].setting as u32;
        if duty == 0 {
            return 0;
        }
        (duty * self.max_rpm[fan]) / 255
    }

    /// Injects or clears a stall condition on the given fan.
    ///
    /// When stalled, the corresponding bit in the fan status register is set.
    /// When cleared, the bit is removed.
    ///
    /// # Panics
    ///
    /// Panics if `fan >= 5`.
    pub fn set_fan_stall(&mut self, fan: usize, stalled: bool) {
        let bit = 1 << fan;
        if stalled {
            self.fan_status |= bit;
        } else {
            self.fan_status &= !bit;
        }
    }

    /// Computes the tach reading bytes (high, low) for the given fan.
    fn compute_tach(&self, fan: usize) -> (u8, u8) {
        let rpm = self.fan_rpm(fan);
        if rpm == 0 {
            return (TACH_STALLED_HIGH, TACH_STALLED_LOW);
        }
        let tach_count = TACH_NUMERATOR / rpm;
        let tach_raw = tach_count << 3;
        let high = (tach_raw >> 8) as u8;
        let low = (tach_raw & 0xFF) as u8;
        (high, low)
    }

    /// Resolves a register address to a fan index and offset, if it falls
    /// within a per-fan register block.
    fn resolve_fan_register(reg: u8) -> Option<(usize, u8)> {
        for (i, &base) in FAN_BASE.iter().enumerate() {
            if reg >= base && reg < base + 0x10 {
                return Some((i, reg - base));
            }
        }
        None
    }

    /// Reads the register at the current command pointer.
    fn read_register(&self, reg: u8) -> Option<u8> {
        // Global registers
        match reg {
            REG_CONFIGURATION => return Some(self.config),
            REG_FAN_STATUS => return Some(self.fan_status),
            REG_DRIVE_FAIL_STATUS => return Some(self.drive_fail_status),
            REG_PWM_POLARITY => return Some(self.pwm_polarity),
            REG_PWM_OUTPUT_CONFIG => return Some(self.pwm_output_config),
            REG_PRODUCT_ID => return Some(PRODUCT_ID),
            REG_MANUFACTURER_ID => return Some(MANUFACTURER_ID),
            REG_REVISION => return Some(REVISION_ID),
            _ => {}
        }

        // Per-fan registers
        if let Some((fan, offset)) = Self::resolve_fan_register(reg) {
            let state = &self.fans[fan];
            return match offset {
                FAN_OFFSET_SETTING => Some(state.setting),
                FAN_OFFSET_CONFIG1 => Some(state.config1),
                FAN_OFFSET_CONFIG2 => Some(state.config2),
                FAN_OFFSET_MIN_DRIVE => Some(state.min_drive),
                FAN_OFFSET_TACH_TARGET_LOW => Some(state.tach_target as u8),
                FAN_OFFSET_TACH_TARGET_HIGH => Some((state.tach_target >> 8) as u8),
                FAN_OFFSET_TACH_READING_HIGH => {
                    let (high, _) = self.compute_tach(fan);
                    Some(high)
                }
                FAN_OFFSET_TACH_READING_LOW => {
                    let (_, low) = self.compute_tach(fan);
                    Some(low)
                }
                _ => None,
            };
        }

        None
    }

    /// Returns true if the given register address is read-only.
    fn is_read_only(reg: u8) -> bool {
        match reg {
            REG_FAN_STATUS
            | REG_DRIVE_FAIL_STATUS
            | REG_PRODUCT_ID
            | REG_MANUFACTURER_ID
            | REG_REVISION => return true,
            _ => {}
        }

        if let Some((_, offset)) = Self::resolve_fan_register(reg) {
            return matches!(
                offset,
                FAN_OFFSET_TACH_READING_HIGH | FAN_OFFSET_TACH_READING_LOW
            );
        }

        false
    }

    /// Writes a byte value to the register at the given address.
    fn write_register(&mut self, reg: u8, val: u8) -> Result<(), BusError> {
        if Self::is_read_only(reg) {
            return Err(BusError::DataNak);
        }

        // Global registers
        match reg {
            REG_CONFIGURATION => {
                self.config = val;
                return Ok(());
            }
            REG_PWM_POLARITY => {
                self.pwm_polarity = val;
                return Ok(());
            }
            REG_PWM_OUTPUT_CONFIG => {
                self.pwm_output_config = val;
                return Ok(());
            }
            _ => {}
        }

        // Per-fan registers
        if let Some((fan, offset)) = Self::resolve_fan_register(reg) {
            let state = &mut self.fans[fan];
            return match offset {
                FAN_OFFSET_SETTING => {
                    state.setting = val;
                    Ok(())
                }
                FAN_OFFSET_CONFIG1 => {
                    state.config1 = val;
                    Ok(())
                }
                FAN_OFFSET_CONFIG2 => {
                    state.config2 = val;
                    Ok(())
                }
                FAN_OFFSET_MIN_DRIVE => {
                    state.min_drive = val;
                    Ok(())
                }
                FAN_OFFSET_TACH_TARGET_LOW => {
                    state.tach_target = (state.tach_target & 0xFF00) | val as u16;
                    Ok(())
                }
                FAN_OFFSET_TACH_TARGET_HIGH => {
                    state.tach_target = (state.tach_target & 0x00FF) | ((val as u16) << 8);
                    Ok(())
                }
                _ => Err(BusError::DataNak),
            };
        }

        Err(BusError::DataNak)
    }

    /// Fills a read buffer from the current command pointer register.
    fn fill_read_buffer(&self, buf: &mut [u8]) -> Result<(), BusError> {
        let val = self.read_register(self.command).ok_or(BusError::DataNak)?;
        for b in buf.iter_mut() {
            *b = val;
        }
        Ok(())
    }

    /// Processes a write operation from the host.
    fn handle_write(&mut self, data: &[u8]) -> Result<(), BusError> {
        if data.is_empty() {
            return Ok(());
        }

        self.command = data[0];

        if data.len() == 1 {
            // Just setting the command pointer
            return Ok(());
        }

        // Write data byte(s) starting at command register
        for (i, &val) in data[1..].iter().enumerate() {
            let reg = self.command.wrapping_add(i as u8);
            self.write_register(reg, val)?;
        }

        Ok(())
    }
}

impl I2cDevice for Emc2305 {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    self.handle_write(data)?;
                }
                Operation::Read(buf) => {
                    self.fill_read_buffer(buf)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::i2c::I2c;
    use i2c_hil_sim::SimBusBuilder;

    const ADDR: u8 = 0x2E;

    fn make_bus() -> i2c_hil_sim::SimBus<(Emc2305, ())> {
        let dev = Emc2305::new(Address::new(ADDR).unwrap());
        SimBusBuilder::new().with_device(dev).build()
    }

    fn make_bus_custom_rpm(max_rpm: [u32; 5]) -> i2c_hil_sim::SimBus<(Emc2305, ())> {
        let dev = Emc2305::with_max_rpm(Address::new(ADDR).unwrap(), max_rpm);
        SimBusBuilder::new().with_device(dev).build()
    }

    /// Helper: write a command byte and read one byte back.
    fn read_byte(bus: &mut impl I2c, reg: u8) -> u8 {
        let mut buf = [0u8; 1];
        bus.write_read(ADDR, &[reg], &mut buf).unwrap();
        buf[0]
    }

    // --- FR-DEV-25: Product ID, Manufacturer ID, Revision ---

    #[test]
    fn read_product_id() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_PRODUCT_ID), 0x34);
    }

    #[test]
    fn read_manufacturer_id() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_MANUFACTURER_ID), 0x5D);
    }

    #[test]
    fn read_revision() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_REVISION), 0x01);
    }

    // --- FR-DEV-26: POR defaults ---

    #[test]
    fn por_configuration() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_CONFIGURATION), 0x40);
    }

    #[test]
    fn por_fan_setting() {
        let mut bus = make_bus();
        for &base in &FAN_BASE {
            assert_eq!(
                read_byte(&mut bus, base + FAN_OFFSET_SETTING),
                0x00,
                "fan base 0x{base:02X}"
            );
        }
    }

    #[test]
    fn por_fan_config1() {
        let mut bus = make_bus();
        for &base in &FAN_BASE {
            assert_eq!(
                read_byte(&mut bus, base + FAN_OFFSET_CONFIG1),
                0x2B,
                "fan base 0x{base:02X}"
            );
        }
    }

    #[test]
    fn por_fan_min_drive() {
        let mut bus = make_bus();
        for &base in &FAN_BASE {
            assert_eq!(
                read_byte(&mut bus, base + FAN_OFFSET_MIN_DRIVE),
                0x66,
                "fan base 0x{base:02X}"
            );
        }
    }

    #[test]
    fn por_tach_target() {
        let mut bus = make_bus();
        for &base in &FAN_BASE {
            let low = read_byte(&mut bus, base + FAN_OFFSET_TACH_TARGET_LOW);
            let high = read_byte(&mut bus, base + FAN_OFFSET_TACH_TARGET_HIGH);
            assert_eq!(low, 0xFF, "tach target low, fan base 0x{base:02X}");
            assert_eq!(high, 0xFF, "tach target high, fan base 0x{base:02X}");
        }
    }

    #[test]
    fn por_fan_status() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_FAN_STATUS), 0x00);
    }

    #[test]
    fn por_pwm_polarity() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_PWM_POLARITY), 0x00);
    }

    #[test]
    fn por_pwm_output_config() {
        let mut bus = make_bus();
        assert_eq!(read_byte(&mut bus, REG_PWM_OUTPUT_CONFIG), 0x00);
    }

    // --- FR-DEV-22: 5 independent fan channels ---

    #[test]
    fn pwm_duty_write_read_roundtrip() {
        let mut bus = make_bus();
        for (i, &base) in FAN_BASE.iter().enumerate() {
            let duty = 50 * (i as u8 + 1);
            bus.write(ADDR, &[base + FAN_OFFSET_SETTING, duty]).unwrap();
            let read_back = read_byte(&mut bus, base + FAN_OFFSET_SETTING);
            assert_eq!(read_back, duty, "fan {i}");
        }
    }

    // --- FR-DEV-23: TACH reading computed via linear TF ---

    #[test]
    fn tach_from_pwm_duty() {
        let mut bus = make_bus();
        let duty: u8 = 128;
        bus.write(ADDR, &[FAN_BASE[0] + FAN_OFFSET_SETTING, duty])
            .unwrap();

        // Expected: rpm = (128 * 10000) / 255 = 5019
        // tach_count = 7_864_320 / 5019 = 1567
        // tach_raw = 1567 << 3 = 12536 = 0x30F8
        let expected_rpm = (128u32 * 10_000) / 255;
        let expected_count = TACH_NUMERATOR / expected_rpm;
        let expected_raw = expected_count << 3;
        let expected_high = (expected_raw >> 8) as u8;
        let expected_low = (expected_raw & 0xFF) as u8;

        let high = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_HIGH);
        let low = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_LOW);

        assert_eq!(high, expected_high);
        assert_eq!(low, expected_low);
    }

    #[test]
    fn tach_stalled_at_pwm_zero() {
        let mut bus = make_bus();
        // PWM defaults to 0, so tach should be stalled
        let high = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_HIGH);
        let low = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_LOW);
        assert_eq!(high, 0xFF);
        assert_eq!(low, 0xE0);
    }

    #[test]
    fn tach_at_max_rpm() {
        let mut bus = make_bus();
        bus.write(ADDR, &[FAN_BASE[0] + FAN_OFFSET_SETTING, 255])
            .unwrap();

        // rpm = (255 * 10000) / 255 = 10000
        // tach_count = 7_864_320 / 10000 = 786
        // tach_raw = 786 << 3 = 6288 = 0x1890
        let expected_count = TACH_NUMERATOR / 10_000;
        let expected_raw = expected_count << 3;
        let expected_high = (expected_raw >> 8) as u8;
        let expected_low = (expected_raw & 0xFF) as u8;

        let high = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_HIGH);
        let low = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_LOW);

        assert_eq!(high, expected_high);
        assert_eq!(low, expected_low);
    }

    // --- FR-DEV-22 + FR-DEV-23: Per-fan independence ---

    #[test]
    fn per_fan_independence() {
        let mut bus = make_bus();
        let duties = [50u8, 100, 150, 200, 255];

        // Set different duties
        for (i, &duty) in duties.iter().enumerate() {
            bus.write(ADDR, &[FAN_BASE[i] + FAN_OFFSET_SETTING, duty])
                .unwrap();
        }

        // Verify each fan has independent tach
        for (i, &duty) in duties.iter().enumerate() {
            let rpm = (duty as u32 * 10_000) / 255;
            let expected_count = TACH_NUMERATOR / rpm;
            let expected_raw = expected_count << 3;
            let expected_high = (expected_raw >> 8) as u8;

            let high = read_byte(&mut bus, FAN_BASE[i] + FAN_OFFSET_TACH_READING_HIGH);
            assert_eq!(high, expected_high, "fan {i} duty {duty}");
        }
    }

    // --- FR-DEV-21: Config register R/W round-trip ---

    #[test]
    fn config_register_roundtrip() {
        let mut bus = make_bus();
        bus.write(ADDR, &[REG_CONFIGURATION, 0xE5]).unwrap();
        assert_eq!(read_byte(&mut bus, REG_CONFIGURATION), 0xE5);
    }

    #[test]
    fn pwm_polarity_roundtrip() {
        let mut bus = make_bus();
        bus.write(ADDR, &[REG_PWM_POLARITY, 0x1F]).unwrap();
        assert_eq!(read_byte(&mut bus, REG_PWM_POLARITY), 0x1F);
    }

    #[test]
    fn pwm_output_config_roundtrip() {
        let mut bus = make_bus();
        bus.write(ADDR, &[REG_PWM_OUTPUT_CONFIG, 0x1F]).unwrap();
        assert_eq!(read_byte(&mut bus, REG_PWM_OUTPUT_CONFIG), 0x1F);
    }

    #[test]
    fn fan_config1_roundtrip() {
        let mut bus = make_bus();
        bus.write(ADDR, &[FAN_BASE[2] + FAN_OFFSET_CONFIG1, 0x55])
            .unwrap();
        assert_eq!(read_byte(&mut bus, FAN_BASE[2] + FAN_OFFSET_CONFIG1), 0x55);
    }

    #[test]
    fn fan_min_drive_roundtrip() {
        let mut bus = make_bus();
        bus.write(ADDR, &[FAN_BASE[3] + FAN_OFFSET_MIN_DRIVE, 0x33])
            .unwrap();
        assert_eq!(
            read_byte(&mut bus, FAN_BASE[3] + FAN_OFFSET_MIN_DRIVE),
            0x33
        );
    }

    #[test]
    fn tach_target_roundtrip() {
        let mut bus = make_bus();
        // Write low byte
        bus.write(ADDR, &[FAN_BASE[1] + FAN_OFFSET_TACH_TARGET_LOW, 0xAB])
            .unwrap();
        // Write high byte
        bus.write(ADDR, &[FAN_BASE[1] + FAN_OFFSET_TACH_TARGET_HIGH, 0xCD])
            .unwrap();

        let low = read_byte(&mut bus, FAN_BASE[1] + FAN_OFFSET_TACH_TARGET_LOW);
        let high = read_byte(&mut bus, FAN_BASE[1] + FAN_OFFSET_TACH_TARGET_HIGH);
        assert_eq!(low, 0xAB);
        assert_eq!(high, 0xCD);
    }

    // --- FR-DEV-27: Read-only registers reject writes ---

    #[test]
    fn write_product_id_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[REG_PRODUCT_ID, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn write_manufacturer_id_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[REG_MANUFACTURER_ID, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn write_revision_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[REG_REVISION, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn write_fan_status_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[REG_FAN_STATUS, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn write_drive_fail_status_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[REG_DRIVE_FAIL_STATUS, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn write_tach_reading_rejected() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[FAN_BASE[0] + FAN_OFFSET_TACH_READING_HIGH, 0x00]);
        assert!(result.is_err());

        let result = bus.write(ADDR, &[FAN_BASE[0] + FAN_OFFSET_TACH_READING_LOW, 0x00]);
        assert!(result.is_err());
    }

    // --- FR-DEV-24: Custom max RPM ---

    #[test]
    fn custom_max_rpm() {
        let mut bus = make_bus_custom_rpm([5000, 8000, 10000, 12000, 15000]);

        // Set fan 0 to full duty with 5000 max RPM
        bus.write(ADDR, &[FAN_BASE[0] + FAN_OFFSET_SETTING, 255])
            .unwrap();

        // rpm = (255 * 5000) / 255 = 5000
        // tach_count = 7_864_320 / 5000 = 1572
        // tach_raw = 1572 << 3 = 12576 = 0x3120
        let expected_count = TACH_NUMERATOR / 5000;
        let expected_raw = expected_count << 3;
        let expected_high = (expected_raw >> 8) as u8;
        let expected_low = (expected_raw & 0xFF) as u8;

        let high = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_HIGH);
        let low = read_byte(&mut bus, FAN_BASE[0] + FAN_OFFSET_TACH_READING_LOW);
        assert_eq!(high, expected_high);
        assert_eq!(low, expected_low);

        // Set fan 4 to full duty with 15000 max RPM
        bus.write(ADDR, &[FAN_BASE[4] + FAN_OFFSET_SETTING, 255])
            .unwrap();

        // rpm = 15000, tach_count = 7_864_320 / 15000 = 524
        // tach_raw = 524 << 3 = 4192 = 0x1060
        let expected_count_4 = TACH_NUMERATOR / 15000;
        let expected_raw_4 = expected_count_4 << 3;
        let expected_high_4 = (expected_raw_4 >> 8) as u8;
        let expected_low_4 = (expected_raw_4 & 0xFF) as u8;

        let high = read_byte(&mut bus, FAN_BASE[4] + FAN_OFFSET_TACH_READING_HIGH);
        let low = read_byte(&mut bus, FAN_BASE[4] + FAN_OFFSET_TACH_READING_LOW);
        assert_eq!(high, expected_high_4);
        assert_eq!(low, expected_low_4);
    }

    // --- FR-DEV-28: No unsafe code (compile-time enforced by #![forbid(unsafe_code)]) ---

    // --- Additional: fan_rpm and set_fan_stall public API ---

    #[test]
    fn fan_rpm_api() {
        let mut dev = Emc2305::new(Address::new(ADDR).unwrap());
        assert_eq!(dev.fan_rpm(0), 0);
        dev.fans[0].setting = 128;
        assert_eq!(dev.fan_rpm(0), (128 * 10_000) / 255);
    }

    #[test]
    fn set_fan_stall_api() {
        let mut dev = Emc2305::new(Address::new(ADDR).unwrap());
        assert_eq!(dev.fan_status, 0x00);

        dev.set_fan_stall(0, true);
        assert_eq!(dev.fan_status, 0x01);

        dev.set_fan_stall(3, true);
        assert_eq!(dev.fan_status, 0x09);

        dev.set_fan_stall(0, false);
        assert_eq!(dev.fan_status, 0x08);
    }

    #[test]
    fn stall_status_readable_on_bus() {
        let mut dev = Emc2305::new(Address::new(ADDR).unwrap());
        dev.set_fan_stall(2, true);
        let mut bus = SimBusBuilder::new().with_device(dev).build();
        assert_eq!(read_byte(&mut bus, REG_FAN_STATUS), 0x04);
    }

    // --- Unknown register returns error ---

    #[test]
    fn read_unknown_register_naks() {
        let mut bus = make_bus();
        let mut buf = [0u8; 1];
        let result = bus.write_read(ADDR, &[0x10], &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn write_unknown_register_naks() {
        let mut bus = make_bus();
        let result = bus.write(ADDR, &[0x10, 0x42]);
        assert!(result.is_err());
    }
}
