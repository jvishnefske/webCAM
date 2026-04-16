//! WebSocket CBOR request dispatcher for I2C bus operations.
//!
//! Provides the [`I2cBusSet`] trait and CBOR encode/decode helpers
//! so board binaries only need to implement the trait for their
//! specific bus topology.

use crate::fw_update::EncodeError;

/// Error type for I2C bus set operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusSetError {
    /// Bus index is out of range.
    InvalidBus,
    /// No device found at the specified address.
    DeviceNotFound,
    /// I2C transaction failed (NAK or bus error).
    TransactionFailed,
}

/// Error type for WebSocket CBOR request dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    /// CBOR decoding of the request failed.
    MalformedRequest,
    /// CBOR encoding of the response failed.
    Encode(EncodeError),
}

impl From<EncodeError> for DispatchError {
    fn from(e: EncodeError) -> Self {
        DispatchError::Encode(e)
    }
}

/// Trait abstracting a set of I2C buses for WebSocket dispatch.
///
/// Board binaries implement this to wire up their specific bus types.
/// Methods return `Result<(), BusSetError>` to provide structured error
/// information for I2C bus operations.
pub trait I2cBusSet {
    /// Reads bytes from an I2C device register on the specified bus.
    ///
    /// Performs a `write_read` transaction: writes the single-byte
    /// register address, then reads `buf.len()` bytes into `buf`.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus index is out of range,
    /// or [`BusSetError::TransactionFailed`] if the I2C transaction fails.
    fn i2c_read(&mut self, bus: u8, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), BusSetError>;

    /// Writes bytes to an I2C device on the specified bus.
    ///
    /// The `data` slice typically begins with the register address
    /// followed by the value bytes.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus index is out of range,
    /// or [`BusSetError::TransactionFailed`] if the I2C transaction fails.
    fn i2c_write(&mut self, bus: u8, addr: u8, data: &[u8]) -> Result<(), BusSetError>;

    /// Returns the number of active buses.
    fn bus_count(&self) -> u8;

    /// Returns the number of active devices on the given bus.
    fn device_count(&self, bus: u8) -> u8;

    /// Returns `(address, name_bytes)` for the `index`-th active device on `bus`.
    ///
    /// Devices are enumerated in slot order, skipping inactive slots.
    /// Returns `None` if `bus` or `index` is out of range.
    fn device_info(&self, bus: u8, index: u8) -> Option<(u8, &[u8])>;

    /// Returns the full register map for the device at `addr` on `bus`.
    ///
    /// Returns `None` if the bus or device does not exist.
    fn device_registers(&self, _bus: u8, _addr: u8) -> Option<&[u8]> {
        None
    }

    /// Adds a runtime device to the specified bus.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus does not support
    /// runtime configuration or the bus index is out of range.
    /// Returns [`BusSetError::DeviceNotFound`] if the address is invalid.
    /// Returns [`BusSetError::TransactionFailed`] if the bus is full or
    /// a device with the same address already exists.
    fn add_device(
        &mut self,
        _bus: u8,
        _addr: u8,
        _name: &[u8],
        _registers: &[u8],
    ) -> Result<(), BusSetError> {
        Err(BusSetError::InvalidBus)
    }

    /// Removes a runtime device from the specified bus.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus does not support
    /// runtime configuration or the bus index is out of range.
    /// Returns [`BusSetError::DeviceNotFound`] if no device with the
    /// given address exists on the bus.
    fn remove_device(&mut self, _bus: u8, _addr: u8) -> Result<(), BusSetError> {
        Err(BusSetError::InvalidBus)
    }

    /// Sets register contents on a device.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus does not support
    /// runtime configuration or the bus index is out of range.
    /// Returns [`BusSetError::DeviceNotFound`] if no device with the
    /// given address exists on the bus.
    fn set_registers(
        &mut self,
        _bus: u8,
        _addr: u8,
        _offset: u8,
        _data: &[u8],
    ) -> Result<(), BusSetError> {
        Err(BusSetError::InvalidBus)
    }

