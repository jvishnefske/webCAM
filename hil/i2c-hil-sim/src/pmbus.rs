//! Reusable PMBus protocol engine for simulated I2C devices.
//!
//! Most PMBus devices share 90% of their protocol logic: little-endian word
//! ordering, byte/word/block command sizes, write-1-to-clear (W1C) status
//! registers, extended command prefix (0xFE), and write protection. This
//! module provides a generic [`PmBusEngine`] that implements [`I2cDevice`]
//! by delegating device-specific personality to the [`PmBusDevice`] trait.
//!
//! # Architecture
//!
//! A device implementation provides two parallel arrays:
//!
//! - **Descriptors** (`&[PmBusRegDesc]`): static register metadata — command
//!   code, size, access mode, extended flag, and power-on-reset default.
//! - **Values** (`&[PmBusValue]` / `&mut [PmBusValue]`): mutable runtime
//!   register storage, indexed in parallel with descriptors.
//!
//! The engine performs linear scan lookup (bounded ≤60 entries) to find a
//! register by `(cmd, extended)` pair, then dispatches reads and writes
//! according to the descriptor's access mode.
//!
//! # Example
//!
//! ```rust
//! use i2c_hil_sim::pmbus::{PmBusEngine, PmBusDevice, PmBusRegDesc, PmBusValue};
//! use i2c_hil_sim::pmbus::{PmBusAccess, PmBusKind};
//! use i2c_hil_sim::{Address, SimBusBuilder};
//! use embedded_hal::i2c::I2c;
//!
//! struct MyDev {
//!     address: Address,
//!     values: [PmBusValue; 2],
//! }
//!
//! const MY_DESCS: [PmBusRegDesc; 2] = [
//!     PmBusRegDesc { cmd: 0x01, kind: PmBusKind::Byte, access: PmBusAccess::ReadWrite, extended: false, por_value: 0 },
//!     PmBusRegDesc { cmd: 0x88, kind: PmBusKind::Word, access: PmBusAccess::ReadOnly, extended: false, por_value: 0 },
//! ];
//!
//! impl PmBusDevice for MyDev {
//!     fn address(&self) -> Address { self.address }
//!     fn descriptors(&self) -> &[PmBusRegDesc] { &MY_DESCS }
//!     fn values(&self) -> &[PmBusValue] { &self.values }
//!     fn values_mut(&mut self) -> &mut [PmBusValue] { &mut self.values }
//!     fn computed_read(&self, _cmd: u8, _extended: bool) -> Option<PmBusValue> { None }
//!     fn handle_send_byte(&mut self, _cmd: u8) {}
//!     fn on_write(&mut self, _cmd: u8, _extended: bool, _value: PmBusValue) {}
//! }
//!
//! let dev = MyDev {
//!     address: Address::new(0x40).unwrap(),
//!     values: [PmBusValue::Byte(0), PmBusValue::Word(0x1234)],
//! };
//! let mut bus = SimBusBuilder::new()
//!     .with_device(PmBusEngine::new(dev))
//!     .build();
//!
//! // Read word register (LE)
//! let mut buf = [0u8; 2];
//! bus.write_read(0x40, &[0x88], &mut buf).unwrap();
//! assert_eq!(u16::from_le_bytes(buf), 0x1234);
//! ```

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

// --- Standard PMBus command codes ---

