//! ISL68224 Triple-Output PWM Controller simulation.
//!
//! Models a Renesas ISL68224 digital multi-phase PWM controller with PMBus
//! protocol using the generic [`PmBusEngine`](i2c_hil_sim::PmBusEngine). The ISL68224 provides three
//! independent voltage rails with digital control loop.
//!
//! # Usage
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address, PmBusEngine};
//! use i2c_hil_devices::Isl68224;
//! use embedded_hal::i2c::I2c;
//!
//! let dev = Isl68224::new(Address::new(0x60).unwrap());
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
    CMD_VOUT_OV_WARN_LIMIT, CMD_VOUT_UV_WARN_LIMIT, CMD_WRITE_PROTECT,
};
use i2c_hil_sim::Address;

const MFR_ID_DATA: &[u8] = b"ISIL";
const MFR_MODEL_DATA: &[u8] = b"ISL68224";
const MFR_REVISION_DATA: &[u8] = b"A";
const NUM_REGS: usize = 29;

const DESCRIPTORS: [PmBusRegDesc; NUM_REGS] = [
    PmBusRegDesc {
        cmd: CMD_PAGE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_OPERATION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x80,
    },
    PmBusRegDesc {
        cmd: CMD_CLEAR_FAULTS,
        kind: PmBusKind::Byte,
        access: PmBusAccess::SendByte,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_WRITE_PROTECT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_CAPABILITY,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0xB0,
    },
    PmBusRegDesc {
        cmd: CMD_VOUT_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_VOUT_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    PmBusRegDesc {
        cmd: CMD_VIN_OV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_VIN_UV_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x0000,
    },
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_IOUT_OC_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_OT_FAULT_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_OT_WARN_LIMIT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadWrite,
        extended: false,
        por_value: 0x7FFF,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_BYTE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_WORD,
        kind: PmBusKind::Word,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_VOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_IOUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_INPUT,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_TEMPERATURE,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_CML,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_STATUS_MFR_SPECIFIC,
        kind: PmBusKind::Byte,
        access: PmBusAccess::W1C,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_READ_VIN,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_READ_VOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_READ_IOUT,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_READ_TEMPERATURE_1,
        kind: PmBusKind::Word,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: CMD_PMBUS_REVISION,
        kind: PmBusKind::Byte,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0x22,
    },
    PmBusRegDesc {
        cmd: CMD_MFR_ID,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: 0x9A,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
    PmBusRegDesc {
        cmd: 0x9B,
        kind: PmBusKind::Block,
        access: PmBusAccess::ReadOnly,
        extended: false,
        por_value: 0,
    },
];

const IDX_STATUS_VOUT: usize = 15;
const IDX_STATUS_IOUT: usize = 16;
const IDX_STATUS_INPUT: usize = 17;
const IDX_STATUS_TEMPERATURE: usize = 18;
const IDX_STATUS_CML: usize = 19;
const IDX_STATUS_MFR_SPECIFIC: usize = 20;
const IDX_READ_VIN: usize = 21;
const IDX_READ_VOUT: usize = 22;
const IDX_READ_IOUT: usize = 23;
const IDX_READ_TEMP: usize = 24;

/// Simulated Renesas ISL68224 Triple-Output PWM Controller.
///
/// Provides PMBus protocol with injectable telemetry and W1C status.
/// Wrap in [`PmBusEngine`](i2c_hil_sim::PmBusEngine) for bus use.
pub struct Isl68224 {
    address: Address,
    values: [PmBusValue; NUM_REGS],
}

impl Isl68224 {
    /// Creates a new ISL68224 at the given address with POR defaults.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            values: [
                PmBusValue::Byte(0),                  // PAGE
                PmBusValue::Byte(0x80),               // OPERATION
                PmBusValue::Byte(0),                  // CLEAR_FAULTS
                PmBusValue::Byte(0),                  // WRITE_PROTECT
                PmBusValue::Byte(0xB0),               // CAPABILITY
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
}

impl PmBusDevice for Isl68224 {
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

    fn on_write(&mut self, _cmd: u8, _extended: bool, _value: PmBusValue) {}
}
