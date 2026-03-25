//! LTC4287 High Power Positive Hot Swap Controller simulation.
//!
//! Models an Analog Devices LTC4287 with PMBus/SMBus 3.1 protocol using
//! **little-endian** word ordering (low byte first), mixed command sizes
//! (byte, word, block read, send byte), and an extended command prefix
//! (0xFE) for manufacturer registers.
//!
//! # Protocol Differences from INA230
//!
//! | Aspect | INA230 (SmBusWordDevice) | LTC4287 (I2cDevice) |
//! |--------|--------------------------|---------------------|
//! | Word order | Big-endian (MSB first) | Little-endian (LSB first) |
//! | Command sizes | All 16-bit words | Mixed: byte, word, block, send-byte |
//! | Extended commands | None | 0xFE prefix for MFR registers |
//! | Status registers | Simple R/W | Write-1-to-Clear (W1C) |
//!
//! # Modelled Registers
//!
//! - Identification: MFR_ID, MFR_MODEL, MFR_REVISION, IC_DEVICE_ID, IC_DEVICE_REV (block reads)
//! - Telemetry: READ_VIN, READ_VOUT, READ_IOUT, READ_TEMPERATURE_1, READ_PIN (injectable)
//! - Configuration: PAGE, OPERATION, WRITE_PROTECT, fault limits and responses
//! - Status: STATUS_BYTE/WORD (computed), sub-status registers (W1C)
//! - CLEAR_FAULTS send-byte command
//! - USER_SCRATCH scratchpad registers
//! - MFR extended registers via 0xFE prefix
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_devices::Ltc4287;
//! use embedded_hal::i2c::I2c;
//!
//! let mut dev = Ltc4287::new(Address::new(0x44).unwrap());
//! dev.set_read_vin(0x1234);
//!
//! let mut bus = SimBusBuilder::new().with_device(dev).build();
//!
//! // Read VIN (little-endian word)
//! let mut buf = [0u8; 2];
//! bus.write_read(0x44, &[0x88], &mut buf).unwrap();
//! assert_eq!(u16::from_le_bytes(buf), 0x1234);
//! ```

use embedded_hal::i2c::Operation;
use i2c_hil_sim::{Address, BusError, I2cDevice};

// --- PMBus command codes ---

const CMD_PAGE: u8 = 0x00;
const CMD_OPERATION: u8 = 0x01;
const CMD_CLEAR_FAULTS: u8 = 0x03;
const CMD_WRITE_PROTECT: u8 = 0x10;
const CMD_CAPABILITY: u8 = 0x19;
const CMD_VOUT_OV_WARN_LIMIT: u8 = 0x42;
const CMD_VOUT_UV_WARN_LIMIT: u8 = 0x43;
const CMD_IOUT_OC_FAULT_RESPONSE: u8 = 0x47;
const CMD_IOUT_OC_WARN_LIMIT: u8 = 0x4A;
const CMD_OT_FAULT_LIMIT: u8 = 0x4F;
const CMD_OT_FAULT_RESPONSE: u8 = 0x50;
const CMD_OT_WARN_LIMIT: u8 = 0x51;
const CMD_UT_WARN_LIMIT: u8 = 0x52;
const CMD_VIN_OV_FAULT_RESPONSE: u8 = 0x56;
const CMD_VIN_OV_WARN_LIMIT: u8 = 0x57;
const CMD_VIN_UV_WARN_LIMIT: u8 = 0x58;
const CMD_VIN_UV_FAULT_RESPONSE: u8 = 0x5A;
const CMD_PIN_OP_WARN_LIMIT: u8 = 0x6B;
const CMD_STATUS_BYTE: u8 = 0x78;
const CMD_STATUS_WORD: u8 = 0x79;
const CMD_STATUS_VOUT: u8 = 0x7A;
const CMD_STATUS_IOUT: u8 = 0x7B;
const CMD_STATUS_INPUT: u8 = 0x7C;
const CMD_STATUS_TEMPERATURE: u8 = 0x7D;
const CMD_STATUS_CML: u8 = 0x7E;
const CMD_STATUS_OTHER: u8 = 0x7F;
const CMD_STATUS_MFR_SPECIFIC: u8 = 0x80;
const CMD_READ_VIN: u8 = 0x88;
const CMD_READ_VOUT: u8 = 0x8B;
const CMD_READ_IOUT: u8 = 0x8C;
const CMD_READ_TEMPERATURE_1: u8 = 0x8D;
const CMD_READ_PIN: u8 = 0x97;
const CMD_PMBUS_REVISION: u8 = 0x98;
const CMD_MFR_ID: u8 = 0x99;
const CMD_MFR_MODEL: u8 = 0x9A;
const CMD_MFR_REVISION: u8 = 0x9B;
const CMD_IC_DEVICE_ID: u8 = 0xAD;
const CMD_IC_DEVICE_REV: u8 = 0xAE;
const CMD_USER_SCRATCH_1: u8 = 0xB3;
const CMD_USER_SCRATCH_2: u8 = 0xB4;
const CMD_USER_SCRATCH_3: u8 = 0xB6;
const CMD_USER_SCRATCH_4: u8 = 0xB7;
const CMD_MFR_FLT_CONFIG: u8 = 0xD2;
const CMD_MFR_OP_FAULT_RESPONSE: u8 = 0xD7;
const CMD_MFR_ADC_CONFIG: u8 = 0xD8;
const CMD_MFR_AVG_SEL: u8 = 0xD9;
const CMD_MFR_LOFF: u8 = 0xDC;
const CMD_MFR_SYSTEM_STATUS1: u8 = 0xE0;
const CMD_MFR_SYSTEM_STATUS2: u8 = 0xE1;
const CMD_MFR_PMB_STAT: u8 = 0xE2;
const CMD_MFR_SPECIAL_ID: u8 = 0xE7;
const CMD_MFR_COMMON: u8 = 0xEF;
const CMD_MFR_SD_CAUSE: u8 = 0xF1;
const CMD_MFR_CONFIG1: u8 = 0xF2;
const CMD_MFR_CONFIG2: u8 = 0xF3;
const CMD_MFR_ON_OFF_CONFIG: u8 = 0xFC;
const CMD_MFR_REBOOT_CONTROL: u8 = 0xFD;
const CMD_EXTENDED_PREFIX: u8 = 0xFE;