/// PAGE command (0x00) — selects the active page for multi-output devices.
pub const CMD_PAGE: u8 = 0x00;
/// OPERATION command (0x01) — controls device operating mode.
pub const CMD_OPERATION: u8 = 0x01;
/// CLEAR_FAULTS command (0x03) — send-byte that clears all latched status.
pub const CMD_CLEAR_FAULTS: u8 = 0x03;
/// WRITE_PROTECT command (0x10) — controls register write protection.
pub const CMD_WRITE_PROTECT: u8 = 0x10;
/// STORE_DEFAULT_ALL command (0x11) — stores configuration to default NVM.
pub const CMD_STORE_DEFAULT_ALL: u8 = 0x11;
/// RESTORE_DEFAULT_ALL command (0x12) — restores configuration from default NVM.
pub const CMD_RESTORE_DEFAULT_ALL: u8 = 0x12;
/// STORE_USER_ALL command (0x15) — stores configuration to user NVM.
pub const CMD_STORE_USER_ALL: u8 = 0x15;
/// RESTORE_USER_ALL command (0x16) — restores configuration from user NVM.
pub const CMD_RESTORE_USER_ALL: u8 = 0x16;
/// CAPABILITY command (0x19) — reports device protocol capabilities.
pub const CMD_CAPABILITY: u8 = 0x19;
/// ON_OFF_CONFIG command (0x02) — configures device on/off control sources.
pub const CMD_ON_OFF_CONFIG: u8 = 0x02;
/// VOUT_MODE command (0x20) — output voltage mode and exponent.
pub const CMD_VOUT_MODE: u8 = 0x20;
/// VOUT_COMMAND command (0x21) — output voltage set point.
pub const CMD_VOUT_COMMAND: u8 = 0x21;
/// VOUT_TRIM command (0x22) — output voltage trim offset.
pub const CMD_VOUT_TRIM: u8 = 0x22;
/// VOUT_CAL_OFFSET command (0x23) — output voltage calibration offset.
pub const CMD_VOUT_CAL_OFFSET: u8 = 0x23;
/// VOUT_MAX command (0x24) — maximum allowed output voltage.
pub const CMD_VOUT_MAX: u8 = 0x24;
/// VOUT_MARGIN_HIGH command (0x25) — margin-high output voltage target.
pub const CMD_VOUT_MARGIN_HIGH: u8 = 0x25;
/// VOUT_MARGIN_LOW command (0x26) — margin-low output voltage target.
pub const CMD_VOUT_MARGIN_LOW: u8 = 0x26;
/// VOUT_TRANSITION_RATE command (0x27) — output voltage transition rate.
pub const CMD_VOUT_TRANSITION_RATE: u8 = 0x27;
/// VOUT_DROOP command (0x28) — output voltage droop (load-line slope).
pub const CMD_VOUT_DROOP: u8 = 0x28;
/// FREQUENCY_SWITCH command (0x33) — switching frequency setting.
pub const CMD_FREQUENCY_SWITCH: u8 = 0x33;
/// INTERLEAVE command (0x37) — phase interleave configuration.
pub const CMD_INTERLEAVE: u8 = 0x37;
/// IOUT_CAL_GAIN command (0x38) — current sense gain calibration.
pub const CMD_IOUT_CAL_GAIN: u8 = 0x38;
/// IOUT_CAL_OFFSET command (0x39) — current sense offset calibration.
pub const CMD_IOUT_CAL_OFFSET: u8 = 0x39;
/// VOUT_OV_FAULT_LIMIT command (0x40) — output overvoltage fault threshold.
pub const CMD_VOUT_OV_FAULT_LIMIT: u8 = 0x40;
/// VOUT_OV_FAULT_RESPONSE command (0x41) — output overvoltage fault response.
pub const CMD_VOUT_OV_FAULT_RESPONSE: u8 = 0x41;
/// VOUT_OV_WARN_LIMIT command (0x42).
pub const CMD_VOUT_OV_WARN_LIMIT: u8 = 0x42;
/// VOUT_UV_WARN_LIMIT command (0x43).
pub const CMD_VOUT_UV_WARN_LIMIT: u8 = 0x43;
/// VOUT_UV_FAULT_LIMIT command (0x44) — output undervoltage fault threshold.
pub const CMD_VOUT_UV_FAULT_LIMIT: u8 = 0x44;
/// VOUT_UV_FAULT_RESPONSE command (0x45) — output undervoltage fault response.
pub const CMD_VOUT_UV_FAULT_RESPONSE: u8 = 0x45;
/// IOUT_OC_FAULT_LIMIT command (0x46).
pub const CMD_IOUT_OC_FAULT_LIMIT: u8 = 0x46;
/// IOUT_OC_FAULT_RESPONSE command (0x47).
pub const CMD_IOUT_OC_FAULT_RESPONSE: u8 = 0x47;
/// IOUT_OC_WARN_LIMIT command (0x4A).
pub const CMD_IOUT_OC_WARN_LIMIT: u8 = 0x4A;
/// IOUT_UC_FAULT_LIMIT command (0x4B) — output undercurrent fault threshold.
pub const CMD_IOUT_UC_FAULT_LIMIT: u8 = 0x4B;
/// OT_FAULT_LIMIT command (0x4F).
pub const CMD_OT_FAULT_LIMIT: u8 = 0x4F;
/// OT_FAULT_RESPONSE command (0x50).
pub const CMD_OT_FAULT_RESPONSE: u8 = 0x50;
/// OT_WARN_LIMIT command (0x51).
pub const CMD_OT_WARN_LIMIT: u8 = 0x51;
/// UT_WARN_LIMIT command (0x52).
pub const CMD_UT_WARN_LIMIT: u8 = 0x52;
/// VIN_OV_FAULT_LIMIT command (0x55) — input overvoltage fault threshold.
pub const CMD_VIN_OV_FAULT_LIMIT: u8 = 0x55;
/// VIN_OV_FAULT_RESPONSE command (0x56).
pub const CMD_VIN_OV_FAULT_RESPONSE: u8 = 0x56;
/// VIN_OV_WARN_LIMIT command (0x57).
pub const CMD_VIN_OV_WARN_LIMIT: u8 = 0x57;
/// VIN_UV_WARN_LIMIT command (0x58).
pub const CMD_VIN_UV_WARN_LIMIT: u8 = 0x58;
/// VIN_UV_FAULT_LIMIT command (0x59) — input undervoltage fault threshold.
pub const CMD_VIN_UV_FAULT_LIMIT: u8 = 0x59;
/// VIN_UV_FAULT_RESPONSE command (0x5A).
pub const CMD_VIN_UV_FAULT_RESPONSE: u8 = 0x5A;
/// POWER_GOOD_ON command (0x5E) — power good on threshold.
pub const CMD_POWER_GOOD_ON: u8 = 0x5E;
/// TON_DELAY command (0x60) — turn-on delay time.
pub const CMD_TON_DELAY: u8 = 0x60;
/// TON_RISE command (0x61) — turn-on rise time.
pub const CMD_TON_RISE: u8 = 0x61;
/// TOFF_DELAY command (0x64) — turn-off delay time.
pub const CMD_TOFF_DELAY: u8 = 0x64;
/// TOFF_FALL command (0x65) — turn-off fall time.
pub const CMD_TOFF_FALL: u8 = 0x65;
/// PIN_OP_WARN_LIMIT command (0x6B).
pub const CMD_PIN_OP_WARN_LIMIT: u8 = 0x6B;
/// STATUS_BYTE command (0x78) — summarises device status in one byte.
pub const CMD_STATUS_BYTE: u8 = 0x78;
/// STATUS_WORD command (0x79) — extended status summary (LE word).
pub const CMD_STATUS_WORD: u8 = 0x79;
/// STATUS_VOUT command (0x7A) — output voltage fault/warning status.
pub const CMD_STATUS_VOUT: u8 = 0x7A;
/// STATUS_IOUT command (0x7B) — output current fault/warning status.
pub const CMD_STATUS_IOUT: u8 = 0x7B;
/// STATUS_INPUT command (0x7C) — input fault/warning status.
pub const CMD_STATUS_INPUT: u8 = 0x7C;
/// STATUS_TEMPERATURE command (0x7D) — temperature fault/warning status.
pub const CMD_STATUS_TEMPERATURE: u8 = 0x7D;
/// STATUS_CML command (0x7E) — communications/logic fault status.
pub const CMD_STATUS_CML: u8 = 0x7E;
/// STATUS_OTHER command (0x7F) — other faults.
pub const CMD_STATUS_OTHER: u8 = 0x7F;
/// STATUS_MFR_SPECIFIC command (0x80) — manufacturer-specific status.
pub const CMD_STATUS_MFR_SPECIFIC: u8 = 0x80;
/// READ_VIN command (0x88) — input voltage telemetry.
pub const CMD_READ_VIN: u8 = 0x88;
/// READ_VOUT command (0x8B) — output voltage telemetry.
pub const CMD_READ_VOUT: u8 = 0x8B;
/// READ_IOUT command (0x8C) — output current telemetry.
pub const CMD_READ_IOUT: u8 = 0x8C;
/// READ_TEMPERATURE_1 command (0x8D) — temperature telemetry.
pub const CMD_READ_TEMPERATURE_1: u8 = 0x8D;
/// READ_TEMPERATURE_3 command (0x8F) — tertiary temperature telemetry.
pub const CMD_READ_TEMPERATURE_3: u8 = 0x8F;
/// READ_DUTY_CYCLE command (0x94) — duty cycle telemetry.
pub const CMD_READ_DUTY_CYCLE: u8 = 0x94;
/// READ_FREQUENCY command (0x95) — switching frequency telemetry.
pub const CMD_READ_FREQUENCY: u8 = 0x95;
/// READ_PIN command (0x97) — input power telemetry.
pub const CMD_READ_PIN: u8 = 0x97;
/// PMBUS_REVISION command (0x98) — reports PMBus protocol revision.
pub const CMD_PMBUS_REVISION: u8 = 0x98;
/// MFR_ID command (0x99) — manufacturer identification (block read).
pub const CMD_MFR_ID: u8 = 0x99;
/// MFR_MODEL command (0x9A) — device model (block read).
pub const CMD_MFR_MODEL: u8 = 0x9A;
/// MFR_REVISION command (0x9B) — device revision (block read).
pub const CMD_MFR_REVISION: u8 = 0x9B;
/// EXTENDED_PREFIX command (0xFE) — prefix byte for extended command space.
pub const CMD_EXTENDED_PREFIX: u8 = 0xFE;

