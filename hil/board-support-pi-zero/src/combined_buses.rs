//! Combined real and simulated I2C bus set.
//!
//! [`CombinedBusSet`] merges real Linux I2C buses (from `/dev/i2c-*`)
//! with simulated [`RuntimeBus`] instances into a single
//! [`I2cBusSet`](hil_firmware_support::ws_dispatch::I2cBusSet)
//! implementation. Bus indices `0..linux_count` route to real hardware;
//! indices `linux_count..total` route to simulated buses.

use embedded_hal::i2c::I2c;
use i2c_hil_sim::{Address, RuntimeBus};
use hil_firmware_support::ws_dispatch::I2cBusSet;

use crate::linux_i2c::LinuxI2cBus;

/// Maximum devices per simulated bus.
const MAX_DEVICES_PER_BUS: usize = 16;

/// Combines real Linux I2C buses with simulated [`RuntimeBus`] instances.
///
/// Real buses are indexed first, followed by simulated buses:
///
/// | Index range                       | Target           |
/// |-----------------------------------|------------------|
/// | `0 .. linux_count`                | Linux `/dev/i2c` |
/// | `linux_count .. linux_count + N`  | Simulated        |
///
/// Runtime configuration methods (`add_device`, `remove_device`, etc.)
/// only affect the simulated portion. Real buses report zero devices
/// for enumeration but support read/write operations.
pub struct CombinedBusSet {
    linux_buses: Vec<LinuxI2cBus>,
    sim_buses: Vec<RuntimeBus<MAX_DEVICES_PER_BUS>>,
    sim_bus_count: u8,
}

impl CombinedBusSet {
    /// Creates a new combined bus set.
    ///
    /// `sim_capacity` simulated bus slots are pre-allocated. The initial
    /// simulated bus count equals `sim_capacity`; use
    /// [`set_bus_count`](I2cBusSet::set_bus_count) to adjust.
    pub fn new(linux_buses: Vec<LinuxI2cBus>, sim_capacity: u8) -> Self {
        let mut sim_buses = Vec::with_capacity(sim_capacity as usize);
        for _ in 0..sim_capacity {
            sim_buses.push(RuntimeBus::new());
        }
        Self {
            linux_buses,
            sim_buses,
            sim_bus_count: sim_capacity,
        }
    }

    /// Returns the number of real Linux I2C buses.
    #[allow(dead_code)]
    pub fn linux_bus_count(&self) -> u8 {
        self.linux_buses.len() as u8
    }

    /// Returns whether the bus at the given index is a real Linux bus.
    fn is_linux_bus(&self, bus: u8) -> bool {
        (bus as usize) < self.linux_buses.len()
    }

    /// Maps a combined bus index to the simulated bus Vec index.
    ///
    /// Returns `None` if the index falls outside the active simulated
    /// bus range.
    fn sim_index(&self, bus: u8) -> Option<usize> {
        let idx = (bus as usize).checked_sub(self.linux_buses.len())?;
        if idx < self.sim_bus_count as usize {
            Some(idx)
        } else {
            None
        }
    }
}

impl I2cBusSet for CombinedBusSet {
    fn i2c_read(&mut self, bus: u8, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), ()> {
        if self.is_linux_bus(bus) {
            self.linux_buses[bus as usize].i2c_read(addr, reg, buf)
        } else if let Some(idx) = self.sim_index(bus) {
            I2c::write_read(&mut self.sim_buses[idx], addr, &[reg], buf).map_err(|_| ())
        } else {
            Err(())
        }
    }

    fn i2c_write(&mut self, bus: u8, addr: u8, data: &[u8]) -> Result<(), ()> {
        if self.is_linux_bus(bus) {
            self.linux_buses[bus as usize].i2c_write(addr, data)
        } else if let Some(idx) = self.sim_index(bus) {
            I2c::write(&mut self.sim_buses[idx], addr, data).map_err(|_| ())
        } else {
            Err(())
        }
    }

    fn bus_count(&self) -> u8 {
        self.linux_buses.len() as u8 + self.sim_bus_count
    }

    fn device_count(&self, bus: u8) -> u8 {
        if self.is_linux_bus(bus) {
            0
        } else if let Some(idx) = self.sim_index(bus) {
            self.sim_buses[idx].active_count()
        } else {
            0
        }
    }