// --- Constant register values ---

const CAPABILITY_VALUE: u8 = 0xD0;
const PMBUS_REVISION_VALUE: u8 = 0x33;
const MFR_SPECIAL_ID_VALUE: u16 = 0x7020;
const MFR_COMMON_VALUE: u8 = 0xEE;

// --- Block read data ---

const MFR_ID_DATA: &[u8] = b"LTC";
const MFR_MODEL_DATA: &[u8] = b"LTC4287";
const MFR_REVISION_DATA: &[u8] = &[0x11];
const IC_DEVICE_ID_DATA: &[u8] = b"LTC4287";
const IC_DEVICE_REV_DATA: &[u8] = &[0x11];

// --- POR defaults ---

const POR_IOUT_OC_FAULT_RESPONSE: u8 = 0xC0;
const POR_OT_FAULT_RESPONSE: u8 = 0x80;
const POR_VIN_OV_FAULT_RESPONSE: u8 = 0xB8;
const POR_VIN_UV_FAULT_RESPONSE: u8 = 0xB8;
const POR_VOUT_OV_WARN_LIMIT: u16 = 0x7FFF;
const POR_VOUT_UV_WARN_LIMIT: u16 = 0x0000;
const POR_IOUT_OC_WARN_LIMIT: u16 = 0x7FFF;
const POR_OT_FAULT_LIMIT: u16 = 0x7FFF;
const POR_OT_WARN_LIMIT: u16 = 0x7FFF;
const POR_UT_WARN_LIMIT: u16 = 0x0000;
const POR_VIN_OV_WARN_LIMIT: u16 = 0x7FFF;
const POR_VIN_UV_WARN_LIMIT: u16 = 0x0000;
const POR_PIN_OP_WARN_LIMIT: u16 = 0x7FFF;
const POR_MFR_ADC_CONFIG: u8 = 0x01;
const POR_MFR_AVG_SEL: u8 = 0x85;
const POR_MFR_OP_FAULT_RESPONSE: u16 = 0xFFE0;
const POR_MFR_CONFIG1: u16 = 0x5572;
const POR_MFR_CONFIG2: u16 = 0x00EF;
const POR_MFR_ON_OFF_CONFIG: u16 = 0x001D;

// --- Write protect bits ---

const WP1_BIT: u8 = 0x80;
const WP2_BIT: u8 = 0x40;

/// Register category for dispatch.
enum RegisterKind {
    /// Single-byte register value.
    Byte(u8),
    /// Two-byte little-endian word value.
    Word(u16),
    /// Block read: byte count prefix followed by data bytes.
    Block(&'static [u8]),
}

/// Simulated Analog Devices LTC4287 High Power Positive Hot Swap Controller.
///
/// Stores writable configuration, status, and injectable telemetry registers.
/// Status registers use write-1-to-clear (W1C) semantics. STATUS_BYTE and
/// STATUS_WORD are computed on read from sub-status registers.
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_devices::Ltc4287;
///
/// let device = Ltc4287::new(Address::new(0x44).unwrap());
/// ```
pub struct Ltc4287 {
    address: Address,
    command: u8,
    extended: bool,

    // Core config (byte registers)
    page: u8,
    operation: u8,
    write_protect: u8,

    // Fault response (byte registers)
    iout_oc_fault_response: u8,
    ot_fault_response: u8,
    vin_ov_fault_response: u8,
    vin_uv_fault_response: u8,

