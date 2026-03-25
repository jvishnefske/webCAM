//! Shared runtime I2C buses accessible from both USB (i2c-tiny-usb) and WebSocket.
//!
//! A single `SharedState` holds 10 `RuntimeBus` instances protected by a
//! `BlockingMutex`. Two thin wrappers provide typed access:
//!
//! - [`UsbI2cHandler`] implements `embassy_usb::Handler` for USB control transfers.
//! - [`WsBusAccess`] implements `I2cBusSet` for WebSocket dispatch.
//!
//! Both wrappers hold `&'static` references to the same mutex, ensuring
//! mutual exclusion via `critical_section` on single-core Cortex-M.

use core::cell::RefCell;

use defmt::{debug, info, warn};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::Handler;
use embedded_hal::i2c::I2c;
use i2c_hil_sim::RuntimeBus;

use hil_firmware_support::ws_dispatch::I2cBusSet;
use usb_composite_dispatchers::i2c_tiny_usb::{
    Command, InterfaceState, Status, I2C_M_RD,
};

/// Maximum buses exposed over USB.
const MAX_BUSES: usize = 10;

/// Maximum devices per bus.
const MAX_DEVS: usize = 8;

/// Shared state: runtime buses + USB per-interface metadata.
pub struct SharedState {
    pub buses: [RuntimeBus<MAX_DEVS>; MAX_BUSES],
    pub bus_count: u8,
    usb_states: [InterfaceState; MAX_BUSES],
    if_nums: [u8; MAX_BUSES],
}

impl SharedState {
    pub const fn new() -> Self {
        Self {
            buses: [RuntimeBus::new(); MAX_BUSES],
            bus_count: 0,
            usb_states: [const { InterfaceState::new() }; MAX_BUSES],
            if_nums: [0; MAX_BUSES],
        }
    }

    /// Register a USB interface number for a given bus index.
    pub fn set_interface(&mut self, bus: usize, if_num: u8) {
        if bus < MAX_BUSES {
            self.if_nums[bus] = if_num;
        }
    }

    fn resolve_interface(&self, req: &Request) -> usize {
        let raw = req.index as u8;
        let mut i = 0;
        while i < self.bus_count as usize {
            if self.if_nums[i] == raw {
                return i;
            }
            i += 1;
        }
        0
    }
}

/// Type alias for the shared mutex.
pub type SharedBusMutex = BlockingMutex<NoopRawMutex, RefCell<SharedState>>;

// ── USB Handler ────────────────────────────────────────────────────

/// USB control transfer handler for i2c-tiny-usb protocol.
/// Holds a `&'static` reference to the shared bus mutex.
pub struct UsbI2cHandler {
    shared: &'static SharedBusMutex,
}

impl UsbI2cHandler {
    pub fn new(shared: &'static SharedBusMutex) -> Self {
        Self { shared }
    }
}

impl Handler for UsbI2cHandler {
    fn enabled(&mut self, enabled: bool) {
        info!("USB enabled: {}", enabled);
    }

    fn reset(&mut self) {
        info!("USB reset");
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let count = state.bus_count as usize;
            let mut i = 0;
            while i < count {
                state.usb_states[i].status = Status::Idle;
                i += 1;
            }
        });
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

        let cmd = Command::from_request(req.request)?;

        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let bus = state.resolve_interface(&req);

            debug!(
                "USB OUT cmd={} bus={} wValue={} wIndex={} len={}",
                cmd, bus, req.value, req.index, data.len(),
            );

            match cmd {
                Command::SetDelay => {
                    state.usb_states[bus].delay_us = req.value;
                    Some(OutResponse::Accepted)
                }
                Command::I2cIo { begin: _, end: _ } if req.value & I2C_M_RD == 0 => {
                    let addr = (req.index & 0x7F) as u8;
                    let ok = I2c::write(&mut state.buses[bus], addr, data).is_ok();
                    if ok {
                        state.usb_states[bus].status = Status::AddressAck;
                    } else {
                        state.usb_states[bus].status = Status::AddressNak;
                        warn!("I2C{} write NAK addr=0x{:02x}", bus, addr);
                    }
                    Some(OutResponse::Accepted)
                }
                _ => None,
            }
        })
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        if req.request_type != RequestType::Vendor {
            return None;
        }

        let cmd = Command::from_request(req.request)?;

        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let bus = state.resolve_interface(&req);

            debug!(
                "USB IN cmd={} bus={} wValue={} wIndex={} wLength={}",
                cmd, bus, req.value, req.index, req.length,
            );

            match cmd {
                Command::Echo => {
                    let len = (req.length as usize).min(buf.len());
                    Some(InResponse::Accepted(&buf[..len]))
                }
                Command::GetFunc => {
                    let func = state.usb_states[bus].functionality.to_le_bytes();
                    buf[..4].copy_from_slice(&func);
                    Some(InResponse::Accepted(&buf[..4]))
                }
                Command::GetStatus => {
                    buf[0] = state.usb_states[bus].status as u8;
                    Some(InResponse::Accepted(&buf[..1]))
                }
                Command::I2cIo { begin: _, end: _ } if req.value & I2C_M_RD != 0 => {
                    let addr = (req.index & 0x7F) as u8;
                    let len = (req.length as usize).min(buf.len());
                    let ok = I2c::read(&mut state.buses[bus], addr, &mut buf[..len]).is_ok();
                    if ok {
                        state.usb_states[bus].status = Status::AddressAck;
                        Some(InResponse::Accepted(&buf[..len]))
                    } else {
                        state.usb_states[bus].status = Status::AddressNak;
                        warn!("I2C{} read NAK addr=0x{:02x}", bus, addr);
                        Some(InResponse::Accepted(&[]))
                    }
                }
                _ => None,
            }
        })
    }
}

