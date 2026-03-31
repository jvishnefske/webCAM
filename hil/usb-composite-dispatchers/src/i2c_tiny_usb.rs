//! Device-side implementation of the i2c-tiny-usb protocol for multiple I2C buses.
//!
//! This module implements the USB vendor control transfer protocol used by
//! the Linux `i2c-tiny-usb` kernel driver. A single [`MultiI2cHandler`] manages
//! up to 16 independent I2C peripherals exposed as separate USB functions within
//! a composite device.
//!
//! # Protocol Commands
//!
//! All commands are vendor-type control transfers on endpoint 0:
//!
//! | bRequest | Direction | Description |
//! |----------|-----------|-------------|
//! | 0 (Echo) | IN | Echo back `wValue` bytes for connectivity test |
//! | 1 (GetFunc) | IN | Return 4-byte I2C functionality bitmask |
//! | 2 (SetDelay) | OUT | Set I2C clock delay from `wValue` |
//! | 3 (GetStatus) | IN | Return 1-byte transaction status |
//! | 4–7 (I2cIo) | IN/OUT | I2C read/write with BEGIN/END flags in bits 0–1 |
//!
//! # I2C IO Flag Encoding
//!
//! The Linux driver OR's `CMD_I2C_IO` (4) with `CMD_I2C_IO_BEGIN` (1) and
//! `CMD_I2C_IO_END` (2), yielding request values 4–7:
//!
//! | bRequest | BEGIN | END | Meaning |
//! |----------|-------|-----|---------|
//! | 4 | no | no | Middle segment of multi-part transfer |
//! | 5 | yes | no | First segment (send START) |
//! | 6 | no | yes | Last segment (send STOP) |
//! | 7 | yes | yes | Complete single transfer (START + STOP) |
//!
//! # Interface Routing Limitation
//!
//! The Linux `i2c-tiny-usb` driver sends `USB_TYPE_VENDOR | USB_RECIP_INTERFACE`
//! control transfers with `wIndex = I2C_slave_address` for I/O commands (not the
//! interface number). Embassy-usb delegates all vendor requests to handlers without
//! interface-based routing. As a result, the firmware cannot determine which
//! interface a given I/O request targets. Non-I/O commands use `wIndex = 0`, which
//! coincidentally matches interface 0. All unresolvable requests default to bus 0.

use defmt::{debug, info, warn, Format};
use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::Handler;
use embedded_hal::i2c::{Error as _, ErrorKind, I2c};

/// USB Vendor ID for i2c-tiny-usb composite devices.
pub const VID: u16 = 0x1c40;

/// USB Product ID for i2c-tiny-usb composite devices.
pub const PID: u16 = 0x0534;

/// Maximum number of I2C buses supported by [`MultiI2cHandler`].
const MAX_BUSES: usize = 16;

/// Base command value for I2C IO operations.
const CMD_I2C_IO: u8 = 4;

/// Flag bit indicating the first segment of a transfer (send START condition).
const CMD_I2C_IO_BEGIN: u8 = 1;

/// Flag bit indicating the last segment of a transfer (send STOP condition).
const CMD_I2C_IO_END: u8 = 2;

/// Protocol commands recognized by the i2c-tiny-usb driver.
///
/// The `I2cIo` variant carries `begin` and `end` flags extracted from the
/// two low bits of the request value (4–7).
#[derive(Clone, Copy, PartialEq, Eq, Format)]
pub enum Command {
    /// Echo test: return `wValue` bytes unchanged.
    Echo,
    /// Report supported I2C functionality bitmask.
    GetFunc,
    /// Set I2C bus clock delay (microseconds in `wValue`).
    SetDelay,
    /// Return last transaction status byte.
    GetStatus,
    /// Perform an I2C read or write operation.
    ///
    /// `begin` = true sends a START condition; `end` = true sends STOP.
    I2cIo { begin: bool, end: bool },
}