    // Warning/fault limits (word registers, LE)
    vout_ov_warn_limit: u16,
    vout_uv_warn_limit: u16,
    iout_oc_warn_limit: u16,
    ot_fault_limit: u16,
    ot_warn_limit: u16,
    ut_warn_limit: u16,
    vin_ov_warn_limit: u16,
    vin_uv_warn_limit: u16,
    pin_op_warn_limit: u16,

    // Status registers (byte, W1C)
    status_vout: u8,
    status_iout: u8,
    status_input: u8,
    status_temperature: u8,
    status_cml: u8,
    status_other: u8,
    status_mfr_specific: u8,

    // Telemetry (injectable, read-only word)
    read_vin: u16,
    read_vout: u16,
    read_iout: u16,
    read_temperature_1: u16,
    read_pin: u16,

    // Scratch registers (R/W word)
    user_scratch: [u16; 4],

    // MFR config
    mfr_flt_config: u8,
    mfr_adc_config: u8,
    mfr_avg_sel: u8,
    mfr_op_fault_response: u16,
    mfr_config1: u16,
    mfr_config2: u16,
    mfr_on_off_config: u16,
    mfr_reboot_control: u8,

    // MFR status (W1C)
    mfr_system_status1: u16,
    mfr_system_status2: u16,
    mfr_pmb_stat: u8,
    mfr_loff: u8,
    mfr_sd_cause: u8,
}

impl Ltc4287 {
    /// Creates a new LTC4287 at the given address with power-on reset defaults.
    ///
    /// All telemetry and status registers start at zero. Configuration registers
    /// are initialized to POR values per the datasheet (Table 15).
    pub fn new(address: Address) -> Self {
        Self {
            address,
            command: 0,
            extended: false,

            page: 0x00,
            operation: 0x00,
            write_protect: 0x00,

            iout_oc_fault_response: POR_IOUT_OC_FAULT_RESPONSE,
            ot_fault_response: POR_OT_FAULT_RESPONSE,
            vin_ov_fault_response: POR_VIN_OV_FAULT_RESPONSE,
            vin_uv_fault_response: POR_VIN_UV_FAULT_RESPONSE,

            vout_ov_warn_limit: POR_VOUT_OV_WARN_LIMIT,
            vout_uv_warn_limit: POR_VOUT_UV_WARN_LIMIT,
            iout_oc_warn_limit: POR_IOUT_OC_WARN_LIMIT,
            ot_fault_limit: POR_OT_FAULT_LIMIT,
            ot_warn_limit: POR_OT_WARN_LIMIT,
            ut_warn_limit: POR_UT_WARN_LIMIT,
            vin_ov_warn_limit: POR_VIN_OV_WARN_LIMIT,
            vin_uv_warn_limit: POR_VIN_UV_WARN_LIMIT,
            pin_op_warn_limit: POR_PIN_OP_WARN_LIMIT,

            status_vout: 0,
            status_iout: 0,
            status_input: 0,
            status_temperature: 0,
            status_cml: 0,
            status_other: 0,
            status_mfr_specific: 0,

            read_vin: 0,
            read_vout: 0,
            read_iout: 0,
            read_temperature_1: 0,
            read_pin: 0,

            user_scratch: [0; 4],

            mfr_flt_config: 0x00,
            mfr_adc_config: POR_MFR_ADC_CONFIG,
            mfr_avg_sel: POR_MFR_AVG_SEL,
            mfr_op_fault_response: POR_MFR_OP_FAULT_RESPONSE,
            mfr_config1: POR_MFR_CONFIG1,
            mfr_config2: POR_MFR_CONFIG2,
            mfr_on_off_config: POR_MFR_ON_OFF_CONFIG,
            mfr_reboot_control: 0x00,

            mfr_system_status1: 0,
            mfr_system_status2: 0,
            mfr_pmb_stat: 0,
            mfr_loff: 0,
            mfr_sd_cause: 0,
        }
    }

    /// Injects a raw VIN telemetry value (PMBus LINEAR16 format).
    pub fn set_read_vin(&mut self, raw: u16) {
        self.read_vin = raw;
    }

    /// Injects a raw VOUT telemetry value (PMBus LINEAR16 format).
    pub fn set_read_vout(&mut self, raw: u16) {
        self.read_vout = raw;
    }

    /// Injects a raw IOUT telemetry value (PMBus LINEAR11 format).
    pub fn set_read_iout(&mut self, raw: u16) {
        self.read_iout = raw;
    }

    /// Injects a raw temperature telemetry value (PMBus LINEAR11 format).
    pub fn set_read_temperature_1(&mut self, raw: u16) {
        self.read_temperature_1 = raw;
    }

    /// Injects a raw input power telemetry value (PMBus LINEAR11 format).
    pub fn set_read_pin(&mut self, raw: u16) {
        self.read_pin = raw;
    }

    /// Injects STATUS_VOUT bits for testing W1C and computed status.
    pub fn set_status_vout(&mut self, val: u8) {
        self.status_vout = val;
    }

    /// Injects STATUS_IOUT bits for testing W1C and computed status.
    pub fn set_status_iout(&mut self, val: u8) {
        self.status_iout = val;
    }