/// Write-protect bit masks.
const WP1_BIT: u8 = 0x80;
const WP2_BIT: u8 = 0x40;

/// Register access mode for a PMBus command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmBusAccess {
    /// Read-only register — writes return [`BusError::DataNak`].
    ReadOnly,
    /// Read-write register — writes store the new value directly.
    ReadWrite,
    /// Write-1-to-clear register — writing a 1 bit clears the corresponding
    /// stored bit (`stored &= !written`).
    W1C,
    /// Send-byte command — single command byte with no data (e.g. CLEAR_FAULTS).
    SendByte,
}

/// Register data size for a PMBus command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmBusKind {
    /// Single-byte register.
    Byte,
    /// Two-byte little-endian word register.
    Word,
    /// Variable-length block read (byte count prefix + data).
    Block,
}

/// Static descriptor for one PMBus register.
///
/// Paired 1:1 with a [`PmBusValue`] in the device's values array.
#[derive(Debug, Clone, Copy)]
pub struct PmBusRegDesc {
    /// PMBus command code (0x00–0xFD).
    pub cmd: u8,
    /// Data size category.
    pub kind: PmBusKind,
    /// Access mode.
    pub access: PmBusAccess,
    /// Whether this register requires the 0xFE extended prefix.
    pub extended: bool,
    /// Power-on-reset default value (low 8 bits for Byte, full 16 for Word,
    /// ignored for Block).
    pub por_value: u16,
}