// ── WebSocket Bus Access ───────────────────────────────────────────

/// WebSocket-side access to the shared buses.
/// Implements `I2cBusSet` for use with `hil_firmware_support::ws_server::run`.
pub struct WsBusAccess {
    shared: &'static SharedBusMutex,
}

impl WsBusAccess {
    pub fn new(shared: &'static SharedBusMutex) -> Self {
        Self { shared }
    }
}

impl I2cBusSet for WsBusAccess {
    fn i2c_read(&mut self, bus: u8, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return Err(());
            }
            I2c::write_read(&mut state.buses[idx], addr, &[reg], buf).map_err(|_| ())
        })
    }

    fn i2c_write(&mut self, bus: u8, addr: u8, data: &[u8]) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return Err(());
            }
            I2c::write(&mut state.buses[idx], addr, data).map_err(|_| ())
        })
    }

    fn bus_count(&self) -> u8 {
        self.shared.lock(|inner| inner.borrow().bus_count)
    }

    fn device_count(&self, bus: u8) -> u8 {
        self.shared.lock(|inner| {
            let state = inner.borrow();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return 0;
            }
            state.buses[idx].active_count()
        })
    }

    fn device_info(&self, bus: u8, index: u8) -> Option<(u8, &[u8])> {
        // SAFETY: The returned slice borrows from the static SharedBusMutex.
        // Since the RuntimeBus is in a StaticCell, the data won't move.
        // We must use a raw pointer to escape the borrow checker since
        // BlockingMutex::lock doesn't allow returning borrows.
        self.shared.lock(|inner| {
            let state = inner.borrow();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return None;
            }
            // Get the device info - returns references into the static bus
            let info = state.buses[idx].active_device_info(index);
            info.map(|(addr, name)| {
                // SAFETY: The name slice lives in the static SharedState allocation.
                // It remains valid as long as the device is not removed. This is safe
                // because we only use this in encode_bus_list_dynamic which consumes
                // the data immediately within the same call.
                let name_ptr = name.as_ptr();
                let name_len = name.len();
                (addr, unsafe { core::slice::from_raw_parts(name_ptr, name_len) })
            })
        })
    }

    fn device_registers(&self, bus: u8, addr: u8) -> Option<&[u8]> {
        self.shared.lock(|inner| {
            let state = inner.borrow();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return None;
            }
            state.buses[idx].device_registers(addr).map(|r| {
                let ptr = r.as_ptr();
                let len = r.len();
                unsafe { core::slice::from_raw_parts(ptr, len) }
            })
        })
    }

    fn add_device(
        &mut self,
        bus: u8,
        addr: u8,
        name: &[u8],
        registers: &[u8],
    ) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return Err(());
            }
            let address = i2c_hil_sim::Address::new(addr).ok_or(())?;
            state.buses[idx].add_device(address, name, registers)
        })
    }

    fn remove_device(&mut self, bus: u8, addr: u8) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return Err(());
            }
            let address = i2c_hil_sim::Address::new(addr).ok_or(())?;
            state.buses[idx].remove_device(address)
        })
    }

    fn set_registers(&mut self, bus: u8, addr: u8, offset: u8, data: &[u8]) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let idx = bus as usize;
            if idx >= state.bus_count as usize {
                return Err(());
            }
            let address = i2c_hil_sim::Address::new(addr).ok_or(())?;
            state.buses[idx].set_registers(address, offset, data)
        })
    }

    fn set_bus_count(&mut self, count: u8) -> Result<(), ()> {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            if count as usize > MAX_BUSES {
                return Err(());
            }
            state.bus_count = count;
            Ok(())
        })
    }

    fn clear_all(&mut self) {
        self.shared.lock(|inner| {
            let mut state = inner.borrow_mut();
            let mut i = 0;
            while i < MAX_BUSES {
                state.buses[i].clear();
                i += 1;
            }
            state.bus_count = 0;
        });
    }
}
