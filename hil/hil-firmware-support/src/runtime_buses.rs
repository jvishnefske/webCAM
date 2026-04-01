//! Runtime-configurable I2C bus set for WebSocket requests.
//!
//! Defines [`RuntimeBusSet`] which holds an array of
//! [`RuntimeBus`](i2c_hil_sim::RuntimeBus) instances and implements
//! [`I2cBusSet`](crate::ws_dispatch::I2cBusSet) to wire them into
//! the generic WebSocket dispatch loop.
//!
//! Unlike compile-time `AllBuses`, this allows the host to
//! configure bus topology at runtime over WebSocket before testing.

use embedded_hal::i2c::I2c;
use i2c_hil_sim::{Address, RuntimeBus};

use crate::ws_dispatch::I2cBusSet;

/// Runtime-configurable set of I2C buses for WebSocket dispatch.
///
/// Wraps a fixed-size array of [`RuntimeBus`] instances. The host
/// configures the topology at runtime by sending WebSocket commands
/// to set the bus count, add devices, and populate registers.
///
/// # Memory
///
/// With `MAX_BUSES = 10` and `MAX_DEVICES = 8`, total RAM usage is
/// approximately 23 KB (10 × 8 × 292 bytes per device slot).
pub struct RuntimeBusSet<const MAX_BUSES: usize, const MAX_DEVICES: usize> {
    buses: [RuntimeBus<MAX_DEVICES>; MAX_BUSES],
    bus_count: u8,
}

impl<const MAX_BUSES: usize, const MAX_DEVICES: usize> Default
    for RuntimeBusSet<MAX_BUSES, MAX_DEVICES>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAX_BUSES: usize, const MAX_DEVICES: usize> RuntimeBusSet<MAX_BUSES, MAX_DEVICES> {
    /// Creates an empty bus set with no active buses.
    ///
    /// The host must call `set_bus_count` and `add_device` via WebSocket
    /// before any I2C operations will succeed.
    pub const fn new() -> Self {
        Self {
            buses: [RuntimeBus::new(); MAX_BUSES],
            bus_count: 0,
        }
    }
}