/// Runtime value of a PMBus register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmBusValue {
    /// Single byte value.
    Byte(u8),
    /// Little-endian 16-bit word value.
    Word(u16),
    /// Block data reference (static lifetime, no allocation).
    Block(&'static [u8]),
}

/// Device-specific personality for the PMBus protocol engine.
///
/// Implementations provide the register table, runtime values, and hooks
/// for device-specific behavior (computed status, fault clearing, write
/// side-effects).
pub trait PmBusDevice {
    /// Returns the 7-bit I2C address this device responds to.
    fn address(&self) -> Address;

    /// Returns the static register descriptor table.
    ///
    /// Must be the same length as [`values`](PmBusDevice::values) and
    /// [`values_mut`](PmBusDevice::values_mut).
    fn descriptors(&self) -> &[PmBusRegDesc];

    /// Returns the runtime register values (parallel to descriptors).
    fn values(&self) -> &[PmBusValue];

    /// Returns mutable runtime register values (parallel to descriptors).
    fn values_mut(&mut self) -> &mut [PmBusValue];

    /// Returns a computed value for a register, overriding stored value.
    ///
    /// Called on every read. Return `Some` to override the stored value
    /// (e.g. for STATUS_BYTE/WORD aggregation), or `None` to use the
    /// stored value from the values array.
    fn computed_read(&self, cmd: u8, extended: bool) -> Option<PmBusValue>;