impl Command {
    /// Decode a USB `bRequest` value into a protocol command.
    ///
    /// Returns `None` for unrecognized request values.
    pub fn from_request(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Echo),
            1 => Some(Self::GetFunc),
            2 => Some(Self::SetDelay),
            3 => Some(Self::GetStatus),
            CMD_I2C_IO..=7 => {
                let flags = val - CMD_I2C_IO;
                Some(Self::I2cIo {
                    begin: flags & CMD_I2C_IO_BEGIN != 0,
                    end: flags & CMD_I2C_IO_END != 0,
                })
            }
            _ => None,
        }
    }
}

/// I2C transaction status reported to the host via `GetStatus`.
#[derive(Clone, Copy, Default, Format)]
#[repr(u8)]
pub enum Status {
    /// No transaction has occurred since last reset.
    #[default]
    Idle = 0,
    /// Last address byte was acknowledged by a device.
    AddressAck = 1,
    /// Last address byte was not acknowledged (no device at that address).
    AddressNak = 2,
}

/// I2C functionality flag: core I2C transfers supported.
pub const I2C_FUNC_I2C: u32 = 0x0000_0001;

/// I2C functionality flag: SMBus emulation capabilities.
///
/// Matches the Linux kernel's `I2C_FUNC_SMBUS_EMUL` value from
/// `include/uapi/linux/i2c.h`.
pub const I2C_FUNC_SMBUS_EMUL: u32 = 0x0EFF_0008;

/// `I2C_M_RD` flag from the Linux kernel, indicating a read operation
/// when set in the `wValue` field of an I2C IO request.
pub const I2C_M_RD: u16 = 0x0001;

/// Per-interface state for one I2C bus within the composite device.
///
/// Tracks the most recent transaction status, clock delay, and supported
/// functionality bitmask independently for each interface.
pub struct InterfaceState {
    /// Last transaction status for this interface.
    pub status: Status,
    /// I2C clock delay in microseconds.
    pub delay_us: u16,
    /// Supported I2C functionality bitmask.
    pub functionality: u32,
}

impl Default for InterfaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl InterfaceState {
    /// Creates new per-interface state with default I2C + SMBus emulation functionality.
    pub const fn new() -> Self {
        Self {
            status: Status::Idle,
            delay_us: 0,
            functionality: I2C_FUNC_I2C | I2C_FUNC_SMBUS_EMUL,
        }
    }
}

/// Recursive tuple dispatch trait for heterogeneous I2C bus collections.
///
/// Enables a single [`MultiI2cHandler`] to own and dispatch to an arbitrary
/// number of I2C peripherals of different concrete types. Buses are stored
/// as nested tuples `(Bus, (Bus, ()))` following the same pattern as
/// `i2c_hil_sim::DeviceSet`.
///
/// Index 0 dispatches to the head element; higher indices recurse into the tail.
pub trait BusSet {
    /// Number of buses in this set.
    const COUNT: usize;

    /// Write data to the bus at `index` targeting I2C address `addr`.
    fn write(&mut self, index: usize, addr: u8, data: &[u8]) -> Result<(), ErrorKind>;

    /// Read data from the bus at `index` targeting I2C address `addr`.
    fn read(&mut self, index: usize, addr: u8, buf: &mut [u8]) -> Result<(), ErrorKind>;
}

/// Base case: empty bus set with zero buses.
impl BusSet for () {
    const COUNT: usize = 0;

    fn write(&mut self, _index: usize, _addr: u8, _data: &[u8]) -> Result<(), ErrorKind> {
        Err(ErrorKind::Other)
    }

    fn read(&mut self, _index: usize, _addr: u8, _buf: &mut [u8]) -> Result<(), ErrorKind> {
        Err(ErrorKind::Other)
    }
}

/// Recursive case: head bus `B` at index 0, remaining buses `R` at index 1+.
impl<B: I2c, R: BusSet> BusSet for (B, R) {
    const COUNT: usize = 1 + R::COUNT;

    fn write(&mut self, index: usize, addr: u8, data: &[u8]) -> Result<(), ErrorKind> {
        if index == 0 {
            self.0.write(addr, data).map_err(|e| e.kind())
        } else {
            self.1.write(index - 1, addr, data)
        }
    }

