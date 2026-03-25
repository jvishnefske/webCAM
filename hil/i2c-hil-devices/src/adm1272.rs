//! ADM1272 High Voltage Positive Hot Swap Controller simulation.
//!
//! Models an Analog Devices ADM1272 with PMBus protocol using the generic
//! [`PmBusEngine`](i2c_hil_sim::PmBusEngine) from `i2c-hil-sim`. The ADM1272 is a hot swap controller
//! with current/voltage/power telemetry and configurable fault responses.
//!
//! # Usage
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address, PmBusEngine};
//! use i2c_hil_devices::Adm1272;
//! use embedded_hal::i2c::I2c;
//!
//! let mut dev = Adm1272::new(Address::new(0x10).unwrap());
//! dev.set_read_vin(0x1234);
//!
//! let mut bus = SimBusBuilder::new()
//!     .with_device(PmBusEngine::new(dev))
//!     .build();
//!
//! let mut buf = [0u8; 2];
//! bus.write_read(0x10, &[0x88], &mut buf).unwrap();
//! assert_eq!(u16::from_le_bytes(buf), 0x1234);
//! ```

use i2c_hil_sim::pmbus::{
    PmBusAccess, PmBusDevice, PmBusKind, PmBusRegDesc, PmBusValue, CMD_CAPABILITY,
    CMD_CLEAR_FAULTS, CMD_IOUT_OC_FAULT_LIMIT, CMD_IOUT_OC_WARN_LIMIT, CMD_MFR_ID, CMD_OPERATION,
    CMD_OT_FAULT_LIMIT, CMD_OT_WARN_LIMIT, CMD_PAGE, CMD_PMBUS_REVISION, CMD_READ_IOUT,
    CMD_READ_PIN, CMD_READ_TEMPERATURE_1, CMD_READ_VIN, CMD_READ_VOUT, CMD_STATUS_BYTE,
    CMD_STATUS_CML, CMD_STATUS_INPUT, CMD_STATUS_IOUT, CMD_STATUS_MFR_SPECIFIC,
    CMD_STATUS_TEMPERATURE, CMD_STATUS_VOUT, CMD_STATUS_WORD, CMD_VIN_OV_WARN_LIMIT,
    CMD_VIN_UV_WARN_LIMIT, CMD_WRITE_PROTECT,
};
use i2c_hil_sim::Address;

const MFR_ID_DATA: &[u8] = b"ADI";
const MFR_MODEL_DATA: &[u8] = b"ADM1272";
const MFR_REVISION_DATA: &[u8] = b"A1";
const NUM_REGS: usize = 28;