    /// Handles a send-byte command (1-byte write with no data).
    ///
    /// Called for registers with [`PmBusAccess::SendByte`] access mode.
    fn handle_send_byte(&mut self, cmd: u8);

    /// Post-write hook called after the engine has applied the write.
    ///
    /// `value` is the original value written by the host (before W1C
    /// modification). Use this for cascading W1C to sub-status registers
    /// or other side effects.
    fn on_write(&mut self, cmd: u8, extended: bool, value: PmBusValue);
}

/// Generic PMBus protocol engine that wraps a [`PmBusDevice`] and
/// implements [`I2cDevice`].
///
/// Handles all protocol framing: LE word byte order, extended command
/// prefix (0xFE), byte/word/block dispatch, W1C semantics, write
/// protection, and send-byte commands.
pub struct PmBusEngine<D: PmBusDevice> {
    device: D,
    command: u8,
    extended: bool,
}

impl<D: PmBusDevice> PmBusEngine<D> {
    /// Creates a new engine wrapping the given device.
    pub fn new(device: D) -> Self {
        Self {
            device,
            command: 0,
            extended: false,
        }
    }

    /// Returns a shared reference to the underlying device.
    ///
    /// Use this to read telemetry values or inspect device state.
    pub fn device(&self) -> &D {
        &self.device
    }

    /// Returns a mutable reference to the underlying device.
    ///
    /// Use this to inject telemetry values or modify device state.
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    /// Finds a register index by command code and extended flag.
    ///
    /// Returns `None` if no matching descriptor exists.
    fn find_register(&self, cmd: u8, extended: bool) -> Option<usize> {
        self.device
            .descriptors()
            .iter()
            .position(|d| d.cmd == cmd && d.extended == extended)
    }

    /// Returns true if the given command is write-protected under current
    /// WRITE_PROTECT settings.
    fn is_write_protected(&self, cmd: u8) -> bool {
        // WRITE_PROTECT, PAGE, and CLEAR_FAULTS are never blocked.
        if cmd == CMD_WRITE_PROTECT || cmd == CMD_PAGE || cmd == CMD_CLEAR_FAULTS {
            return false;
        }

        let wp = self.current_write_protect();

        // WP1 (bit 7): blocks all writes except WRITE_PROTECT, PAGE, CLEAR_FAULTS
        if wp & WP1_BIT != 0 {
            return true;
        }

        // WP2 (bit 6): blocks all writes except above + OPERATION
        if wp & WP2_BIT != 0 {
            return cmd != CMD_OPERATION;
        }

        false
    }