    fn read(&mut self, index: usize, addr: u8, buf: &mut [u8]) -> Result<(), ErrorKind> {
        if index == 0 {
            self.0.read(addr, buf).map_err(|e| e.kind())
        } else {
            self.1.read(index - 1, addr, buf)
        }
    }
}

/// USB control transfer handler implementing the i2c-tiny-usb protocol for
/// multiple buses.
///
/// Owns a [`BusSet`] of I2C peripherals and per-bus protocol state. Processes
/// vendor-type control requests on endpoint 0 and routes them to the
/// appropriate bus based on USB interface number resolution.
///
/// Constructed via [`MultiI2cHandlerBuilder`].
///
/// See [module-level docs](self) for the interface routing limitation.
pub struct MultiI2cHandler<B: BusSet> {
    buses: B,
    states: [InterfaceState; MAX_BUSES],
    if_nums: [u8; MAX_BUSES],
}

impl<B: BusSet> MultiI2cHandler<B> {
    /// Resolve which bus index a USB control request targets.
    ///
    /// Scans the interface number table for a match against `req.index`.
    /// Returns 0 as the default fallback for unresolvable requests.
    fn resolve_interface(&self, req: &Request) -> usize {
        let raw = req.index as u8;
        let mut i = 0;
        while i < B::COUNT {
            if self.if_nums[i] == raw {
                return i;
            }
            i += 1;
        }
        0
    }
}

impl<B: BusSet> Handler for MultiI2cHandler<B> {
    fn enabled(&mut self, enabled: bool) {
        info!("USB enabled: {}", enabled);
    }

    fn reset(&mut self) {
        info!("USB reset");
        let mut i = 0;
        while i < B::COUNT {
            self.states[i].status = Status::Idle;
            i += 1;
        }
    }

    fn addressed(&mut self, addr: u8) {
        info!("USB addressed: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        info!("USB configured: {}", configured);
    }

    fn suspended(&mut self, suspended: bool) {
        info!("USB suspended: {}", suspended);
    }

    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request_type != RequestType::Vendor {
            return None;
        }

        let cmd = match Command::from_request(req.request) {
            Some(c) => c,
            None => {
                warn!(
                    "USB OUT unknown request: bRequest={} wValue={} wIndex={} wLength={}",
                    req.request, req.value, req.index, req.length,
                );
                return None;
            }
        };

        let bus = self.resolve_interface(&req);

        debug!(
            "USB OUT cmd={} bus={} wValue={} wIndex={} wLength={} data_len={}",
            cmd,
            bus,
            req.value,
            req.index,
            req.length,
            data.len(),
        );

        match cmd {
            Command::SetDelay => {
                self.states[bus].delay_us = req.value;
                debug!("I2C{} delay set to {} us", bus, req.value);
                Some(OutResponse::Accepted)
            }
            Command::I2cIo { begin, end } if req.value & I2C_M_RD == 0 => {
                let addr = (req.index & 0x7F) as u8;
                debug!(
                    "I2C{} write addr=0x{:02x} len={} begin={} end={}",
                    bus,
                    addr,
                    data.len(),
                    begin,
                    end,
                );
                let ok = self.buses.write(bus, addr, data).is_ok();
                if ok {
                    self.states[bus].status = Status::AddressAck;
                    debug!("I2C{} write ACK", bus);
                } else {
                    self.states[bus].status = Status::AddressNak;
                    warn!("I2C{} write NAK addr=0x{:02x}", bus, addr);
                }
                Some(OutResponse::Accepted)
            }
            _ => None,
        }
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        if req.request_type != RequestType::Vendor {
            return None;
        }

        let cmd = match Command::from_request(req.request) {
            Some(c) => c,
            None => {
                warn!(
                    "USB IN unknown request: bRequest={} wValue={} wIndex={} wLength={}",
                    req.request, req.value, req.index, req.length,
                );
                return None;
            }
        };

        let bus = self.resolve_interface(&req);

        debug!(
            "USB IN cmd={} bus={} wValue={} wIndex={} wLength={}",
            cmd, bus, req.value, req.index, req.length,
        );

        match cmd {
            Command::Echo => {
                let len = (req.length as usize).min(buf.len());
                debug!("Echo {} bytes", len);
                Some(InResponse::Accepted(&buf[..len]))
            }
            Command::GetFunc => {
                let func = self.states[bus].functionality.to_le_bytes();
                buf[..4].copy_from_slice(&func);
                debug!(
                    "I2C{} GetFunc -> 0x{:08x}",
                    bus, self.states[bus].functionality
                );
                Some(InResponse::Accepted(&buf[..4]))
            }
            Command::GetStatus => {
                buf[0] = self.states[bus].status as u8;
                debug!("I2C{} GetStatus -> {}", bus, self.states[bus].status);
                Some(InResponse::Accepted(&buf[..1]))
            }
            Command::I2cIo { begin, end } if req.value & I2C_M_RD != 0 => {
                let addr = (req.index & 0x7F) as u8;
                let len = (req.length as usize).min(buf.len());
                debug!(
                    "I2C{} read addr=0x{:02x} len={} begin={} end={}",
                    bus, addr, len, begin, end,
                );
                let ok = self.buses.read(bus, addr, &mut buf[..len]).is_ok();
                if ok {
                    self.states[bus].status = Status::AddressAck;
                    debug!("I2C{} read ACK", bus);
                    Some(InResponse::Accepted(&buf[..len]))
                } else {
                    self.states[bus].status = Status::AddressNak;
                    warn!("I2C{} read NAK addr=0x{:02x}", bus, addr);
                    Some(InResponse::Accepted(&[]))
                }
            }
            _ => None,
        }
    }
}