const DESCRIPTORS: [PmBusRegDesc; NUM_REGS] = [
    // 0: PAGE
    PmBusRegDesc {
        cmd: CMD_PAGE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 1: OPERATION
    PmBusRegDesc {
        cmd: CMD_OPERATION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x80,
    },
    // 2: CLEAR_FAULTS
    PmBusRegDesc {
        cmd: CMD_CLEAR_FAULTS,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    // 3: WRITE_PROTECT
    PmBusRegDesc {
        cmd: CMD_WRITE_PROTECT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    // 4: CAPABILITY
    PmBusRegDesc {
        cmd: CMD_CAPABILITY,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0xB0,
    },
    // 5: VIN_OV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VIN_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 6: VIN_UV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VIN_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    // 7: IOUT_OC_FAULT_LIMIT
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 8: IOUT_OC_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 9: OT_FAULT_LIMIT
    PmBusRegDesc {
        cmd: CMD_OT_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 10: OT_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_OT_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 11: STATUS_BYTE (computed)
    PmBusRegDesc {
        cmd: CMD_STATUS_BYTE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 12: STATUS_WORD (computed)
    PmBusRegDesc {
        cmd: CMD_STATUS_WORD,
        kind: PmBusKind::Word,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 13: STATUS_VOUT
    PmBusRegDesc {
        cmd: CMD_STATUS_VOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 14: STATUS_IOUT
    PmBusRegDesc {
        cmd: CMD_STATUS_IOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 15: STATUS_INPUT
    PmBusRegDesc {
        cmd: CMD_STATUS_INPUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 16: STATUS_TEMPERATURE
    PmBusRegDesc {
        cmd: CMD_STATUS_TEMPERATURE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 17: STATUS_CML
    PmBusRegDesc {
        cmd: CMD_STATUS_CML,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 18: STATUS_MFR_SPECIFIC
    PmBusRegDesc {
        cmd: CMD_STATUS_MFR_SPECIFIC,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 19: READ_VIN
    PmBusRegDesc {
        cmd: CMD_READ_VIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 20: READ_VOUT
    PmBusRegDesc {
        cmd: CMD_READ_VOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 21: READ_IOUT
    PmBusRegDesc {
        cmd: CMD_READ_IOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 22: READ_TEMPERATURE_1
    PmBusRegDesc {
        cmd: CMD_READ_TEMPERATURE_1,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 23: READ_PIN
    PmBusRegDesc {
        cmd: CMD_READ_PIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 24: PMBUS_REVISION
    PmBusRegDesc {
        cmd: CMD_PMBUS_REVISION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x22,
    },
    // 25: MFR_ID
    PmBusRegDesc {
        cmd: CMD_MFR_ID,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 26: MFR_MODEL
    PmBusRegDesc {
        cmd: 0x9A,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 27: MFR_REVISION
    PmBusRegDesc {
        cmd: 0x9B,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
];

/// Register index constants for direct value array access.
const IDX_STATUS_VOUT: usize = 13;
const IDX_STATUS_IOUT: usize = 14;
const IDX_STATUS_INPUT: usize = 15;
const IDX_STATUS_TEMPERATURE: usize = 16;
const IDX_STATUS_CML: usize = 17;
const IDX_STATUS_MFR_SPECIFIC: usize = 18;
const IDX_READ_VIN: usize = 19;
const IDX_READ_VOUT: usize = 20;
const IDX_READ_IOUT: usize = 21;
const IDX_READ_TEMP: usize = 22;
const IDX_READ_PIN: usize = 23;

/// Simulated Analog Devices ADM1272 Hot Swap Controller.
///
/// Provides PMBus protocol with injectable telemetry, W1C status registers,
/// and computed STATUS_BYTE/WORD aggregation. Wrap in [`PmBusEngine`](i2c_hil_sim::PmBusEngine) for
/// use on a [`SimBus`](i2c_hil_sim::SimBus).
pub struct Adm1272 {
    address: Address,
    values: [PmBusValue; NUM_REGS],
}

impl Adm1272 {
    /// Creates a new ADM1272 at the given address with power-on reset defaults.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            values: [
                PmBusValue::Byte(0),                  // PAGE
                PmBusValue::Byte(0x80),               // OPERATION
                PmBusValue::Byte(0),                  // CLEAR_FAULTS placeholder
                PmBusValue::Byte(0),                  // WRITE_PROTECT
                PmBusValue::Byte(0xB0),               // CAPABILITY
                PmBusValue::Word(0x7FFF),             // VIN_OV_WARN_LIMIT
                PmBusValue::Word(0x0000),             // VIN_UV_WARN_LIMIT
                PmBusValue::Word(0x7FFF),             // IOUT_OC_FAULT_LIMIT
                PmBusValue::Word(0x7FFF),             // IOUT_OC_WARN_LIMIT
                PmBusValue::Word(0x7FFF),             // OT_FAULT_LIMIT
                PmBusValue::Word(0x7FFF),             // OT_WARN_LIMIT
                PmBusValue::Byte(0),                  // STATUS_BYTE placeholder
                PmBusValue::Word(0),                  // STATUS_WORD placeholder
                PmBusValue::Byte(0),                  // STATUS_VOUT
                PmBusValue::Byte(0),                  // STATUS_IOUT
                PmBusValue::Byte(0),                  // STATUS_INPUT
                PmBusValue::Byte(0),                  // STATUS_TEMPERATURE
                PmBusValue::Byte(0),                  // STATUS_CML
                PmBusValue::Byte(0),                  // STATUS_MFR_SPECIFIC
                PmBusValue::Word(0),                  // READ_VIN
                PmBusValue::Word(0),                  // READ_VOUT
                PmBusValue::Word(0),                  // READ_IOUT
                PmBusValue::Word(0),                  // READ_TEMPERATURE_1
                PmBusValue::Word(0),                  // READ_PIN
                PmBusValue::Byte(0x22),               // PMBUS_REVISION
                PmBusValue::Block(MFR_ID_DATA),       // MFR_ID
                PmBusValue::Block(MFR_MODEL_DATA),    // MFR_MODEL
                PmBusValue::Block(MFR_REVISION_DATA), // MFR_REVISION
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

    /// Injects a raw temperature telemetry value (PMBus LINEAR11 format).
    pub fn set_read_temperature_1(&mut self, raw: u16) {
        self.values[IDX_READ_TEMP] = PmBusValue::Word(raw);
    }

    /// Injects a raw input power telemetry value (PMBus LINEAR11 format).
    pub fn set_read_pin(&mut self, raw: u16) {
        self.values[IDX_READ_PIN] = PmBusValue::Word(raw);
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

    /// Computes STATUS_BYTE from sub-status registers.
    fn compute_status_byte(&self) -> u8 {
        let mut sb: u8 = 0;
        if let PmBusValue::Byte(op) = self.values[1] {
            if op & 0x80 == 0 {
                sb |= 1 << 6; // OFF
            }
        }
        if let PmBusValue::Byte(iout) = self.values[IDX_STATUS_IOUT] {
            if iout & 0x80 != 0 {
                sb |= 1 << 4; // IOUT_OC_FAULT
            }
        }
        if let PmBusValue::Byte(inp) = self.values[IDX_STATUS_INPUT] {
            if inp & 0x10 != 0 {
                sb |= 1 << 3; // VIN_UV_FAULT
            }
        }
        if let PmBusValue::Byte(temp) = self.values[IDX_STATUS_TEMPERATURE] {
            if temp != 0 {
                sb |= 1 << 2; // TEMPERATURE
            }
        }
        if let PmBusValue::Byte(cml) = self.values[IDX_STATUS_CML] {
            if cml != 0 {
                sb |= 1 << 1; // CML
            }
        }
        if let PmBusValue::Byte(mfr) = self.values[IDX_STATUS_MFR_SPECIFIC] {
            if mfr != 0 {
                sb |= 1; // NONE_OF_THE_ABOVE
            }
        }
        sb
    }

    /// Computes STATUS_WORD from sub-status registers.
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
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_VOUT] {
            if v & 0x18 != 0 {
                high |= 1 << 3;
            } // POWER_GOOD#
        }
        high << 8 | low
    }

    /// Clears all latched W1C status registers.
    fn clear_all_faults(&mut self) {
        self.values[IDX_STATUS_VOUT] = PmBusValue::Byte(0);
        self.values[IDX_STATUS_IOUT] = PmBusValue::Byte(0);
        self.values[IDX_STATUS_INPUT] = PmBusValue::Byte(0);
        self.values[IDX_STATUS_TEMPERATURE] = PmBusValue::Byte(0);
        self.values[IDX_STATUS_CML] = PmBusValue::Byte(0);
        self.values[IDX_STATUS_MFR_SPECIFIC] = PmBusValue::Byte(0);
    }

    /// Applies W1C cascade for STATUS_BYTE write.
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

impl PmBusDevice for Adm1272 {
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