    /// Reads the current WRITE_PROTECT register value from the device.
    fn current_write_protect(&self) -> u8 {
        if let Some(idx) = self.find_register(CMD_WRITE_PROTECT, false) {
            match self.device.values()[idx] {
                PmBusValue::Byte(v) => v,
                _ => 0,
            }
        } else {
            0
        }
    }

    /// Fills a read buffer from the current command register.
    fn fill_read_buffer(&self, buf: &mut [u8]) -> Result<(), BusError> {
        // Check for computed override first
        let value = self
            .device
            .computed_read(self.command, self.extended)
            .or_else(|| {
                let idx = self.find_register(self.command, self.extended)?;
                Some(self.device.values()[idx])
            })
            .ok_or(BusError::DataNak)?;

        match value {
            PmBusValue::Byte(val) => {
                for b in buf.iter_mut() {
                    *b = val;
                }
            }
            PmBusValue::Word(val) => {
                let le = val.to_le_bytes();
                for (i, b) in buf.iter_mut().enumerate() {
                    *b = le[i % 2];
                }
            }
            PmBusValue::Block(data) => {
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
                // Just the prefix byte — set extended mode for next read
                self.extended = true;
                return Ok(());
            }
            self.extended = true;
            self.command = data[1];

            if data.len() == 2 {
                return Ok(()); // Just set pointer
            }

            return self.write_value(&data[2..], data[1], true);
        }

        self.extended = false;
        self.command = data[0];

        if data.len() == 1 {
            // Pointer set + possible send-byte
            if let Some(idx) = self.find_register(data[0], false) {
                if self.device.descriptors()[idx].access == PmBusAccess::SendByte {
                    self.device.handle_send_byte(data[0]);
                }
            }
            return Ok(());
        }

        self.write_value(&data[1..], data[0], false)
    }

    /// Writes register data (after command byte has been extracted).
    fn write_value(&mut self, payload: &[u8], cmd: u8, extended: bool) -> Result<(), BusError> {
        let idx = self.find_register(cmd, extended).ok_or(BusError::DataNak)?;
        let desc = self.device.descriptors()[idx];

        // Check write protection (only for non-extended commands)
        if !extended && self.is_write_protected(cmd) {
            return Err(BusError::DataNak);
        }

        // Build the written value
        let written = match desc.kind {
            PmBusKind::Byte => PmBusValue::Byte(payload[0]),
            PmBusKind::Word => {
                if payload.len() >= 2 {
                    PmBusValue::Word(u16::from_le_bytes([payload[0], payload[1]]))
                } else {
                    // Single byte write to word register — treat as byte
                    PmBusValue::Byte(payload[0])
                }
            }
            PmBusKind::Block => {
                // Block writes not supported by engine (read-only blocks)
                return Err(BusError::DataNak);
            }
        };

        // Apply write based on access mode
        match desc.access {
            PmBusAccess::ReadOnly => return Err(BusError::DataNak),
            PmBusAccess::SendByte => return Err(BusError::DataNak),
            PmBusAccess::ReadWrite => {
                self.device.values_mut()[idx] = written;
            }
            PmBusAccess::W1C => {
                let stored = &mut self.device.values_mut()[idx];
                match (stored, &written) {
                    (PmBusValue::Byte(s), PmBusValue::Byte(w)) => *s &= !w,
                    (PmBusValue::Word(s), PmBusValue::Word(w)) => *s &= !w,
                    (PmBusValue::Word(s), PmBusValue::Byte(w)) => {
                        // Byte write to W1C word — apply to low byte only
                        *s &= !((*w) as u16);
                    }
                    _ => {}
                }
            }
        }

        // Post-write hook
        self.device.on_write(cmd, extended, written);

        Ok(())
    }
}

impl<D: PmBusDevice> I2cDevice for PmBusEngine<D> {
    fn address(&self) -> Address {
        self.device.address()
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => self.handle_write(data)?,
                Operation::Read(buf) => self.fill_read_buffer(buf)?,
            }
        }
        Ok(())
    }
}
