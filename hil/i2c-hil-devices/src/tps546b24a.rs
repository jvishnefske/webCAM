//! TPS546B24A Buck Converter simulation.
//!
//! Models a Texas Instruments TPS546B24A step-down converter with PMBus
//! protocol using the generic [`PmBusEngine`](i2c_hil_sim::PmBusEngine). The TPS546B24A is a 4.5V to
//! 18V input, 40A synchronous buck converter with adaptive on-time control.
//!
//! # Usage
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address, PmBusEngine};
//! use i2c_hil_devices::Tps546b24a;
//! use embedded_hal::i2c::I2c;
//!
//! let dev = Tps546b24a::new(Address::new(0x24).unwrap());
//! let mut bus = SimBusBuilder::new()
//!     .with_device(PmBusEngine::new(dev))
//!     .build();
//! ```

use i2c_hil_sim::pmbus::{
    PmBusAccess, PmBusDevice, PmBusKind, PmBusRegDesc, PmBusValue, CMD_CAPABILITY,
    CMD_CLEAR_FAULTS, CMD_IOUT_OC_FAULT_LIMIT, CMD_IOUT_OC_WARN_LIMIT, CMD_MFR_ID, CMD_OPERATION,
    CMD_OT_FAULT_LIMIT, CMD_OT_WARN_LIMIT, CMD_PAGE, CMD_PMBUS_REVISION, CMD_READ_IOUT,
    CMD_READ_TEMPERATURE_1, CMD_READ_VIN, CMD_READ_VOUT, CMD_STATUS_BYTE, CMD_STATUS_CML,
    CMD_STATUS_INPUT, CMD_STATUS_IOUT, CMD_STATUS_MFR_SPECIFIC, CMD_STATUS_TEMPERATURE,
    CMD_STATUS_VOUT, CMD_STATUS_WORD, CMD_VIN_OV_WARN_LIMIT, CMD_VIN_UV_WARN_LIMIT,
    CMD_VOUT_COMMAND, CMD_VOUT_MODE, CMD_VOUT_OV_WARN_LIMIT, CMD_VOUT_UV_WARN_LIMIT,
    CMD_WRITE_PROTECT,
};
use i2c_hil_sim::Address;