/// Type-changing builder for [`MultiI2cHandler`].
///
/// Each [`with_bus`](Self::with_bus) call prepends a bus to the internal
/// [`BusSet`] tuple and records its USB interface number. The resulting
/// bus set index 0 corresponds to the last bus added.
///
/// # Example
///
/// ```ignore
/// let handler = MultiI2cHandlerBuilder::new()
///     .with_bus(bus0, if0)
///     .with_bus(bus1, if1)
///     .build();
/// ```
pub struct MultiI2cHandlerBuilder<B: BusSet> {
    buses: B,
    if_nums: [u8; MAX_BUSES],
}

impl Default for MultiI2cHandlerBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiI2cHandlerBuilder<()> {
    /// Creates a new builder with no buses.
    pub fn new() -> Self {
        Self {
            buses: (),
            if_nums: [0; MAX_BUSES],
        }
    }
}

impl<B: BusSet> MultiI2cHandlerBuilder<B> {
    /// Adds an I2C bus with its assigned USB interface number.
    ///
    /// The new bus is prepended to the bus set (becomes index 0), shifting
    /// all previously-added buses up by one index.
    pub fn with_bus<I: I2c>(
        self,
        bus: I,
        if_num: InterfaceNumber,
    ) -> MultiI2cHandlerBuilder<(I, B)> {
        let mut if_nums = self.if_nums;
        // Shift existing entries right to make room at index 0
        let mut i = B::COUNT;
        while i > 0 {
            if_nums[i] = if_nums[i - 1];
            i -= 1;
        }
        if_nums[0] = if_num.0;
        MultiI2cHandlerBuilder {
            buses: (bus, self.buses),
            if_nums,
        }
    }

