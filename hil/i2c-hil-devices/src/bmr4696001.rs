//! BMR4696001 Dual-Output PoL DC-DC Converter simulation.
//!
//! Models a Flex BMR4696001 point-of-load converter with PMBus protocol
//! using the generic [`PmBusEngine`](i2c_hil_sim::PmBusEngine). The BMR4696001 is a 7.5–14V input,
//! 0.6–5V output, up to 50A/100W dual-output DC-DC converter.
//!
//! # Register Set
//!
//! The simulation models 72 registers from the BMR4696001 datasheet
//! command appendix (pages 48–49), including:
//!
//! - Configuration: PAGE, OPERATION, ON_OFF_CONFIG, WRITE_PROTECT, VOUT_MODE
//! - Output voltage: VOUT_COMMAND/TRIM/CAL_OFFSET/MAX/MARGIN_HIGH/MARGIN_LOW
//! - Timing: TON_DELAY/RISE, TOFF_DELAY/FALL, VOUT_TRANSITION_RATE
//! - Limits & fault responses for VOUT, IOUT, VIN, and temperature
//! - Status: STATUS_BYTE/WORD (computed), 6 sub-status W1C registers
//! - Telemetry: READ_VIN/VOUT/IOUT/TEMPERATURE_1/TEMPERATURE_3/DUTY_CYCLE/FREQUENCY
//! - Identification: MFR_ID ("FLEX"), MFR_MODEL ("BMR4696001"), MFR_REVISION ("A")
//! - Manufacturer-specific: USER_CONFIG, POWER_GOOD_DELAY, MFR fault responses
//!
//! # Usage
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address, PmBusEngine};
//! use i2c_hil_devices::Bmr4696001;
//! use embedded_hal::i2c::I2c;
//!
//! let dev = Bmr4696001::new(Address::new(0x20).unwrap());
//! let mut bus = SimBusBuilder::new()
//!     .with_device(PmBusEngine::new(dev))
//!     .build();
//! ```

use i2c_hil_sim::pmbus::{
    PmBusAccess, PmBusDevice, PmBusKind, PmBusRegDesc, PmBusValue, CMD_CAPABILITY,
    CMD_CLEAR_FAULTS, CMD_FREQUENCY_SWITCH, CMD_INTERLEAVE, CMD_IOUT_CAL_GAIN, CMD_IOUT_CAL_OFFSET,
    CMD_IOUT_OC_FAULT_LIMIT, CMD_IOUT_OC_FAULT_RESPONSE, CMD_IOUT_OC_WARN_LIMIT,
    CMD_IOUT_UC_FAULT_LIMIT, CMD_MFR_ID, CMD_MFR_MODEL, CMD_MFR_REVISION, CMD_ON_OFF_CONFIG,
    CMD_OPERATION, CMD_OT_FAULT_LIMIT, CMD_OT_FAULT_RESPONSE, CMD_OT_WARN_LIMIT, CMD_PAGE,
    CMD_PMBUS_REVISION, CMD_POWER_GOOD_ON, CMD_READ_DUTY_CYCLE, CMD_READ_FREQUENCY, CMD_READ_IOUT,
    CMD_READ_TEMPERATURE_1, CMD_READ_TEMPERATURE_3, CMD_READ_VIN, CMD_READ_VOUT,
    CMD_RESTORE_DEFAULT_ALL, CMD_RESTORE_USER_ALL, CMD_STATUS_BYTE, CMD_STATUS_CML,
    CMD_STATUS_INPUT, CMD_STATUS_IOUT, CMD_STATUS_MFR_SPECIFIC, CMD_STATUS_TEMPERATURE,
    CMD_STATUS_VOUT, CMD_STATUS_WORD, CMD_STORE_DEFAULT_ALL, CMD_STORE_USER_ALL, CMD_TOFF_DELAY,
    CMD_TOFF_FALL, CMD_TON_DELAY, CMD_TON_RISE, CMD_UT_WARN_LIMIT, CMD_VIN_OV_FAULT_LIMIT,
    CMD_VIN_OV_FAULT_RESPONSE, CMD_VIN_OV_WARN_LIMIT, CMD_VIN_UV_FAULT_LIMIT,
    CMD_VIN_UV_FAULT_RESPONSE, CMD_VIN_UV_WARN_LIMIT, CMD_VOUT_CAL_OFFSET, CMD_VOUT_COMMAND,
    CMD_VOUT_DROOP, CMD_VOUT_MARGIN_HIGH, CMD_VOUT_MARGIN_LOW, CMD_VOUT_MAX, CMD_VOUT_MODE,
    CMD_VOUT_OV_FAULT_LIMIT, CMD_VOUT_OV_FAULT_RESPONSE, CMD_VOUT_OV_WARN_LIMIT,
    CMD_VOUT_TRANSITION_RATE, CMD_VOUT_TRIM, CMD_VOUT_UV_FAULT_LIMIT, CMD_VOUT_UV_FAULT_RESPONSE,
    CMD_VOUT_UV_WARN_LIMIT, CMD_WRITE_PROTECT,
};
use i2c_hil_sim::Address;