    /// Sets the number of active buses.
    ///
    /// # Errors
    ///
    /// Returns [`BusSetError::InvalidBus`] if the bus set does not support
    /// runtime configuration or the count exceeds the maximum.
    fn set_bus_count(&mut self, _count: u8) -> Result<(), BusSetError> {
        Err(BusSetError::InvalidBus)
    }

    /// Clears all runtime configuration, removing all devices from all buses.
    fn clear_all(&mut self) {}
}

/// Encodes a CBOR error response into `buf`.
///
/// Wire format: `{0: 255, 1: "message"}`.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
pub fn encode_error(buf: &mut [u8], message: &str) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(255).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.str(message).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes an I2cData response containing the read bytes.
///
/// Wire format: `{0: 1, 1: h'...' }`.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
pub fn encode_i2c_data(buf: &mut [u8], data: &[u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.bytes(data).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes a WriteOk response.
///
/// Wire format: `{0: 2}`.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
pub fn encode_write_ok(buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(2).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes a success response with only the tag.
///
/// Wire format: `{0: tag}`.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
pub fn encode_tag_ok(buf: &mut [u8], tag: u32) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(tag).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes the bus list inventory as a CBOR response from static data.
///
/// Wire format: `{0: 3, 1: [{0: bus_idx, 1: [{0: addr, 1: "name"}, ...]}, ...]}`.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small for the encoded response.
pub fn encode_bus_list(
    buf: &mut [u8],
    inventory: &[(u8, &[(u8, &str)])],
) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    // Key 0: tag = 3 (BusList)
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(3).map_err(|_| EncodeError::BufferTooSmall)?;
    // Key 1: array of bus entries
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.array(inventory.len() as u64)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    let mut bus_i = 0;
    while bus_i < inventory.len() {
        let (bus_idx, devices) = inventory[bus_i];
        enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
        // Key 0: bus index
        enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u8(bus_idx).map_err(|_| EncodeError::BufferTooSmall)?;
        // Key 1: device array
        enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.array(devices.len() as u64)
            .map_err(|_| EncodeError::BufferTooSmall)?;

        let mut dev_i = 0;
        while dev_i < devices.len() {
            let (addr, name) = devices[dev_i];
            enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
            enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
            enc.u8(addr).map_err(|_| EncodeError::BufferTooSmall)?;
            enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
            enc.str(name).map_err(|_| EncodeError::BufferTooSmall)?;
            dev_i += 1;
        }
        bus_i += 1;
    }

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes the bus list from an [`I2cBusSet`] using its query methods.
///
/// Same wire format as [`encode_bus_list`] but reads topology dynamically.
/// Returns the number of bytes written.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
fn encode_bus_list_dynamic<B: I2cBusSet>(buses: &B, buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    let count = buses.bus_count();
    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(3).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.array(count as u64)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    let mut bus_i = 0u8;
    while bus_i < count {
        let dev_count = buses.device_count(bus_i);
        enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u8(bus_i).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.array(dev_count as u64)
            .map_err(|_| EncodeError::BufferTooSmall)?;

        let mut dev_i = 0u8;
        while dev_i < dev_count {
            if let Some((addr, name)) = buses.device_info(bus_i, dev_i) {
                enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u8(addr).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
                let name_str = core::str::from_utf8(name).unwrap_or("?");
                enc.str(name_str).map_err(|_| EncodeError::BufferTooSmall)?;
            }
            dev_i += 1;
        }
        bus_i += 1;
    }

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes the full configuration as a CBOR response.
///
/// Wire format: `{0: 35, 1: [{0: bus_idx, 1: [{0: addr, 1: "name"}, ...]}, ...]}`.
/// Same structure as bus list but with tag 35.
///
/// # Errors
///
/// Returns [`EncodeError::BufferTooSmall`] if the buffer is too small.
fn encode_config<B: I2cBusSet>(buses: &B, buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    let count = buses.bus_count();
    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(35).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.array(count as u64)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    let mut bus_i = 0u8;
    while bus_i < count {
        let dev_count = buses.device_count(bus_i);
        enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u8(bus_i).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
        enc.array(dev_count as u64)
            .map_err(|_| EncodeError::BufferTooSmall)?;

        let mut dev_i = 0u8;
        while dev_i < dev_count {
            if let Some((addr, name)) = buses.device_info(bus_i, dev_i) {
                enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u8(addr).map_err(|_| EncodeError::BufferTooSmall)?;
                enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
                let name_str = core::str::from_utf8(name).unwrap_or("?");
                enc.str(name_str).map_err(|_| EncodeError::BufferTooSmall)?;
            }
            dev_i += 1;
        }
        bus_i += 1;
    }

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Decodes a CBOR request from a byte slice and dispatches it.
///
/// Parses the CBOR map to extract the message tag (key 0), then
/// executes the corresponding I2C operation on the appropriate bus.
/// The CBOR-encoded response is written into `resp_buf`.
///
/// # Supported request tags
///
/// | Tag | Request       | Response      |
/// |-----|---------------|---------------|
/// | 1   | I2cRead       | I2cData (1)   |
/// | 2   | I2cWrite      | WriteOk (2)   |
/// | 3   | ListBuses     | BusList (3)   |
/// | 12  | RebootBootsel | Error (255)   |
/// | 30  | AddDevice     | TagOk (30)    |
/// | 31  | RemoveDevice  | TagOk (31)    |
/// | 32  | SetRegisters  | TagOk (32)    |
/// | 33  | SetBusCount   | TagOk (33)    |
/// | 34  | ClearAll      | TagOk (34)    |
/// | 35  | GetConfig     | Config (35)   |
///
/// # Errors
///
/// Returns [`DispatchError::MalformedRequest`] if CBOR decoding fails, or
/// [`DispatchError::Encode`] if the response buffer is too small.
/// I2C transaction errors are reported as Error responses rather
/// than function-level errors.
pub fn handle_request<B: I2cBusSet>(
    buses: &mut B,
    request: &[u8],
    resp_buf: &mut [u8],
) -> Result<usize, DispatchError> {
    let mut dec = minicbor::Decoder::new(request);
    let _map_len = dec.map().map_err(|_| DispatchError::MalformedRequest)?;

    // Read tag from key 0
    let _key0 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
    let tag = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;

    match tag {
        // I2cRead: {0:1, 1:bus, 2:addr, 3:reg, 4:len}
        1 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let bus = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k2 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let addr = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k3 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let reg = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k4 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let len = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;

            // Cap read length to a reasonable maximum
            let read_len = if len > 128 { 128 } else { len as usize };
            let mut read_buf = [0u8; 128];

            match buses.i2c_read(bus, addr, reg, &mut read_buf[..read_len]) {
                Ok(()) => Ok(encode_i2c_data(resp_buf, &read_buf[..read_len])?),
                Err(_) => Ok(encode_error(resp_buf, "i2c read failed")?),
            }
        }
        // I2cWrite: {0:2, 1:bus, 2:addr, 3:data}
        2 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let bus = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k2 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let addr = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k3 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let data = dec.bytes().map_err(|_| DispatchError::MalformedRequest)?;

            match buses.i2c_write(bus, addr, data) {
                Ok(()) => Ok(encode_write_ok(resp_buf)?),
                Err(_) => Ok(encode_error(resp_buf, "i2c write failed")?),
            }
        }
        // ListBuses: {0:3}
        3 => Ok(encode_bus_list_dynamic(buses, resp_buf)?),
        // RebootBootsel: {0:12}
        12 => Ok(encode_error(resp_buf, "reboot not supported in sim")?),
        // AddDevice: {0:30, 1:bus, 2:addr, 3:"name", 4:h'registers'}
        30 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let bus = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k2 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let addr = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k3 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let name = dec.str().map_err(|_| DispatchError::MalformedRequest)?;
            let _k4 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let registers = dec.bytes().map_err(|_| DispatchError::MalformedRequest)?;

            match buses.add_device(bus, addr, name.as_bytes(), registers) {
                Ok(()) => Ok(encode_tag_ok(resp_buf, 30)?),
                Err(_) => Ok(encode_error(resp_buf, "add device failed")?),
            }
        }
        // RemoveDevice: {0:31, 1:bus, 2:addr}
        31 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let bus = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k2 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let addr = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;

            match buses.remove_device(bus, addr) {
                Ok(()) => Ok(encode_tag_ok(resp_buf, 31)?),
                Err(_) => Ok(encode_error(resp_buf, "remove device failed")?),
            }
        }
        // SetRegisters: {0:32, 1:bus, 2:addr, 3:offset, 4:h'data'}
        32 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let bus = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k2 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let addr = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k3 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let offset = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;
            let _k4 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let data = dec.bytes().map_err(|_| DispatchError::MalformedRequest)?;

            match buses.set_registers(bus, addr, offset, data) {
                Ok(()) => Ok(encode_tag_ok(resp_buf, 32)?),
                Err(_) => Ok(encode_error(resp_buf, "set registers failed")?),
            }
        }
        // SetBusCount: {0:33, 1:count}
        33 => {
            let _k1 = dec.u32().map_err(|_| DispatchError::MalformedRequest)?;
            let count = dec.u8().map_err(|_| DispatchError::MalformedRequest)?;

            match buses.set_bus_count(count) {
                Ok(()) => Ok(encode_tag_ok(resp_buf, 33)?),
                Err(_) => Ok(encode_error(resp_buf, "set bus count failed")?),
            }
        }
        // ClearAll: {0:34}
        34 => {
            buses.clear_all();
            Ok(encode_tag_ok(resp_buf, 34)?)
        }
        // GetConfig: {0:35}
        35 => Ok(encode_config(buses, resp_buf)?),
        // Unknown tag
        _ => Ok(encode_error(resp_buf, "unknown request tag")?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBusSet {
        fail_reads: bool,
    }

    impl I2cBusSet for MockBusSet {
        fn i2c_read(
            &mut self,
            _bus: u8,
            _addr: u8,
            _reg: u8,
            buf: &mut [u8],
        ) -> Result<(), BusSetError> {
            if self.fail_reads {
                return Err(BusSetError::TransactionFailed);
            }
            let mut i = 0;
            while i < buf.len() {
                buf[i] = (i & 0xFF) as u8;
                i += 1;
            }
            Ok(())
        }

        fn i2c_write(&mut self, _bus: u8, _addr: u8, _data: &[u8]) -> Result<(), BusSetError> {
            Ok(())
        }

        fn bus_count(&self) -> u8 {
            2
        }

        fn device_count(&self, bus: u8) -> u8 {
            match bus {
                0 => 2,
                1 => 1,
                _ => 0,
            }
        }

        fn device_info(&self, bus: u8, index: u8) -> Option<(u8, &[u8])> {
            match (bus, index) {
                (0, 0) => Some((0x48, b"TMP1075")),
                (0, 1) => Some((0x50, b"EEPROM")),
                (1, 0) => Some((0x41, b"INA230")),
                _ => None,
            }
        }
    }

    fn decode_tag(buf: &[u8]) -> u32 {
        let mut dec = minicbor::Decoder::new(buf);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        dec.u32().unwrap()
    }

    fn decode_error_message(buf: &[u8]) -> &str {
        let mut dec = minicbor::Decoder::new(buf);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        let _tag = dec.u32().unwrap();
        let _k1 = dec.u32().unwrap();
        dec.str().unwrap()
    }

    /// Encodes `{0: tag}` into `buf`, returns bytes written.
    fn encode_simple_request(buf: &mut [u8], tag: u32) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(1).unwrap();
        enc.u32(0).unwrap();
        enc.u32(tag).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:1, 1:bus, 2:addr, 3:reg, 4:len}` into `buf`.
    fn encode_i2c_read_request(buf: &mut [u8], bus: u8, addr: u8, reg: u8, len: u8) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(5).unwrap();
        enc.u32(0).unwrap();
        enc.u32(1).unwrap();
        enc.u32(1).unwrap();
        enc.u8(bus).unwrap();
        enc.u32(2).unwrap();
        enc.u8(addr).unwrap();
        enc.u32(3).unwrap();
        enc.u8(reg).unwrap();
        enc.u32(4).unwrap();
        enc.u8(len).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:2, 1:bus, 2:addr, 3:data}` into `buf`.
    fn encode_i2c_write_request(buf: &mut [u8], bus: u8, addr: u8, data: &[u8]) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(4).unwrap();
        enc.u32(0).unwrap();
        enc.u32(2).unwrap();
        enc.u32(1).unwrap();
        enc.u8(bus).unwrap();
        enc.u32(2).unwrap();
        enc.u8(addr).unwrap();
        enc.u32(3).unwrap();
        enc.bytes(data).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:30, 1:bus, 2:addr, 3:"name", 4:h'registers'}` into `buf`.
    fn encode_add_device_request(
        buf: &mut [u8],
        bus: u8,
        addr: u8,
        name: &str,
        registers: &[u8],
    ) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(5).unwrap();
        enc.u32(0).unwrap();
        enc.u32(30).unwrap();
        enc.u32(1).unwrap();
        enc.u8(bus).unwrap();
        enc.u32(2).unwrap();
        enc.u8(addr).unwrap();
        enc.u32(3).unwrap();
        enc.str(name).unwrap();
        enc.u32(4).unwrap();
        enc.bytes(registers).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:31, 1:bus, 2:addr}` into `buf`.
    fn encode_remove_device_request(buf: &mut [u8], bus: u8, addr: u8) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(3).unwrap();
        enc.u32(0).unwrap();
        enc.u32(31).unwrap();
        enc.u32(1).unwrap();
        enc.u8(bus).unwrap();
        enc.u32(2).unwrap();
        enc.u8(addr).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:32, 1:bus, 2:addr, 3:offset, 4:h'data'}` into `buf`.
    fn encode_set_registers_request(
        buf: &mut [u8],
        bus: u8,
        addr: u8,
        offset: u8,
        data: &[u8],
    ) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(5).unwrap();
        enc.u32(0).unwrap();
        enc.u32(32).unwrap();
        enc.u32(1).unwrap();
        enc.u8(bus).unwrap();
        enc.u32(2).unwrap();
        enc.u8(addr).unwrap();
        enc.u32(3).unwrap();
        enc.u8(offset).unwrap();
        enc.u32(4).unwrap();
        enc.bytes(data).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    /// Encodes `{0:33, 1:count}` into `buf`.
    fn encode_set_bus_count_request(buf: &mut [u8], count: u8) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut *buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(2).unwrap();
        enc.u32(0).unwrap();
        enc.u32(33).unwrap();
        enc.u32(1).unwrap();
        enc.u8(count).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    #[test]
    fn test_encode_error() {
        let mut buf = [0u8; 64];
        let n = encode_error(&mut buf, "test error").unwrap();
        assert_eq!(decode_tag(&buf[..n]), 255);
        assert_eq!(decode_error_message(&buf[..n]), "test error");
    }

    #[test]
    fn test_encode_i2c_data() {
        let mut buf = [0u8; 64];
        let data = [0xDE, 0xAD];
        let n = encode_i2c_data(&mut buf, &data).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 1);
        let mut dec = minicbor::Decoder::new(&buf[..n]);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        let _tag = dec.u32().unwrap();
        let _k1 = dec.u32().unwrap();
        let payload = dec.bytes().unwrap();
        assert_eq!(payload, &[0xDE, 0xAD]);
    }

    #[test]
    fn test_encode_write_ok() {
        let mut buf = [0u8; 64];
        let n = encode_write_ok(&mut buf).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 2);
    }

    #[test]
    fn test_encode_tag_ok() {
        let mut buf = [0u8; 64];
        let n = encode_tag_ok(&mut buf, 30).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 30);
    }

    #[test]
    fn test_encode_bus_list() {
        let inventory: &[(u8, &[(u8, &str)])] =
            &[(0, &[(0x48, "TMP1075")]), (1, &[(0x41, "INA230")])];
        let mut buf = [0u8; 256];
        let n = encode_bus_list(&mut buf, inventory).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 3);
    }

    #[test]
    fn test_encode_bus_list_empty() {
        let inventory: &[(u8, &[(u8, &str)])] = &[];
        let mut buf = [0u8; 64];
        let n = encode_bus_list(&mut buf, inventory).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 3);
    }

    #[test]
    fn test_encode_error_buffer_too_small() {
        let mut buf = [0u8; 3];
        assert!(encode_error(&mut buf, "too long").is_err());
    }

    #[test]
    fn test_handle_request_list_buses() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_simple_request(&mut req, 3);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 3);
    }

    #[test]
    fn test_handle_request_i2c_read() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_i2c_read_request(&mut req, 0, 0x48, 0x00, 2);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 1);
    }

    #[test]
    fn test_handle_request_i2c_write() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_i2c_write_request(&mut req, 0, 0x48, &[0x00, 0x42]);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 2);
    }

    #[test]
    fn test_handle_request_unknown_tag() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_simple_request(&mut req, 99);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "unknown request tag");
    }

    #[test]
    fn test_handle_request_reboot_bootsel() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_simple_request(&mut req, 12);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
    }

    #[test]
    fn test_handle_request_i2c_read_failure() {
        let mut buses = MockBusSet { fail_reads: true };
        let mut req = [0u8; 64];
        let req_len = encode_i2c_read_request(&mut req, 0, 0x48, 0x00, 2);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "i2c read failed");
    }

    #[test]
    fn test_handle_request_add_device_default_fails() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 256];
        let req_len = encode_add_device_request(&mut req, 0, 0x48, "TMP1075", &[0; 4]);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "add device failed");
    }

    #[test]
    fn test_handle_request_remove_device_default_fails() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_remove_device_request(&mut req, 0, 0x48);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "remove device failed");
    }

    #[test]
    fn test_handle_request_set_registers_default_fails() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_set_registers_request(&mut req, 0, 0x48, 0, &[0xAB]);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "set registers failed");
    }

    #[test]
    fn test_handle_request_set_bus_count_default_fails() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_set_bus_count_request(&mut req, 5);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 255);
        assert_eq!(decode_error_message(&resp[..n]), "set bus count failed");
    }

    #[test]
    fn test_handle_request_clear_all() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_simple_request(&mut req, 34);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 34);
    }

    #[test]
    fn test_handle_request_get_config() {
        let mut buses = MockBusSet { fail_reads: false };
        let mut req = [0u8; 64];
        let req_len = encode_simple_request(&mut req, 35);
        let mut resp = [0u8; 256];
        let n = handle_request::<MockBusSet>(&mut buses, &req[..req_len], &mut resp).unwrap();
        assert_eq!(decode_tag(&resp[..n]), 35);
    }

    #[test]
    fn test_encode_bus_list_dynamic() {
        let buses = MockBusSet { fail_reads: false };
        let mut buf = [0u8; 256];
        let n = encode_bus_list_dynamic(&buses, &mut buf).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 3);
    }

    #[test]
    fn test_default_device_registers_returns_none() {
        let buses = MockBusSet { fail_reads: false };
        // MockBusSet uses the default device_registers impl which returns None
        assert!(buses.device_registers(0, 0x48).is_none());
    }
}