    /// Builds the handler from the accumulated buses and interface numbers.
    pub fn build(self) -> MultiI2cHandler<B> {
        MultiI2cHandler {
            buses: self.buses,
            states: core::array::from_fn(|_| InterfaceState::new()),
            if_nums: self.if_nums,
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use embassy_usb::control::Recipient;
    use embassy_usb_driver::Direction;
    use i2c_hil_sim::devices::RegisterDevice;
    use i2c_hil_sim::{Address, SimBusBuilder};

    fn vendor_out(request: u8, value: u16, index: u16, length: u16) -> Request {
        Request {
            direction: Direction::Out,
            request_type: RequestType::Vendor,
            recipient: Recipient::Interface,
            request,
            value,
            index,
            length,
        }
    }

    fn vendor_in(request: u8, value: u16, index: u16, length: u16) -> Request {
        Request {
            direction: Direction::In,
            request_type: RequestType::Vendor,
            recipient: Recipient::Interface,
            request,
            value,
            index,
            length,
        }
    }

    // --- BusSet trait tests ---

    #[test]
    fn empty_bus_set_write_returns_error() {
        let mut set: () = ();
        assert!(set.write(0, 0x48, &[]).is_err());
    }

    #[test]
    fn empty_bus_set_read_returns_error() {
        let mut set: () = ();
        let mut buf = [0u8; 2];
        assert!(set.read(0, 0x48, &mut buf).is_err());
    }

    #[test]
    fn empty_bus_set_count_is_zero() {
        assert_eq!(<() as BusSet>::COUNT, 0);
    }

    #[test]
    fn single_bus_write_dispatches() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut set = (bus, ());
        assert!(set.write(0, 0x48, &[0x01, 0xFF]).is_ok());
    }

    #[test]
    fn single_bus_count_is_one() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        type Set = (i2c_hil_sim::SimBus<(RegisterDevice<4>, ())>, ());
        assert_eq!(<Set as BusSet>::COUNT, 1);
        let _ = (bus, ());
    }