    /// Injects STATUS_INPUT bits for testing W1C and computed status.
    pub fn set_status_input(&mut self, val: u8) {
        self.status_input = val;
    }

    /// Injects STATUS_TEMPERATURE bits for testing W1C and computed status.
    pub fn set_status_temperature(&mut self, val: u8) {
        self.status_temperature = val;
    }

    /// Injects STATUS_CML bits for testing W1C and computed status.
    pub fn set_status_cml(&mut self, val: u8) {
        self.status_cml = val;
    }

    /// Injects STATUS_OTHER bits for testing W1C and computed status.
    pub fn set_status_other(&mut self, val: u8) {
        self.status_other = val;
    }

    /// Injects STATUS_MFR_SPECIFIC bits for testing W1C and computed status.
    pub fn set_status_mfr_specific(&mut self, val: u8) {
        self.status_mfr_specific = val;
    }

    /// Returns the raw VIN telemetry value.
    pub fn read_vin(&self) -> u16 {
        self.read_vin
    }

    /// Returns the raw VOUT telemetry value.
    pub fn read_vout(&self) -> u16 {
        self.read_vout
    }

    /// Returns the raw IOUT telemetry value.
    pub fn read_iout(&self) -> u16 {
        self.read_iout
    }

    /// Returns the raw temperature telemetry value.
    pub fn read_temperature_1(&self) -> u16 {
        self.read_temperature_1
    }

    /// Returns the raw input power telemetry value.
    pub fn read_pin(&self) -> u16 {
        self.read_pin
    }

    /// Returns the OPERATION register value.
    pub fn operation(&self) -> u8 {
        self.operation
    }

    /// Returns the WRITE_PROTECT register value.
    pub fn write_protect(&self) -> u8 {
        self.write_protect
    }

    /// Computes STATUS_BYTE from sub-status registers.
    ///
    /// - Bit 6: OFF (OPERATION bit 7 inverted)
    /// - Bit 4: IOUT_OC_FAULT (status_iout bit 7)
    /// - Bit 3: VIN_UV_FAULT (status_input bit 4)
    /// - Bit 2: TEMPERATURE (status_temperature != 0)
    /// - Bit 1: CML (status_cml != 0)
    /// - Bit 0: NONE_OF_THE_ABOVE (other status bits set)
    fn compute_status_byte(&self) -> u8 {
        let mut sb: u8 = 0;

        // Bit 6: OFF — operation bit 7 is ON, so OFF = !ON
        if self.operation & 0x80 == 0 {
            sb |= 1 << 6;
        }

        // Bit 4: IOUT_OC_FAULT
        if self.status_iout & 0x80 != 0 {
            sb |= 1 << 4;
        }

        // Bit 3: VIN_UV_FAULT (status_input bit 4)
        if self.status_input & 0x10 != 0 {
            sb |= 1 << 3;
        }

        // Bit 2: TEMPERATURE
        if self.status_temperature != 0 {
            sb |= 1 << 2;
        }

        // Bit 1: CML
        if self.status_cml != 0 {
            sb |= 1 << 1;
        }

        // Bit 0: NONE_OF_THE_ABOVE
        if self.status_other != 0 || self.status_mfr_specific != 0 {
            sb |= 1;
        }

        sb
    }

    /// Computes STATUS_WORD from sub-status registers.
    ///
    /// Low byte is STATUS_BYTE. High byte aggregates additional sub-status:
    /// - Bit 15: VOUT (status_vout != 0)
    /// - Bit 14: IOUT/POUT (status_iout != 0)
    /// - Bit 13: INPUT (status_input != 0)
    /// - Bit 12: MFR_SPECIFIC (status_mfr_specific != 0)
    /// - Bit 11: POWER_GOOD# (status_vout bit 4 or bit 3)
    /// - Bit 10: reserved (0)
    /// - Bit 9: OTHER (status_other != 0)
    /// - Bit 8: UNKNOWN (0)
    fn compute_status_word(&self) -> u16 {
        let low = self.compute_status_byte() as u16;
        let mut high: u16 = 0;

        if self.status_vout != 0 {
            high |= 1 << 7; // bit 15 of word = bit 7 of high byte
        }
        if self.status_iout != 0 {
            high |= 1 << 6; // bit 14
        }
        if self.status_input != 0 {
            high |= 1 << 5; // bit 13
        }
        if self.status_mfr_specific != 0 {
            high |= 1 << 4; // bit 12
        }
        // Bit 11: POWER_GOOD# — vout bit 4 (UV) or bit 3 (low)
        if self.status_vout & 0x18 != 0 {
            high |= 1 << 3;
        }
        if self.status_other != 0 {
            high |= 1 << 1; // bit 9
        }

        high << 8 | low
    }