const MFR_ID_DATA: &[u8] = b"TI";
const MFR_MODEL_DATA: &[u8] = b"TPS546B24A";
const MFR_REVISION_DATA: &[u8] = b"A";
const NUM_REGS: usize = 31;

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
    // 5: VOUT_MODE
    PmBusRegDesc {
        cmd: CMD_VOUT_MODE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x17,
    },
    // 6: VOUT_COMMAND
    PmBusRegDesc {
        cmd: CMD_VOUT_COMMAND,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0199,
    },
    // 7: VOUT_OV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VOUT_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 8: VOUT_UV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VOUT_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    // 9: VIN_OV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VIN_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 10: VIN_UV_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_VIN_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    // 11: IOUT_OC_FAULT_LIMIT
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 12: IOUT_OC_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 13: OT_FAULT_LIMIT
    PmBusRegDesc {
        cmd: CMD_OT_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 14: OT_WARN_LIMIT
    PmBusRegDesc {
        cmd: CMD_OT_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    // 15: STATUS_BYTE (computed)
    PmBusRegDesc {
        cmd: CMD_STATUS_BYTE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 16: STATUS_WORD (computed)
    PmBusRegDesc {
        cmd: CMD_STATUS_WORD,
        kind: PmBusKind::Word,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 17: STATUS_VOUT
    PmBusRegDesc {
        cmd: CMD_STATUS_VOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 18: STATUS_IOUT
    PmBusRegDesc {
        cmd: CMD_STATUS_IOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 19: STATUS_INPUT
    PmBusRegDesc {
        cmd: CMD_STATUS_INPUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 20: STATUS_TEMPERATURE
    PmBusRegDesc {
        cmd: CMD_STATUS_TEMPERATURE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 21: STATUS_CML
    PmBusRegDesc {
        cmd: CMD_STATUS_CML,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 22: STATUS_MFR_SPECIFIC
    PmBusRegDesc {
        cmd: CMD_STATUS_MFR_SPECIFIC,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    // 23: READ_VIN
    PmBusRegDesc {
        cmd: CMD_READ_VIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 24: READ_VOUT
    PmBusRegDesc {
        cmd: CMD_READ_VOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 25: READ_IOUT
    PmBusRegDesc {
        cmd: CMD_READ_IOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 26: READ_TEMPERATURE_1
    PmBusRegDesc {
        cmd: CMD_READ_TEMPERATURE_1,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 27: PMBUS_REVISION
    PmBusRegDesc {
        cmd: CMD_PMBUS_REVISION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x22,
    },
    // 28: MFR_ID
    PmBusRegDesc {
        cmd: CMD_MFR_ID,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 29: MFR_MODEL
    PmBusRegDesc {
        cmd: 0x9A,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    // 30: MFR_REVISION
    PmBusRegDesc {
        cmd: 0x9B,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
];

const IDX_STATUS_VOUT: usize = 17;
const IDX_STATUS_IOUT: usize = 18;
const IDX_STATUS_INPUT: usize = 19;
const IDX_STATUS_TEMPERATURE: usize = 20;
const IDX_STATUS_CML: usize = 21;
const IDX_STATUS_MFR_SPECIFIC: usize = 22;
const IDX_READ_VIN: usize = 23;
const IDX_READ_VOUT: usize = 24;
const IDX_READ_IOUT: usize = 25;
const IDX_READ_TEMP: usize = 26;

/// Simulated Texas Instruments TPS546B24A Buck Converter.
///
/// Provides PMBus protocol with injectable telemetry, W1C status, and
/// computed STATUS_BYTE/WORD. Wrap in [`PmBusEngine`](i2c_hil_sim::PmBusEngine) for bus use.
pub struct Tps546b24a {
    address: Address,
    values: [PmBusValue; NUM_REGS],
}

impl Tps546b24a {
    /// Creates a new TPS546B24A at the given address with POR defaults.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            values: [
                PmBusValue::Byte(0),                  // PAGE
                PmBusValue::Byte(0x80),               // OPERATION
                PmBusValue::Byte(0),                  // CLEAR_FAULTS
                PmBusValue::Byte(0),                  // WRITE_PROTECT
                PmBusValue::Byte(0xB0),               // CAPABILITY
                PmBusValue::Byte(0x17),               // VOUT_MODE
                PmBusValue::Word(0x0199),             // VOUT_COMMAND
                PmBusValue::Word(0x7FFF),             // VOUT_OV_WARN
                PmBusValue::Word(0x0000),             // VOUT_UV_WARN
                PmBusValue::Word(0x7FFF),             // VIN_OV_WARN
                PmBusValue::Word(0x0000),             // VIN_UV_WARN
                PmBusValue::Word(0x7FFF),             // IOUT_OC_FAULT
                PmBusValue::Word(0x7FFF),             // IOUT_OC_WARN
                PmBusValue::Word(0x7FFF),             // OT_FAULT
                PmBusValue::Word(0x7FFF),             // OT_WARN
                PmBusValue::Byte(0),                  // STATUS_BYTE
                PmBusValue::Word(0),                  // STATUS_WORD
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
                PmBusValue::Byte(0x22),               // PMBUS_REVISION
                PmBusValue::Block(MFR_ID_DATA),       // MFR_ID
                PmBusValue::Block(MFR_MODEL_DATA),    // MFR_MODEL
                PmBusValue::Block(MFR_REVISION_DATA), // MFR_REVISION
            ],
        }
    }

    /// Injects a raw VIN telemetry value.
    pub fn set_read_vin(&mut self, raw: u16) {
        self.values[IDX_READ_VIN] = PmBusValue::Word(raw);
    }

    /// Injects a raw VOUT telemetry value.
    pub fn set_read_vout(&mut self, raw: u16) {
        self.values[IDX_READ_VOUT] = PmBusValue::Word(raw);
    }

    /// Injects a raw IOUT telemetry value.
    pub fn set_read_iout(&mut self, raw: u16) {
        self.values[IDX_READ_IOUT] = PmBusValue::Word(raw);
    }

    /// Injects a raw temperature telemetry value.
    pub fn set_read_temperature_1(&mut self, raw: u16) {
        self.values[IDX_READ_TEMP] = PmBusValue::Word(raw);
    }

    /// Computes STATUS_BYTE from sub-status registers.
    fn compute_status_byte(&self) -> u8 {
        let mut sb: u8 = 0;
        if let PmBusValue::Byte(op) = self.values[1] {
            if op & 0x80 == 0 {
                sb |= 1 << 6;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_IOUT] {
            if v & 0x80 != 0 {
                sb |= 1 << 4;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_INPUT] {
            if v & 0x10 != 0 {
                sb |= 1 << 3;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_TEMPERATURE] {
            if v != 0 {
                sb |= 1 << 2;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_CML] {
            if v != 0 {
                sb |= 1 << 1;
            }
        }
        if let PmBusValue::Byte(v) = self.values[IDX_STATUS_MFR_SPECIFIC] {
            if v != 0 {
                sb |= 1;
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
        high << 8 | low
    }

    /// Clears all latched status registers.
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
}

impl PmBusDevice for Tps546b24a {
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
        if cmd == CMD_STATUS_BYTE {
            if let PmBusValue::Byte(v) = value {
                self.apply_status_byte_w1c(v);
            }
        }
    }
}