    fn device_info(&self, bus: u8, index: u8) -> Option<(u8, &[u8])> {
        if self.is_linux_bus(bus) {
            None
        } else {
            let idx = self.sim_index(bus)?;
            self.sim_buses[idx].active_device_info(index)
        }
    }

    fn device_registers(&self, bus: u8, addr: u8) -> Option<&[u8]> {
        if self.is_linux_bus(bus) {
            None
        } else {
            let idx = self.sim_index(bus)?;
            self.sim_buses[idx]
                .device_registers(addr)
                .map(|r| r.as_slice())
        }
    }

    fn add_device(&mut self, bus: u8, addr: u8, name: &[u8], registers: &[u8]) -> Result<(), ()> {
        if self.is_linux_bus(bus) {
            Err(())
        } else if let Some(idx) = self.sim_index(bus) {
            let address = Address::new(addr).ok_or(())?;
            self.sim_buses[idx].add_device(address, name, registers)
        } else {
            Err(())
        }
    }

    fn remove_device(&mut self, bus: u8, addr: u8) -> Result<(), ()> {
        if self.is_linux_bus(bus) {
            Err(())
        } else if let Some(idx) = self.sim_index(bus) {
            let address = Address::new(addr).ok_or(())?;
            self.sim_buses[idx].remove_device(address)
        } else {
            Err(())
        }
    }

    fn set_registers(&mut self, bus: u8, addr: u8, offset: u8, data: &[u8]) -> Result<(), ()> {
        if self.is_linux_bus(bus) {
            Err(())
        } else if let Some(idx) = self.sim_index(bus) {
            let address = Address::new(addr).ok_or(())?;
            self.sim_buses[idx].set_registers(address, offset, data)
        } else {
            Err(())
        }
    }

    fn set_bus_count(&mut self, count: u8) -> Result<(), ()> {
        let linux_count = self.linux_buses.len() as u8;
        let new_sim_count = count.checked_sub(linux_count).ok_or(())?;
        if new_sim_count as usize > self.sim_buses.len() {
            return Err(());
        }
        self.sim_bus_count = new_sim_count;
        Ok(())
    }

    fn clear_all(&mut self) {
        for bus in &mut self.sim_buses {
            bus.clear();
        }
        self.sim_bus_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hil_firmware_support::ws_dispatch::handle_request;

    #[test]
    fn sim_only_bus_count() {
        let buses = CombinedBusSet::new(Vec::new(), 4);
        assert_eq!(buses.bus_count(), 4);
    }

    #[test]
    fn sim_add_device_and_read() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses
            .add_device(0, 0x48, b"TMP1075", &[0xCA, 0xFE])
            .unwrap();
        let mut buf = [0u8; 2];
        buses.i2c_read(0, 0x48, 0, &mut buf).unwrap();
        assert_eq!(buf, [0xCA, 0xFE]);
    }