    /// Reads a register and returns its kind (byte, word, or block).
    ///
    /// Returns `None` for unknown/invalid commands.
    fn read_register(&self, cmd: u8, extended: bool) -> Option<RegisterKind> {
        if extended {
            return self.read_extended_register(cmd);
        }
        match cmd {
            // Byte registers
            CMD_PAGE => Some(RegisterKind::Byte(self.page)),
            CMD_OPERATION => Some(RegisterKind::Byte(self.operation)),
            CMD_WRITE_PROTECT => Some(RegisterKind::Byte(self.write_protect)),
            CMD_CAPABILITY => Some(RegisterKind::Byte(CAPABILITY_VALUE)),
            CMD_IOUT_OC_FAULT_RESPONSE => Some(RegisterKind::Byte(self.iout_oc_fault_response)),
            CMD_OT_FAULT_RESPONSE => Some(RegisterKind::Byte(self.ot_fault_response)),
            CMD_VIN_OV_FAULT_RESPONSE => Some(RegisterKind::Byte(self.vin_ov_fault_response)),
            CMD_VIN_UV_FAULT_RESPONSE => Some(RegisterKind::Byte(self.vin_uv_fault_response)),
            CMD_PMBUS_REVISION => Some(RegisterKind::Byte(PMBUS_REVISION_VALUE)),
            CMD_MFR_COMMON => Some(RegisterKind::Byte(MFR_COMMON_VALUE)),

            // Computed status
            CMD_STATUS_BYTE => Some(RegisterKind::Byte(self.compute_status_byte())),
            CMD_STATUS_WORD => Some(RegisterKind::Word(self.compute_status_word())),

            // W1C status (byte)
            CMD_STATUS_VOUT => Some(RegisterKind::Byte(self.status_vout)),
            CMD_STATUS_IOUT => Some(RegisterKind::Byte(self.status_iout)),
            CMD_STATUS_INPUT => Some(RegisterKind::Byte(self.status_input)),
            CMD_STATUS_TEMPERATURE => Some(RegisterKind::Byte(self.status_temperature)),
            CMD_STATUS_CML => Some(RegisterKind::Byte(self.status_cml)),
            CMD_STATUS_OTHER => Some(RegisterKind::Byte(self.status_other)),
            CMD_STATUS_MFR_SPECIFIC => Some(RegisterKind::Byte(self.status_mfr_specific)),

            // Word limits
            CMD_VOUT_OV_WARN_LIMIT => Some(RegisterKind::Word(self.vout_ov_warn_limit)),
            CMD_VOUT_UV_WARN_LIMIT => Some(RegisterKind::Word(self.vout_uv_warn_limit)),
            CMD_IOUT_OC_WARN_LIMIT => Some(RegisterKind::Word(self.iout_oc_warn_limit)),
            CMD_OT_FAULT_LIMIT => Some(RegisterKind::Word(self.ot_fault_limit)),
            CMD_OT_WARN_LIMIT => Some(RegisterKind::Word(self.ot_warn_limit)),
            CMD_UT_WARN_LIMIT => Some(RegisterKind::Word(self.ut_warn_limit)),
            CMD_VIN_OV_WARN_LIMIT => Some(RegisterKind::Word(self.vin_ov_warn_limit)),
            CMD_VIN_UV_WARN_LIMIT => Some(RegisterKind::Word(self.vin_uv_warn_limit)),
            CMD_PIN_OP_WARN_LIMIT => Some(RegisterKind::Word(self.pin_op_warn_limit)),

            // Read-only telemetry (word)
            CMD_READ_VIN => Some(RegisterKind::Word(self.read_vin)),
            CMD_READ_VOUT => Some(RegisterKind::Word(self.read_vout)),
            CMD_READ_IOUT => Some(RegisterKind::Word(self.read_iout)),
            CMD_READ_TEMPERATURE_1 => Some(RegisterKind::Word(self.read_temperature_1)),
            CMD_READ_PIN => Some(RegisterKind::Word(self.read_pin)),

            // Scratch registers (word)
            CMD_USER_SCRATCH_1 => Some(RegisterKind::Word(self.user_scratch[0])),
            CMD_USER_SCRATCH_2 => Some(RegisterKind::Word(self.user_scratch[1])),
            CMD_USER_SCRATCH_3 => Some(RegisterKind::Word(self.user_scratch[2])),
            CMD_USER_SCRATCH_4 => Some(RegisterKind::Word(self.user_scratch[3])),

            // Block read identification
            CMD_MFR_ID => Some(RegisterKind::Block(MFR_ID_DATA)),
            CMD_MFR_MODEL => Some(RegisterKind::Block(MFR_MODEL_DATA)),
            CMD_MFR_REVISION => Some(RegisterKind::Block(MFR_REVISION_DATA)),
            CMD_IC_DEVICE_ID => Some(RegisterKind::Block(IC_DEVICE_ID_DATA)),
            CMD_IC_DEVICE_REV => Some(RegisterKind::Block(IC_DEVICE_REV_DATA)),

            // MFR registers (non-extended)
            CMD_MFR_FLT_CONFIG => Some(RegisterKind::Byte(self.mfr_flt_config)),
            CMD_MFR_OP_FAULT_RESPONSE => Some(RegisterKind::Word(self.mfr_op_fault_response)),
            CMD_MFR_ADC_CONFIG => Some(RegisterKind::Byte(self.mfr_adc_config)),
            CMD_MFR_AVG_SEL => Some(RegisterKind::Byte(self.mfr_avg_sel)),
            CMD_MFR_LOFF => Some(RegisterKind::Byte(self.mfr_loff)),
            CMD_MFR_SYSTEM_STATUS1 => Some(RegisterKind::Word(self.mfr_system_status1)),
            CMD_MFR_SYSTEM_STATUS2 => Some(RegisterKind::Word(self.mfr_system_status2)),
            CMD_MFR_PMB_STAT => Some(RegisterKind::Byte(self.mfr_pmb_stat)),
            CMD_MFR_SPECIAL_ID => Some(RegisterKind::Word(MFR_SPECIAL_ID_VALUE)),
            CMD_MFR_SD_CAUSE => Some(RegisterKind::Byte(self.mfr_sd_cause)),
            CMD_MFR_CONFIG1 => Some(RegisterKind::Word(self.mfr_config1)),
            CMD_MFR_CONFIG2 => Some(RegisterKind::Word(self.mfr_config2)),
            CMD_MFR_ON_OFF_CONFIG => Some(RegisterKind::Word(self.mfr_on_off_config)),
            CMD_MFR_REBOOT_CONTROL => Some(RegisterKind::Byte(self.mfr_reboot_control)),

            _ => None,
        }
    }