const MFR_ID_DATA: &[u8] = b"FLEX";
const MFR_MODEL_DATA: &[u8] = b"BMR4696001";
const MFR_REVISION_DATA: &[u8] = b"A";

const NUM_REGS: usize = 72;

/// MFR_IOUT_OC_FAULT_RESPONSE command (0xE5).
const CMD_MFR_IOUT_OC_FAULT_RESPONSE: u8 = 0xE5;
/// USER_CONFIG command (0xD1).
const CMD_USER_CONFIG: u8 = 0xD1;
/// POWER_GOOD_DELAY command (0xD4).
const CMD_POWER_GOOD_DELAY: u8 = 0xD4;
/// MIN_VOUT_REG command (0xCE).
const CMD_MIN_VOUT_REG: u8 = 0xCE;
/// ISENSE_CONFIG command (0xD0).
const CMD_ISENSE_CONFIG: u8 = 0xD0;

const DESCRIPTORS: [PmBusRegDesc; NUM_REGS] = [
    // 0: PAGE (0x00)
    PmBusRegDesc {
        cmd: CMD_PAGE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 1: OPERATION (0x01)
    PmBusRegDesc {
        cmd: CMD_OPERATION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x40,
    },
    // 2: ON_OFF_CONFIG (0x02)
    PmBusRegDesc {
        cmd: CMD_ON_OFF_CONFIG,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x17,
    },
    // 3: CLEAR_FAULTS (0x03)
    PmBusRegDesc {
        cmd: CMD_CLEAR_FAULTS,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 4: WRITE_PROTECT (0x10)
    PmBusRegDesc {
        cmd: CMD_WRITE_PROTECT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 5: STORE_DEFAULT_ALL (0x11)
    PmBusRegDesc {
        cmd: CMD_STORE_DEFAULT_ALL,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 6: RESTORE_DEFAULT_ALL (0x12)
    PmBusRegDesc {
        cmd: CMD_RESTORE_DEFAULT_ALL,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 7: STORE_USER_ALL (0x15)
    PmBusRegDesc {
        cmd: CMD_STORE_USER_ALL,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 8: RESTORE_USER_ALL (0x16)
    PmBusRegDesc {
        cmd: CMD_RESTORE_USER_ALL,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 9: CAPABILITY (0x19)
    PmBusRegDesc {
        cmd: CMD_CAPABILITY,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0xB0,
    },
    // 10: VOUT_MODE (0x20)
    PmBusRegDesc {
        cmd: CMD_VOUT_MODE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x13,
    },
    // 11: VOUT_COMMAND (0x21)
    PmBusRegDesc {
        cmd: CMD_VOUT_COMMAND,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x2000,
    },
    // 12: VOUT_TRIM (0x22)
    PmBusRegDesc {
        cmd: CMD_VOUT_TRIM,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    // 13: VOUT_CAL_OFFSET (0x23)
    PmBusRegDesc {
        cmd: CMD_VOUT_CAL_OFFSET,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 14: VOUT_MAX (0x24)
    PmBusRegDesc {
        cmd: CMD_VOUT_MAX,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 15: VOUT_MARGIN_HIGH (0x25)
    PmBusRegDesc {
        cmd: CMD_VOUT_MARGIN_HIGH,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 16: VOUT_MARGIN_LOW (0x26)
    PmBusRegDesc {
        cmd: CMD_VOUT_MARGIN_LOW,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 17: VOUT_TRANSITION_RATE (0x27)
    PmBusRegDesc {
        cmd: CMD_VOUT_TRANSITION_RATE,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBA00,
    },
    // 18: VOUT_DROOP (0x28)
    PmBusRegDesc {
        cmd: CMD_VOUT_DROOP,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 19: FREQUENCY_SWITCH (0x33)
    PmBusRegDesc {
        cmd: CMD_FREQUENCY_SWITCH,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 20: INTERLEAVE (0x37)
    PmBusRegDesc {
        cmd: CMD_INTERLEAVE,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 21: IOUT_CAL_GAIN (0x38)
    PmBusRegDesc {
        cmd: CMD_IOUT_CAL_GAIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 22: IOUT_CAL_OFFSET (0x39)
    PmBusRegDesc {
        cmd: CMD_IOUT_CAL_OFFSET,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 23: VOUT_OV_FAULT_LIMIT (0x40)
    PmBusRegDesc {
        cmd: CMD_VOUT_OV_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 24: VOUT_OV_FAULT_RESPONSE (0x41)
    PmBusRegDesc {
        cmd: CMD_VOUT_OV_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBF,
    },
    // 25: VOUT_OV_WARN_LIMIT (0x42)
    PmBusRegDesc {
        cmd: CMD_VOUT_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 26: VOUT_UV_WARN_LIMIT (0x43)
    PmBusRegDesc {
        cmd: CMD_VOUT_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 27: VOUT_UV_FAULT_LIMIT (0x44)
    PmBusRegDesc {
        cmd: CMD_VOUT_UV_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 28: VOUT_UV_FAULT_RESPONSE (0x45)
    PmBusRegDesc {
        cmd: CMD_VOUT_UV_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBF,
    },
    // 29: IOUT_OC_FAULT_LIMIT (0x46)
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 30: IOUT_OC_FAULT_RESPONSE (0x47)
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 31: IOUT_OC_WARN_LIMIT (0x4A)
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 32: IOUT_UC_FAULT_LIMIT (0x4B)
    PmBusRegDesc {
        cmd: CMD_IOUT_UC_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 33: OT_FAULT_LIMIT (0x4F)
    PmBusRegDesc {
        cmd: CMD_OT_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xEBE8,
    },
    // 34: OT_FAULT_RESPONSE (0x50)
    PmBusRegDesc {
        cmd: CMD_OT_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x80,
    },
    // 35: OT_WARN_LIMIT (0x51)
    PmBusRegDesc {
        cmd: CMD_OT_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xEB70,
    },
    // 36: UT_WARN_LIMIT (0x52)
    PmBusRegDesc {
        cmd: CMD_UT_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 37: VIN_OV_FAULT_LIMIT (0x55)
    PmBusRegDesc {
        cmd: CMD_VIN_OV_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xDA00,
    },
    // 38: VIN_OV_FAULT_RESPONSE (0x56)
    PmBusRegDesc {
        cmd: CMD_VIN_OV_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBF,
    },
    // 39: VIN_OV_WARN_LIMIT (0x57)
    PmBusRegDesc {
        cmd: CMD_VIN_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xD3C0,
    },
    // 40: VIN_UV_WARN_LIMIT (0x58)
    PmBusRegDesc {
        cmd: CMD_VIN_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xCB66,
    },
    // 41: VIN_UV_FAULT_LIMIT (0x59)
    PmBusRegDesc {
        cmd: CMD_VIN_UV_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xCB33,
    },
    // 42: VIN_UV_FAULT_RESPONSE (0x5A)
    PmBusRegDesc {
        cmd: CMD_VIN_UV_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBF,
    },
    // 43: POWER_GOOD_ON (0x5E)
    PmBusRegDesc {
        cmd: CMD_POWER_GOOD_ON,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 44: TON_DELAY (0x60)
    PmBusRegDesc {
        cmd: CMD_TON_DELAY,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xCA80,
    },
    // 45: TON_RISE (0x61)
    PmBusRegDesc {
        cmd: CMD_TON_RISE,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xCA80,
    },
    // 46: TOFF_DELAY (0x64)
    PmBusRegDesc {
        cmd: CMD_TOFF_DELAY,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    // 47: TOFF_FALL (0x65)
    PmBusRegDesc {
        cmd: CMD_TOFF_FALL,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xCA80,
    },
    // 48: STATUS_BYTE (0x78) — computed
    PmBusRegDesc {
        cmd: CMD_STATUS_BYTE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 49: STATUS_WORD (0x79) — computed
    PmBusRegDesc {
        cmd: CMD_STATUS_WORD,
        kind: PmBusKind::Word,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 50: STATUS_VOUT (0x7A)
    PmBusRegDesc {
        cmd: CMD_STATUS_VOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 51: STATUS_IOUT (0x7B)
    PmBusRegDesc {
        cmd: CMD_STATUS_IOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 52: STATUS_INPUT (0x7C)
    PmBusRegDesc {
        cmd: CMD_STATUS_INPUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 53: STATUS_TEMPERATURE (0x7D)
    PmBusRegDesc {
        cmd: CMD_STATUS_TEMPERATURE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 54: STATUS_CML (0x7E)
    PmBusRegDesc {
        cmd: CMD_STATUS_CML,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 55: STATUS_MFR_SPECIFIC (0x80)
    PmBusRegDesc {
        cmd: CMD_STATUS_MFR_SPECIFIC,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 56: READ_VIN (0x88)
    PmBusRegDesc {
        cmd: CMD_READ_VIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 57: READ_VOUT (0x8B)
    PmBusRegDesc {
        cmd: CMD_READ_VOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 58: READ_IOUT (0x8C)
    PmBusRegDesc {
        cmd: CMD_READ_IOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 59: READ_TEMPERATURE_1 (0x8D)
    PmBusRegDesc {
        cmd: CMD_READ_TEMPERATURE_1,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 60: READ_TEMPERATURE_3 (0x8F)
    PmBusRegDesc {
        cmd: CMD_READ_TEMPERATURE_3,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 61: READ_DUTY_CYCLE (0x94)
    PmBusRegDesc {
        cmd: CMD_READ_DUTY_CYCLE,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 62: READ_FREQUENCY (0x95)
    PmBusRegDesc {
        cmd: CMD_READ_FREQUENCY,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 63: PMBUS_REVISION (0x98)
    PmBusRegDesc {
        cmd: CMD_PMBUS_REVISION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x22,
    },
    // 64: MFR_ID (0x99)
    PmBusRegDesc {
        cmd: CMD_MFR_ID,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 65: MFR_MODEL (0x9A)
    PmBusRegDesc {
        cmd: CMD_MFR_MODEL,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 66: MFR_REVISION (0x9B)
    PmBusRegDesc {
        cmd: CMD_MFR_REVISION,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 67: MIN_VOUT_REG (0xCE)
    PmBusRegDesc {
        cmd: CMD_MIN_VOUT_REG,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x8000,
    },
    // 68: ISENSE_CONFIG (0xD0)
    PmBusRegDesc {
        cmd: CMD_ISENSE_CONFIG,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x420E,
    },
    // 69: USER_CONFIG (0xD1)
    PmBusRegDesc {
        cmd: CMD_USER_CONFIG,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x10A4,
    },
    // 70: POWER_GOOD_DELAY (0xD4)
    PmBusRegDesc {
        cmd: CMD_POWER_GOOD_DELAY,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0xBA00,
    },
    // 71: MFR_IOUT_OC_FAULT_RESPONSE (0xE5)
    PmBusRegDesc {
        cmd: CMD_MFR_IOUT_OC_FAULT_RESPONSE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x80,
    },
];

/// Register index constants for direct value array access.
const IDX_STATUS_VOUT: usize = 50;
const IDX_STATUS_IOUT: usize = 51;
const IDX_STATUS_INPUT: usize = 52;
const IDX_STATUS_TEMPERATURE: usize = 53;
const IDX_STATUS_CML: usize = 54;
const IDX_STATUS_MFR_SPECIFIC: usize = 55;
const IDX_READ_VIN: usize = 56;
const IDX_READ_VOUT: usize = 57;
const IDX_READ_IOUT: usize = 58;
const IDX_READ_TEMP1: usize = 59;
const IDX_READ_TEMP3: usize = 60;
const IDX_READ_DUTY_CYCLE: usize = 61;
const IDX_READ_FREQUENCY: usize = 62;

/// Simulated Flex BMR4696001 Dual-Output PoL DC-DC Converter.
///
/// Provides PMBus protocol with 72 registers, injectable telemetry
/// (VIN/VOUT/IOUT/TEMPERATURE_1/TEMPERATURE_3/DUTY_CYCLE/FREQUENCY),
/// W1C status with cascade, and computed STATUS_BYTE/WORD aggregation.
/// Wrap in [`PmBusEngine`](i2c_hil_sim::PmBusEngine) for bus use.
pub struct Bmr4696001 {
    address: Address,
    values: [PmBusValue; NUM_REGS],
}

impl Bmr4696001 {
    /// Creates a new BMR4696001 at the given address with POR defaults.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            values: [
                PmBusValue::Byte(0),                  // 0: PAGE
                PmBusValue::Byte(0x40),               // 1: OPERATION
                PmBusValue::Byte(0x17),               // 2: ON_OFF_CONFIG
                PmBusValue::Byte(0),                  // 3: CLEAR_FAULTS
                PmBusValue::Byte(0),                  // 4: WRITE_PROTECT
                PmBusValue::Byte(0),                  // 5: STORE_DEFAULT_ALL
                PmBusValue::Byte(0),                  // 6: RESTORE_DEFAULT_ALL
                PmBusValue::Byte(0),                  // 7: STORE_USER_ALL
                PmBusValue::Byte(0),                  // 8: RESTORE_USER_ALL
                PmBusValue::Byte(0xB0),               // 9: CAPABILITY
                PmBusValue::Byte(0x13),               // 10: VOUT_MODE
                PmBusValue::Word(0x2000),             // 11: VOUT_COMMAND
                PmBusValue::Word(0x0000),             // 12: VOUT_TRIM
                PmBusValue::Word(0),                  // 13: VOUT_CAL_OFFSET
                PmBusValue::Word(0),                  // 14: VOUT_MAX
                PmBusValue::Word(0),                  // 15: VOUT_MARGIN_HIGH
                PmBusValue::Word(0),                  // 16: VOUT_MARGIN_LOW
                PmBusValue::Word(0xBA00),             // 17: VOUT_TRANSITION_RATE
                PmBusValue::Word(0),                  // 18: VOUT_DROOP
                PmBusValue::Word(0),                  // 19: FREQUENCY_SWITCH
                PmBusValue::Word(0),                  // 20: INTERLEAVE
                PmBusValue::Word(0),                  // 21: IOUT_CAL_GAIN
                PmBusValue::Word(0),                  // 22: IOUT_CAL_OFFSET
                PmBusValue::Word(0),                  // 23: VOUT_OV_FAULT_LIMIT
                PmBusValue::Byte(0xBF),               // 24: VOUT_OV_FAULT_RESPONSE
                PmBusValue::Word(0),                  // 25: VOUT_OV_WARN_LIMIT
                PmBusValue::Word(0),                  // 26: VOUT_UV_WARN_LIMIT
                PmBusValue::Word(0),                  // 27: VOUT_UV_FAULT_LIMIT
                PmBusValue::Byte(0xBF),               // 28: VOUT_UV_FAULT_RESPONSE
                PmBusValue::Word(0),                  // 29: IOUT_OC_FAULT_LIMIT
                PmBusValue::Byte(0),                  // 30: IOUT_OC_FAULT_RESPONSE (RO)
                PmBusValue::Word(0),                  // 31: IOUT_OC_WARN_LIMIT
                PmBusValue::Word(0),                  // 32: IOUT_UC_FAULT_LIMIT
                PmBusValue::Word(0xEBE8),             // 33: OT_FAULT_LIMIT
                PmBusValue::Byte(0x80),               // 34: OT_FAULT_RESPONSE
                PmBusValue::Word(0xEB70),             // 35: OT_WARN_LIMIT
                PmBusValue::Word(0),                  // 36: UT_WARN_LIMIT
                PmBusValue::Word(0xDA00),             // 37: VIN_OV_FAULT_LIMIT
                PmBusValue::Byte(0xBF),               // 38: VIN_OV_FAULT_RESPONSE
                PmBusValue::Word(0xD3C0),             // 39: VIN_OV_WARN_LIMIT
                PmBusValue::Word(0xCB66),             // 40: VIN_UV_WARN_LIMIT
                PmBusValue::Word(0xCB33),             // 41: VIN_UV_FAULT_LIMIT
                PmBusValue::Byte(0xBF),               // 42: VIN_UV_FAULT_RESPONSE
                PmBusValue::Word(0),                  // 43: POWER_GOOD_ON
                PmBusValue::Word(0xCA80),             // 44: TON_DELAY
                PmBusValue::Word(0xCA80),             // 45: TON_RISE
                PmBusValue::Word(0x0000),             // 46: TOFF_DELAY
                PmBusValue::Word(0xCA80),             // 47: TOFF_FALL
                PmBusValue::Byte(0),                  // 48: STATUS_BYTE
                PmBusValue::Word(0),                  // 49: STATUS_WORD
                PmBusValue::Byte(0),                  // 50: STATUS_VOUT
                PmBusValue::Byte(0),                  // 51: STATUS_IOUT
                PmBusValue::Byte(0),                  // 52: STATUS_INPUT
                PmBusValue::Byte(0),                  // 53: STATUS_TEMPERATURE
                PmBusValue::Byte(0),                  // 54: STATUS_CML
                PmBusValue::Byte(0),                  // 55: STATUS_MFR_SPECIFIC
                PmBusValue::Word(0),                  // 56: READ_VIN
                PmBusValue::Word(0),                  // 57: READ_VOUT
                PmBusValue::Word(0),                  // 58: READ_IOUT
                PmBusValue::Word(0),                  // 59: READ_TEMPERATURE_1
                PmBusValue::Word(0),                  // 60: READ_TEMPERATURE_3
                PmBusValue::Word(0),                  // 61: READ_DUTY_CYCLE
                PmBusValue::Word(0),                  // 62: READ_FREQUENCY
                PmBusValue::Byte(0x22),               // 63: PMBUS_REVISION
                PmBusValue::Block(MFR_ID_DATA),       // 64: MFR_ID
                PmBusValue::Block(MFR_MODEL_DATA),    // 65: MFR_MODEL
                PmBusValue::Block(MFR_REVISION_DATA), // 66: MFR_REVISION
                PmBusValue::Word(0x8000),             // 67: MIN_VOUT_REG
                PmBusValue::Word(0x420E),             // 68: ISENSE_CONFIG
                PmBusValue::Word(0x10A4),             // 69: USER_CONFIG
                PmBusValue::Word(0xBA00),             // 70: POWER_GOOD_DELAY
                PmBusValue::Byte(0x80),               // 71: MFR_IOUT_OC_FAULT_RESPONSE
            ],
        }
    }

    /// Injects a raw VIN telemetry value (PMBus LINEAR11 format).
    pub fn set_read_vin(&mut self, raw: u16) {
        self.values[IDX_READ_VIN] = PmBusValue::Word(raw);
    }

    /// Injects a raw VOUT telemetry value (PMBus LINEAR16 format).
    pub fn set_read_vout(&mut self, raw: u16) {
        self.values[IDX_READ_VOUT] = PmBusValue::Word(raw);
    }

    /// Injects a raw IOUT telemetry value (PMBus LINEAR11 format).
    pub fn set_read_iout(&mut self, raw: u16) {
        self.values[IDX_READ_IOUT] = PmBusValue::Word(raw);
    }

    /// Injects a raw TEMPERATURE_1 telemetry value (PMBus LINEAR11 format).
    pub fn set_read_temperature_1(&mut self, raw: u16) {
        self.values[IDX_READ_TEMP1] = PmBusValue::Word(raw);
    }

    /// Injects a raw TEMPERATURE_3 telemetry value (PMBus LINEAR11 format).
    pub fn set_read_temperature_3(&mut self, raw: u16) {
        self.values[IDX_READ_TEMP3] = PmBusValue::Word(raw);
    }

    /// Injects a raw duty cycle telemetry value (PMBus LINEAR11 format).
    pub fn set_read_duty_cycle(&mut self, raw: u16) {
        self.values[IDX_READ_DUTY_CYCLE] = PmBusValue::Word(raw);
    }

    /// Injects a raw switching frequency telemetry value (PMBus LINEAR11 format).
    pub fn set_read_frequency(&mut self, raw: u16) {
        self.values[IDX_READ_FREQUENCY] = PmBusValue::Word(raw);
    }

    /// Injects STATUS_VOUT bits for testing.
    pub fn set_status_vout(&mut self, val: u8) {
        self.values[IDX_STATUS_VOUT] = PmBusValue::Byte(val);
    }

    /// Injects STATUS_IOUT bits for testing.
    pub fn set_status_iout(&mut self, val: u8) {
        self.values[IDX_STATUS_IOUT] = PmBusValue::Byte(val);
    }

    /// Injects STATUS_INPUT bits for testing.
    pub fn set_status_input(&mut self, val: u8) {
        self.values[IDX_STATUS_INPUT] = PmBusValue::Byte(val);
    }

    /// Injects STATUS_TEMPERATURE bits for testing.
    pub fn set_status_temperature(&mut self, val: u8) {
        self.values[IDX_STATUS_TEMPERATURE] = PmBusValue::Byte(val);
    }

    /// Injects STATUS_CML bits for testing.
    pub fn set_status_cml(&mut self, val: u8) {
        self.values[IDX_STATUS_CML] = PmBusValue::Byte(val);
    }

    /// Injects STATUS_MFR_SPECIFIC bits for testing.
    pub fn set_status_mfr_specific(&mut self, val: u8) {
        self.values[IDX_STATUS_MFR_SPECIFIC] = PmBusValue::Byte(val);
    }

    /// Computes STATUS_BYTE from sub-status registers and OPERATION state.
    ///
    /// Bit mapping per PMBus spec:
    /// - Bit 6: OFF (OPERATION bit 7 clear)
    /// - Bit 4: IOUT_OC (STATUS_IOUT bit 7)
    /// - Bit 3: VIN_UV (STATUS_INPUT bit 4)
    /// - Bit 2: TEMPERATURE (any STATUS_TEMPERATURE bit)
    /// - Bit 1: CML (any STATUS_CML bit)
    /// - Bit 0: NONE_OF_THE_ABOVE (any STATUS_MFR_SPECIFIC bit)
    fn compute_status_byte(&self) -> u8 {
        let mut sb: u8 = 0;
        if let PmBusValue::Byte(op) = self.values[1] {
            if op & 0x80 == 0 {
                sb |= 1 << 6; // OFF
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_IOUT] {
            if v & 0x80 != 0 {
                sb |= 1 << 4; // IOUT_OC_FAULT
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_INPUT] {
            if v & 0x10 != 0 {
                sb |= 1 << 3; // VIN_UV_FAULT
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_TEMPERATURE] {
            if v != 0 {
                sb |= 1 << 2; // TEMPERATURE
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_CML] {
            if v != 0 {
                sb |= 1 << 1; // CML
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_MFR_SPECIFIC] {
            if v != 0 {
                sb |= 1; // NONE_OF_THE_ABOVE
            }
        }
        sb
    }

    /// Computes STATUS_WORD from sub-status registers.
    ///
    /// Low byte is STATUS_BYTE. High byte aggregates:
    /// - Bit 15: VOUT (any STATUS_VOUT bit)
    /// - Bit 14: IOUT (any STATUS_IOUT bit)
    /// - Bit 13: INPUT (any STATUS_INPUT bit)
    /// - Bit 12: MFR_SPECIFIC (any STATUS_MFR_SPECIFIC bit)
    fn compute_status_word(&self) -> u16 {
        let low = self.compute_status_byte() as u16;
        let mut high: u16 = 0;
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_VOUT] {
            if v != 0 {
                high |= 1 << 7;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_IOUT] {
            if v != 0 {
                high |= 1 << 6;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_INPUT] {
            if v != 0 {
                high |= 1 << 5;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_MFR_SPECIFIC] {
            if v != 0 {
                high |= 1 << 4;
            }
        }
        high << 8 | low
    }

    /// Clears all latched W1C status registers.
    fn clear_all_faults(&mut self) {
        for idx in [
            IDX_STATUS_VOUT,
            IDX_STATUS_IOUT,
            IDX_STATUS_INPUT,
            IDX_STATUS_TEMPERATURE,
            IDX_STATUS_CML,
            IDX_STATUS_MFR_SPECIFIC,
        ] {
            self.values[idx] = PmBusValue::Byte(0);
        }
    }

    /// Applies W1C cascade for STATUS_BYTE write.
    ///
    /// When the host writes a 1 to a STATUS_BYTE bit, the corresponding
    /// source bit in the sub-status register is also cleared.
    fn apply_status_byte_w1c(&mut self, val: u8) {
        if val & (1 << 4) != 0 {
            if let PmBusValue::Byte(ref mut v) = self.values[IDX_STATUS_IOUT] {
                *v &= !0x80;
            }
        }
        if val & (1 << 3) != 0 {
            if let PmBusValue::Byte(ref mut v) = self.values[IDX_STATUS_INPUT] {
                *v &= !0x10;
            }
        }
        if val & (1 << 2) != 0 {
            self.values[IDX_STATUS_TEMPERATURE] = PmBusValue::Byte(0);
        }
        if val & (1 << 1) != 0 {
            self.values[IDX_STATUS_CML] = PmBusValue::Byte(0);
        }
        if val & 1 != 0 {
            self.values[IDX_STATUS_MFR_SPECIFIC] = PmBusValue::Byte(0);
        }
    }

    /// Applies W1C cascade for STATUS_WORD write.
    ///
    /// Low byte cascades via [`apply_status_byte_w1c`](Self::apply_status_byte_w1c).
    /// High byte bits cascade to clear entire sub-status registers.
    fn apply_status_word_w1c(&mut self, val: u16) {
        let low = val as u8;
        let high = (val >> 8) as u8;
        self.apply_status_byte_w1c(low);
        if high & 0x80 != 0 {
            self.values[IDX_STATUS_VOUT] = PmBusValue::Byte(0);
        }
        if high & 0x40 != 0 {
            self.values[IDX_STATUS_IOUT] = PmBusValue::Byte(0);
        }
        if high & 0x20 != 0 {
            self.values[IDX_STATUS_INPUT] = PmBusValue::Byte(0);
        }
        if high & 0x10 != 0 {
            self.values[IDX_STATUS_MFR_SPECIFIC] = PmBusValue::Byte(0);
        }
    }
}

impl PmBusDevice for Bmr4696001 {
    fn address(&self) -> Address {
        self.address
    }

    fn descriptors(&self) -> &[PmBusRegDesc] {
        &DESCRIPTORS
    }

    fn values(&self) -> &[PmBusValue] {
        &self.values
    }

    fn values_mut(&mut self) -> &mut [PmBusValue] {
        &mut self.values
    }

    fn computed_read(&self, cmd: u8, _extended: bool) -> Option<PmBusValue> {
        match cmd {
            CMD_STATUS_BYTE => Some(PmBusValue::Byte(self.compute_status_byte())),
            CMD_STATUS_WORD => Some(PmBusValue::Word(self.compute_status_word())),
            _ => None,
        }
    }

    fn handle_send_byte(&mut self, cmd: u8) {
        if cmd == CMD_CLEAR_FAULTS {
            self.clear_all_faults();
        }
        // STORE_DEFAULT_ALL, RESTORE_DEFAULT_ALL, STORE_USER_ALL,
        // RESTORE_USER_ALL are accepted (no NAK) but are no-ops in simulation.
    }

    fn on_write(&mut self, cmd: u8, _extended: bool, value: PmBusValue) {
        match cmd {
            CMD_STATUS_BYTE => {
                if let PmBusValue::Byte(v) = value {
                    self.apply_status_byte_w1c(v);
                }
            }
            CMD_STATUS_WORD => {
                if let PmBusValue::Word(v) = value {
                    self.apply_status_word_w1c(v);
                }
            }
            _ => {}
        }
    }
}