impl<const MAX_BUSES: usize, const MAX_DEVICES: usize> I2cBusSet
    for RuntimeBusSet<MAX_BUSES, MAX_DEVICES>
{
    fn i2c_read(&mut self, bus: u8, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), ()> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return Err(());
        }
        I2c::write_read(&mut self.buses[idx], addr, &[reg], buf).map_err(|_| ())
    }

    fn i2c_write(&mut self, bus: u8, addr: u8, data: &[u8]) -> Result<(), ()> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return Err(());
        }
        I2c::write(&mut self.buses[idx], addr, data).map_err(|_| ())
    }

    fn bus_count(&self) -> u8 {
        self.bus_count
    }

    fn device_count(&self, bus: u8) -> u8 {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return 0;
        }
        self.buses[idx].active_count()
    }

    fn device_info(&self, bus: u8, index: u8) -> Option<(u8, &[u8])> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return None;
        }
        self.buses[idx].active_device_info(index)
    }

    fn device_registers(&self, bus: u8, addr: u8) -> Option<&[u8]> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return None;
        }
        self.buses[idx].device_registers(addr).map(|r| r.as_slice())
    }

    fn add_device(&mut self, bus: u8, addr: u8, name: &[u8], registers: &[u8]) -> Result<(), ()> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return Err(());
        }
        let address = Address::new(addr).ok_or(())?;
        self.buses[idx].add_device(address, name, registers)
    }

    fn remove_device(&mut self, bus: u8, addr: u8) -> Result<(), ()> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return Err(());
        }
        let address = Address::new(addr).ok_or(())?;
        self.buses[idx].remove_device(address)
    }

    fn set_registers(&mut self, bus: u8, addr: u8, offset: u8, data: &[u8]) -> Result<(), ()> {
        let idx = bus as usize;
        if idx >= self.bus_count as usize {
            return Err(());
        }
        let address = Address::new(addr).ok_or(())?;
        self.buses[idx].set_registers(address, offset, data)
    }

    fn set_bus_count(&mut self, count: u8) -> Result<(), ()> {
        if count as usize > MAX_BUSES {
            return Err(());
        }
        self.bus_count = count;
        Ok(())
    }

    fn clear_all(&mut self) {
        let mut i = 0;
        while i < MAX_BUSES {
            self.buses[i].clear();
            i += 1;
        }
        self.bus_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws_dispatch::handle_request;

    #[test]
    fn runtime_bus_set_add_device_and_read() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(2).unwrap();
        buses
            .add_device(0, 0x48, b"TMP1075", &[0xCA, 0xFE])
            .unwrap();
        let mut buf = [0u8; 2];
        buses.i2c_read(0, 0x48, 0, &mut buf).unwrap();
        assert_eq!(buf, [0xCA, 0xFE]);
    }

    #[test]
    fn runtime_bus_set_write_then_read() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(2).unwrap();
        buses.add_device(0, 0x48, b"REG", &[0; 256]).unwrap();
        buses.i2c_write(0, 0x48, &[0x05, 0xAB, 0xCD]).unwrap();
        let mut buf = [0u8; 2];
        buses.i2c_read(0, 0x48, 0x05, &mut buf).unwrap();
        assert_eq!(buf, [0xAB, 0xCD]);
    }

    #[test]
    fn runtime_bus_set_device_info() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(2).unwrap();
        buses.add_device(0, 0x48, b"TMP1075", &[]).unwrap();
        assert_eq!(buses.device_count(0), 1);
        assert_eq!(buses.device_info(0, 0), Some((0x48, b"TMP1075".as_slice())));
    }

    #[test]
    fn runtime_bus_set_bus_count_operations() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        assert_eq!(buses.bus_count(), 0);
        buses.set_bus_count(3).unwrap();
        assert_eq!(buses.bus_count(), 3);
        assert!(buses.set_bus_count(5).is_err()); // exceeds MAX_BUSES=4
    }

    #[test]
    fn runtime_bus_set_clear_all() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(2).unwrap();
        buses.add_device(0, 0x48, b"X", &[]).unwrap();
        buses.clear_all();
        assert_eq!(buses.bus_count(), 0);
    }

    #[test]
    fn runtime_bus_set_out_of_range() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(1).unwrap();
        assert!(buses.i2c_read(99, 0x48, 0, &mut [0u8; 1]).is_err());
        assert!(buses.i2c_write(99, 0x48, &[0x00, 0x01]).is_err());
    }

    #[test]
    fn runtime_bus_set_remove_device() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(1).unwrap();
        buses.add_device(0, 0x48, b"TMP", &[]).unwrap();
        assert_eq!(buses.device_count(0), 1);
        buses.remove_device(0, 0x48).unwrap();
        assert_eq!(buses.device_count(0), 0);
    }

    #[test]
    fn runtime_bus_set_set_and_read_registers() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(1).unwrap();
        buses.add_device(0, 0x48, b"REG", &[0; 256]).unwrap();
        buses.set_registers(0, 0x48, 5, &[0xAB, 0xCD]).unwrap();
        let regs = buses.device_registers(0, 0x48).unwrap();
        assert_eq!(regs[5], 0xAB);
        assert_eq!(regs[6], 0xCD);
    }

    #[test]
    fn runtime_bus_set_device_registers_out_of_range() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(1).unwrap();
        assert!(buses.device_registers(0, 0x99).is_none());
        assert!(buses.device_registers(99, 0x48).is_none());
    }

    #[test]
    fn runtime_bus_set_handle_request_list_buses() {
        let mut buses = RuntimeBusSet::<4, 8>::new();
        buses.set_bus_count(2).unwrap();
        buses.add_device(0, 0x48, b"TMP1075", &[]).unwrap();

        let mut req = [0u8; 16];
        let req_len = {
            let buf_len = req.len();
            let mut writer: &mut [u8] = &mut req;
            let mut enc = minicbor::Encoder::new(&mut writer);
            enc.map(1).unwrap();
            enc.u32(0).unwrap();
            enc.u32(3).unwrap();
            drop(enc);
            buf_len - writer.len()
        };

        let mut resp = [0u8; 1024];
        let n = handle_request(&mut buses, &req[..req_len], &mut resp).unwrap();

        let mut dec = minicbor::Decoder::new(&resp[..n]);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        let tag = dec.u32().unwrap();
        assert_eq!(tag, 3);
    }
}