    #[test]
    fn sim_device_write_then_read() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses.add_device(0, 0x48, b"REG", &[0; 256]).unwrap();
        buses.i2c_write(0, 0x48, &[0x05, 0xAB, 0xCD]).unwrap();
        let mut buf = [0u8; 2];
        buses.i2c_read(0, 0x48, 0x05, &mut buf).unwrap();
        assert_eq!(buf, [0xAB, 0xCD]);
    }

    #[test]
    fn sim_device_info() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses.add_device(0, 0x48, b"TMP1075", &[]).unwrap();
        assert_eq!(buses.device_count(0), 1);
        assert_eq!(buses.device_info(0, 0), Some((0x48, b"TMP1075".as_slice())));
    }

    #[test]
    fn sim_device_registers_query() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses.add_device(0, 0x48, b"X", &[0x42, 0x43]).unwrap();
        let regs = buses.device_registers(0, 0x48).unwrap();
        assert_eq!(regs[0], 0x42);
        assert_eq!(regs[1], 0x43);
    }

    #[test]
    fn set_bus_count_no_linux() {
        let mut buses = CombinedBusSet::new(Vec::new(), 10);
        assert_eq!(buses.bus_count(), 10);
        buses.set_bus_count(5).unwrap();
        assert_eq!(buses.bus_count(), 5);
    }

    #[test]
    fn set_bus_count_exceeds_capacity_fails() {
        let mut buses = CombinedBusSet::new(Vec::new(), 4);
        assert!(buses.set_bus_count(100).is_err());
    }

    #[test]
    fn clear_all_resets_sim() {
        let mut buses = CombinedBusSet::new(Vec::new(), 4);
        buses.add_device(0, 0x48, b"X", &[]).unwrap();
        buses.clear_all();
        assert_eq!(buses.bus_count(), 0);
    }

    #[test]
    fn out_of_range_bus_read_fails() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        assert!(buses.i2c_read(99, 0x48, 0, &mut [0u8; 1]).is_err());
    }

    #[test]
    fn out_of_range_bus_write_fails() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        assert!(buses.i2c_write(99, 0x48, &[0x00, 0x01]).is_err());
    }

    #[test]
    fn add_device_to_invalid_bus_fails() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        assert!(buses.add_device(99, 0x48, b"X", &[]).is_err());
    }

    #[test]
    fn remove_device_from_sim_bus() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses.add_device(0, 0x48, b"X", &[]).unwrap();
        assert_eq!(buses.device_count(0), 1);
        buses.remove_device(0, 0x48).unwrap();
        assert_eq!(buses.device_count(0), 0);
    }

    #[test]
    fn set_registers_on_sim_device() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
        buses.add_device(0, 0x48, b"X", &[0; 256]).unwrap();
        buses.set_registers(0, 0x48, 10, &[0xDE, 0xAD]).unwrap();
        let mut buf = [0u8; 2];
        buses.i2c_read(0, 0x48, 10, &mut buf).unwrap();
        assert_eq!(buf, [0xDE, 0xAD]);
    }

    #[test]
    fn handle_request_list_buses() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);
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

    #[test]
    fn handle_request_add_and_read_device() {
        let mut buses = CombinedBusSet::new(Vec::new(), 2);

        // AddDevice: {0:30, 1:0, 2:0x48, 3:"SIM", 4:h'CAFE'}
        let mut req = [0u8; 64];
        let req_len = {
            let buf_len = req.len();
            let mut writer: &mut [u8] = &mut req;
            let mut enc = minicbor::Encoder::new(&mut writer);
            enc.map(5).unwrap();
            enc.u32(0).unwrap();
            enc.u32(30).unwrap();
            enc.u32(1).unwrap();
            enc.u8(0).unwrap();
            enc.u32(2).unwrap();
            enc.u8(0x48).unwrap();
            enc.u32(3).unwrap();
            enc.str("SIM").unwrap();
            enc.u32(4).unwrap();
            enc.bytes(&[0xCA, 0xFE]).unwrap();
            drop(enc);
            buf_len - writer.len()
        };
        let mut resp = [0u8; 256];
        let n = handle_request(&mut buses, &req[..req_len], &mut resp).unwrap();
        let mut dec = minicbor::Decoder::new(&resp[..n]);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        assert_eq!(dec.u32().unwrap(), 30);

        // I2cRead: {0:1, 1:0, 2:0x48, 3:0, 4:2}
        let req_len = {
            let buf_len = req.len();
            let mut writer: &mut [u8] = &mut req;
            let mut enc = minicbor::Encoder::new(&mut writer);
            enc.map(5).unwrap();
            enc.u32(0).unwrap();
            enc.u32(1).unwrap();
            enc.u32(1).unwrap();
            enc.u8(0).unwrap();
            enc.u32(2).unwrap();
            enc.u8(0x48).unwrap();
            enc.u32(3).unwrap();
            enc.u8(0).unwrap();
            enc.u32(4).unwrap();
            enc.u8(2).unwrap();
            drop(enc);
            buf_len - writer.len()
        };
        let n = handle_request(&mut buses, &req[..req_len], &mut resp).unwrap();
        let mut dec = minicbor::Decoder::new(&resp[..n]);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        assert_eq!(dec.u32().unwrap(), 1); // I2cData tag
        let _k1 = dec.u32().unwrap();
        let data = dec.bytes().unwrap();
        assert_eq!(data, &[0xCA, 0xFE]);
    }
}