    #[test]
    fn single_bus_read_dispatches() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAB; 4]))
            .build();
        let mut set = (bus, ());
        // Set read pointer to register 0
        set.write(0, 0x48, &[0x00]).unwrap();
        let mut buf = [0u8; 1];
        set.read(0, 0x48, &mut buf).unwrap();
        assert_eq!(buf[0], 0xAB);
    }

    #[test]
    fn single_bus_wrong_addr_returns_error() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut set = (bus, ());
        assert!(set.write(0, 0x50, &[0x01]).is_err());
    }

    #[test]
    fn two_bus_dispatch_to_index_0() {
        let bus0 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let bus1 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0u8; 4]))
            .build();
        let mut set = (bus0, (bus1, ()));
        // Index 0 -> bus0 at 0x48
        assert!(set.write(0, 0x48, &[0x01, 0xFF]).is_ok());
        // bus0 doesn't have 0x50
        assert!(set.write(0, 0x50, &[0x01]).is_err());
    }

    #[test]
    fn two_bus_dispatch_to_index_1() {
        let bus0 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let bus1 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0u8; 4]))
            .build();
        let mut set = (bus0, (bus1, ()));
        // Index 1 -> bus1 at 0x50
        assert!(set.write(1, 0x50, &[0x01, 0xFF]).is_ok());
        // bus1 doesn't have 0x48
        assert!(set.write(1, 0x48, &[0x01]).is_err());
    }

    #[test]
    fn two_bus_count() {
        let bus0 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let bus1 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0u8; 4]))
            .build();
        let set = (bus0, (bus1, ()));
        type Set = (
            i2c_hil_sim::SimBus<(RegisterDevice<4>, ())>,
            (i2c_hil_sim::SimBus<(RegisterDevice<4>, ())>, ()),
        );
        assert_eq!(<Set as BusSet>::COUNT, 2);
        let _ = set;
    }

    #[test]
    fn out_of_range_index_returns_error() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut set = (bus, ());
        // Index 1 is out of range for a single-bus set — falls through to ()
        assert!(set.write(1, 0x48, &[0x01]).is_err());
    }

    // --- MultiI2cHandlerBuilder tests ---

    #[test]
    fn builder_zero_buses() {
        let handler = MultiI2cHandlerBuilder::new().build();
        assert_eq!(<() as BusSet>::COUNT, 0);
        let _ = handler;
    }

    #[test]
    fn builder_one_bus() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(3))
            .build();
        // Interface 3 should map to bus index 0
        assert_eq!(handler.if_nums[0], 3);
    }

    #[test]
    fn builder_two_buses_interface_order() {
        let bus0 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let bus1 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0u8; 4]))
            .build();
        let handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus0, InterfaceNumber(5))
            .with_bus(bus1, InterfaceNumber(7))
            .build();
        // Last added (bus1) is at BusSet index 0
        assert_eq!(handler.if_nums[0], 7);
        // First added (bus0) is at BusSet index 1
        assert_eq!(handler.if_nums[1], 5);
    }

    // --- MultiI2cHandler routing tests ---

    #[test]
    fn handler_set_delay_routes_to_resolved_bus() {
        let bus0 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let bus1 = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus0, InterfaceNumber(0))
            .with_bus(bus1, InterfaceNumber(1))
            .build();

        // SetDelay (bRequest=2) with wIndex=1 should match interface 1 = bus index 0
        let req = vendor_out(2, 500, 1, 0);
        let result = handler.control_out(req, &[]);
        assert!(matches!(result, Some(OutResponse::Accepted)));
        assert_eq!(handler.states[0].delay_us, 500);
    }

    #[test]
    fn handler_get_status_returns_idle_initially() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        let req = vendor_in(3, 0, 0, 1);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data == [0]));
    }

    #[test]
    fn handler_write_sets_ack_status() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        // I2cIo write: bRequest=7 (BEGIN+END), wValue=0 (write), wIndex=0x48 (addr)
        let req = vendor_out(7, 0, 0x48, 2);
        handler.control_out(req, &[0x00, 0xFF]);

        // GetStatus should return AddressAck (1)
        let req = vendor_in(3, 0, 0, 1);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data == [1]));
    }

    #[test]
    fn handler_write_to_missing_addr_sets_nak() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        // Write to address 0x60 which doesn't exist
        let req = vendor_out(7, 0, 0x60, 1);
        handler.control_out(req, &[0x00]);

        // GetStatus should return AddressNak (2)
        let req = vendor_in(3, 0, 0, 1);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data == [2]));
    }

    #[test]
    fn handler_read_returns_device_data() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(
                Address::new(0x48).unwrap(),
                [0xDE, 0xAD, 0xBE, 0xEF],
            ))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        // Set register pointer to 0
        let req = vendor_out(7, 0, 0x48, 1);
        handler.control_out(req, &[0x00]);

        // Read 4 bytes: bRequest=7 (BEGIN+END), wValue=1 (I2C_M_RD), wIndex=0x48
        let req = vendor_in(7, I2C_M_RD, 0x48, 4);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        match result {
            Some(InResponse::Accepted(data)) => {
                assert_eq!(data.len(), 4);
                assert_eq!(data, [0xDE, 0xAD, 0xBE, 0xEF]);
            }
            _ => panic!("Expected Accepted with 4 bytes"),
        }
    }

    #[test]
    fn handler_echo() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        // Echo: bRequest=0, wLength=3
        let req = vendor_in(0, 0, 0, 3);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data.len() == 3));
    }

    #[test]
    fn handler_get_func() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        let req = vendor_in(1, 0, 0, 4);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        let expected = (I2C_FUNC_I2C | I2C_FUNC_SMBUS_EMUL).to_le_bytes();
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data == expected));
    }

    #[test]
    fn handler_non_vendor_request_ignored() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        let req = Request {
            direction: Direction::Out,
            request_type: RequestType::Standard,
            recipient: Recipient::Device,
            request: 0,
            value: 0,
            index: 0,
            length: 0,
        };
        assert!(handler.control_out(req, &[]).is_none());
    }

    #[test]
    fn handler_reset_clears_status() {
        let bus = SimBusBuilder::new()
            .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
            .build();
        let mut handler = MultiI2cHandlerBuilder::new()
            .with_bus(bus, InterfaceNumber(0))
            .build();

        // Trigger a write to set status to ACK
        let req = vendor_out(7, 0, 0x48, 1);
        handler.control_out(req, &[0x00]);

        // Reset should clear status back to Idle
        handler.reset();

        let req = vendor_in(3, 0, 0, 1);
        let mut buf = [0u8; 64];
        let result = handler.control_in(req, &mut buf);
        assert!(matches!(result, Some(InResponse::Accepted(data)) if data == [0]));
    }
}