    /// Reads an extended register (accessed via 0xFE prefix).
    fn read_extended_register(&self, cmd: u8) -> Option<RegisterKind> {
        // Extended registers map to the same MFR register space.
        // The 0xFE prefix selects the extended command page.
        match cmd {
            CMD_MFR_CONFIG1 => Some(RegisterKind::Word(self.mfr_config1)),
            CMD_MFR_CONFIG2 => Some(RegisterKind::Word(self.mfr_config2)),
            CMD_MFR_ON_OFF_CONFIG => Some(RegisterKind::Word(self.mfr_on_off_config)),
            CMD_MFR_REBOOT_CONTROL => Some(RegisterKind::Byte(self.mfr_reboot_control)),
            _ => None,
        }
    }

    /// Returns true if the given command is write-protected under current settings.
    fn is_write_protected(&self, cmd: u8) -> bool {
        // WRITE_PROTECT, PAGE, and CLEAR_FAULTS are never blocked.
        if cmd == CMD_WRITE_PROTECT || cmd == CMD_PAGE || cmd == CMD_CLEAR_FAULTS {
            return false;
        }

        // WP1 (bit 7): blocks all writes except WRITE_PROTECT, PAGE
        if self.write_protect & WP1_BIT != 0 {
            return true;
        }

        // WP2 (bit 6): blocks all writes except above + OPERATION, CLEAR_FAULTS
        if self.write_protect & WP2_BIT != 0 {
            return cmd != CMD_OPERATION;
        }

        false
    }

    /// Clears all latched W1C status registers.
    fn clear_faults(&mut self) {
        self.status_vout = 0;
        self.status_iout = 0;
        self.status_input = 0;
        self.status_temperature = 0;
        self.status_cml = 0;
        self.status_other = 0;
        self.status_mfr_specific = 0;
        self.mfr_system_status1 = 0;
        self.mfr_system_status2 = 0;
        self.mfr_pmb_stat = 0;
        self.mfr_loff = 0;
    }

    /// Writes a byte value to the given command register.
    fn write_byte_register(&mut self, cmd: u8, val: u8, extended: bool) -> Result<(), BusError> {
        if !extended && self.is_write_protected(cmd) {
            return Err(BusError::DataNak);
        }
        if extended {
            return self.write_extended_byte(cmd, val);
        }
        match cmd {
            CMD_PAGE => {
                self.page = val;
                Ok(())
            }
            CMD_OPERATION => {
                self.operation = val;
                Ok(())
            }
            CMD_WRITE_PROTECT => {
                self.write_protect = val;
                Ok(())
            }
            CMD_IOUT_OC_FAULT_RESPONSE => {
                self.iout_oc_fault_response = val;
                Ok(())
            }
            CMD_OT_FAULT_RESPONSE => {
                self.ot_fault_response = val;
                Ok(())
            }
            CMD_VIN_OV_FAULT_RESPONSE => {
                self.vin_ov_fault_response = val;
                Ok(())
            }
            CMD_VIN_UV_FAULT_RESPONSE => {
                self.vin_uv_fault_response = val;
                Ok(())
            }
            CMD_MFR_FLT_CONFIG => {
                self.mfr_flt_config = val;
                Ok(())
            }
            CMD_MFR_ADC_CONFIG => {
                self.mfr_adc_config = val;
                Ok(())
            }
            CMD_MFR_AVG_SEL => {
                self.mfr_avg_sel = val;
                Ok(())
            }
            CMD_MFR_REBOOT_CONTROL => {
                self.mfr_reboot_control = val;
                Ok(())
            }
            // W1C status byte registers
            CMD_STATUS_VOUT => {
                self.status_vout &= !val;
                Ok(())
            }
            CMD_STATUS_IOUT => {
                self.status_iout &= !val;
                Ok(())
            }
            CMD_STATUS_INPUT => {
                self.status_input &= !val;
                Ok(())
            }
            CMD_STATUS_TEMPERATURE => {
                self.status_temperature &= !val;
                Ok(())
            }
            CMD_STATUS_CML => {
                self.status_cml &= !val;
                Ok(())
            }
            CMD_STATUS_OTHER => {
                self.status_other &= !val;
                Ok(())
            }
            CMD_STATUS_MFR_SPECIFIC => {
                self.status_mfr_specific &= !val;
                Ok(())
            }
            CMD_MFR_PMB_STAT => {
                self.mfr_pmb_stat &= !val;
                Ok(())
            }
            CMD_MFR_LOFF => {
                self.mfr_loff &= !val;
                Ok(())
            }
            // Read-only and constant byte registers
            CMD_CAPABILITY | CMD_PMBUS_REVISION | CMD_MFR_COMMON | CMD_MFR_SD_CAUSE => {
                Err(BusError::DataNak)
            }
            // STATUS_BYTE write: W1C to sub-status registers
            CMD_STATUS_BYTE => {
                self.apply_status_byte_w1c(val);
                Ok(())
            }
            _ => Err(BusError::DataNak),
        }
    }

    /// Writes a word value to the given command register.
    fn write_word_register(&mut self, cmd: u8, val: u16, extended: bool) -> Result<(), BusError> {
        if !extended && self.is_write_protected(cmd) {
            return Err(BusError::DataNak);
        }
        if extended {
            return self.write_extended_word(cmd, val);
        }
        match cmd {
            CMD_VOUT_OV_WARN_LIMIT => {
                self.vout_ov_warn_limit = val;
                Ok(())
            }
            CMD_VOUT_UV_WARN_LIMIT => {
                self.vout_uv_warn_limit = val;
                Ok(())
            }
            CMD_IOUT_OC_WARN_LIMIT => {
                self.iout_oc_warn_limit = val;
                Ok(())
            }
            CMD_OT_FAULT_LIMIT => {
                self.ot_fault_limit = val;
                Ok(())
            }
            CMD_OT_WARN_LIMIT => {
                self.ot_warn_limit = val;
                Ok(())
            }
            CMD_UT_WARN_LIMIT => {
                self.ut_warn_limit = val;
                Ok(())
            }
            CMD_VIN_OV_WARN_LIMIT => {
                self.vin_ov_warn_limit = val;
                Ok(())
            }
            CMD_VIN_UV_WARN_LIMIT => {
                self.vin_uv_warn_limit = val;
                Ok(())
            }
            CMD_PIN_OP_WARN_LIMIT => {
                self.pin_op_warn_limit = val;
                Ok(())
            }
            CMD_USER_SCRATCH_1 => {
                self.user_scratch[0] = val;
                Ok(())
            }
            CMD_USER_SCRATCH_2 => {
                self.user_scratch[1] = val;
                Ok(())
            }
            CMD_USER_SCRATCH_3 => {
                self.user_scratch[2] = val;
                Ok(())
            }
            CMD_USER_SCRATCH_4 => {
                self.user_scratch[3] = val;
                Ok(())
            }
            CMD_MFR_OP_FAULT_RESPONSE => {
                self.mfr_op_fault_response = val;
                Ok(())
            }
            CMD_MFR_CONFIG1 => {
                self.mfr_config1 = val;
                Ok(())
            }
            CMD_MFR_CONFIG2 => {
                self.mfr_config2 = val;
                Ok(())
            }
            CMD_MFR_ON_OFF_CONFIG => {
                self.mfr_on_off_config = val;
                Ok(())
            }
            // W1C status word registers
            CMD_MFR_SYSTEM_STATUS1 => {
                self.mfr_system_status1 &= !val;
                Ok(())
            }
            CMD_MFR_SYSTEM_STATUS2 => {
                self.mfr_system_status2 &= !val;
                Ok(())
            }
            // STATUS_WORD write: W1C to sub-status
            CMD_STATUS_WORD => {
                self.apply_status_word_w1c(val);
                Ok(())
            }
            // Read-only word registers
            CMD_READ_VIN
            | CMD_READ_VOUT
            | CMD_READ_IOUT
            | CMD_READ_TEMPERATURE_1
            | CMD_READ_PIN
            | CMD_MFR_SPECIAL_ID => Err(BusError::DataNak),
            _ => Err(BusError::DataNak),
        }
    }

    /// Writes to an extended byte register (0xFE prefix).
    fn write_extended_byte(&mut self, cmd: u8, val: u8) -> Result<(), BusError> {
        match cmd {
            CMD_MFR_REBOOT_CONTROL => {
                self.mfr_reboot_control = val;
                Ok(())
            }
            _ => Err(BusError::DataNak),
        }
    }

    /// Writes to an extended word register (0xFE prefix).
    fn write_extended_word(&mut self, cmd: u8, val: u16) -> Result<(), BusError> {
        match cmd {
            CMD_MFR_CONFIG1 => {
                self.mfr_config1 = val;
                Ok(())
            }
            CMD_MFR_CONFIG2 => {
                self.mfr_config2 = val;
                Ok(())
            }
            CMD_MFR_ON_OFF_CONFIG => {
                self.mfr_on_off_config = val;
                Ok(())
            }
            _ => Err(BusError::DataNak),
        }
    }

    /// Applies W1C semantics for a STATUS_BYTE write.
    fn apply_status_byte_w1c(&mut self, val: u8) {
        // Bit 4: IOUT_OC_FAULT → clear status_iout bit 7
        if val & (1 << 4) != 0 {
            self.status_iout &= !0x80;
        }
        // Bit 3: VIN_UV_FAULT → clear status_input bit 4
        if val & (1 << 3) != 0 {
            self.status_input &= !0x10;
        }
        // Bit 2: TEMPERATURE → clear all status_temperature
        if val & (1 << 2) != 0 {
            self.status_temperature = 0;
        }
        // Bit 1: CML → clear all status_cml
        if val & (1 << 1) != 0 {
            self.status_cml = 0;
        }
        // Bit 0: NONE_OF_THE_ABOVE → clear status_other and status_mfr_specific
        if val & 1 != 0 {
            self.status_other = 0;
            self.status_mfr_specific = 0;
        }
    }

    /// Applies W1C semantics for a STATUS_WORD write.
    fn apply_status_word_w1c(&mut self, val: u16) {
        let low = val as u8;
        let high = (val >> 8) as u8;

        // Low byte: same as STATUS_BYTE W1C
        self.apply_status_byte_w1c(low);

        // High byte bit 7 (word bit 15): VOUT → clear all status_vout
        if high & 0x80 != 0 {
            self.status_vout = 0;
        }
        // High byte bit 6 (word bit 14): IOUT → clear all status_iout
        if high & 0x40 != 0 {
            self.status_iout = 0;
        }
        // High byte bit 5 (word bit 13): INPUT → clear all status_input
        if high & 0x20 != 0 {
            self.status_input = 0;
        }
        // High byte bit 4 (word bit 12): MFR → clear status_mfr_specific
        if high & 0x10 != 0 {
            self.status_mfr_specific = 0;
        }
        // High byte bit 1 (word bit 9): OTHER → clear status_other
        if high & 0x02 != 0 {
            self.status_other = 0;
        }
    }

    /// Fills a read buffer from the current register value.
    fn fill_read_buffer(&self, buf: &mut [u8]) -> Result<(), BusError> {
        let kind = self
            .read_register(self.command, self.extended)
            .ok_or(BusError::DataNak)?;

        match kind {
            RegisterKind::Byte(val) => {
                for b in buf.iter_mut() {
                    *b = val;
                }
            }
            RegisterKind::Word(val) => {
                let le = val.to_le_bytes();
                for (i, b) in buf.iter_mut().enumerate() {
                    *b = le[i % 2];
                }
            }
            RegisterKind::Block(data) => {
                // PMBus block read: first byte is count, then data bytes
                if !buf.is_empty() {
                    buf[0] = data.len() as u8;
                }
                for (i, b) in buf.iter_mut().enumerate().skip(1) {
                    if i - 1 < data.len() {
                        *b = data[i - 1];
                    } else {
                        *b = 0xFF;
                    }
                }
            }
        }

        Ok(())
    }

    /// Processes a write operation from the host.
    fn handle_write(&mut self, data: &[u8]) -> Result<(), BusError> {
        if data.is_empty() {
            return Ok(());
        }

        // Extended command prefix: 0xFE
        if data[0] == CMD_EXTENDED_PREFIX {
            if data.len() < 2 {
                // Just the prefix byte alone — set extended mode
                self.extended = true;
                return Ok(());
            }
            self.extended = true;
            self.command = data[1];

            return match data.len() {
                2 => Ok(()), // Just set pointer
                3 => self.write_byte_register(data[1], data[2], true),
                _ => {
                    let val = u16::from_le_bytes([data[2], data[3]]);
                    self.write_word_register(data[1], val, true)
                }
            };
        }

        self.extended = false;
        self.command = data[0];

        match data.len() {
            1 => {
                // Send-byte commands
                if data[0] == CMD_CLEAR_FAULTS {
                    self.clear_faults();
                }
                Ok(())
            }
            2 => self.write_byte_register(data[0], data[1], false),
            _ => {
                let val = u16::from_le_bytes([data[1], data[2]]);
                self.write_word_register(data[0], val, false)
            }
        }
    }
}

impl I2cDevice for Ltc4287 {
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
