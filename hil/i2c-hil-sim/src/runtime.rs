//! Runtime-configurable I2C devices and buses.
//!
//! [`RuntimeDevice`] is a generic register-map I2C device with a 256-byte
//! register space, runtime-assignable address and name, and an active flag
//! for slot management. [`RuntimeBus`] is an array-backed bus holding up
//! to `MAX_DEVICES` runtime-configurable devices and implementing
//! [`embedded_hal::i2c::I2c`].
//!
//! These types enable I2C bus topologies to be configured at runtime over
//! WebSocket, rather than requiring compile-time configuration.

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// Error type for runtime bus device management operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDeviceError {
    /// All device slots on the bus are occupied.
    SlotsFull,
    /// A device with the same address is already active on the bus.
    DuplicateAddress,
    /// No active device was found at the specified address.
    DeviceNotFound,
}

/// Placeholder address for inactive device slots.
const INACTIVE_ADDR: Address = match Address::new(0) {
    Some(a) => a,
    None => panic!("0 is a valid 7-bit address"),
};

/// A runtime-configurable I2C slave device with a 256-byte register map.
///
/// Unlike [`RegisterDevice`](crate::devices::RegisterDevice) which is
/// parameterized by const generics at compile time, `RuntimeDevice` always
/// uses a fixed 256-byte register space and includes a name field for
/// identification. Devices can be activated and deactivated for slot
/// management in [`RuntimeBus`].
///
/// # Protocol
///
/// Same single-byte pointer protocol as `RegisterDevice<256>`:
/// - **Write**: first byte sets the register pointer, subsequent bytes
///   are written to consecutive registers with wrapping.
/// - **Read**: returns bytes starting from the current pointer,
///   auto-incrementing with wrapping after each byte.
#[derive(Clone, Copy)]
pub struct RuntimeDevice {
    address: Address,
    name: [u8; 32],
    name_len: u8,
    registers: [u8; 256],
    pointer: u8,
    active: bool,
}

impl RuntimeDevice {
    /// Creates an inactive device with zeroed state.
    ///
    /// The device occupies no logical slot until activated via
    /// [`init`](Self::init).
    pub const fn empty() -> Self {
        Self {
            address: INACTIVE_ADDR,
            name: [0u8; 32],
            name_len: 0,
            registers: [0u8; 256],
            pointer: 0,
            active: false,
        }
    }

    /// Activates this device slot with the given address, name, and
    /// initial register contents.
    ///
    /// The name is truncated to 32 bytes if longer. Register data is
    /// copied starting at offset 0; bytes beyond index 255 are ignored,
    /// and any trailing registers beyond the provided data are zeroed.
    pub fn init(&mut self, address: Address, name: &[u8], registers: &[u8]) {
        self.address = address;
        self.active = true;
        self.pointer = 0;

        let name_len = if name.len() > 32 { 32 } else { name.len() };
        let mut i = 0;
        while i < name_len {
            self.name[i] = name[i];
            i += 1;
        }
        while i < 32 {
            self.name[i] = 0;
            i += 1;
        }
        self.name_len = name_len as u8;

        let reg_len = if registers.len() > 256 {
            256
        } else {
            registers.len()
        };
        let mut j = 0;
        while j < reg_len {
            self.registers[j] = registers[j];
            j += 1;
        }
        while j < 256 {
            self.registers[j] = 0;
            j += 1;
        }
    }

    /// Deactivates this device slot.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Returns whether this device slot is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the device name as a byte slice.
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    /// Returns a shared reference to the 256-byte register map.
    pub fn registers(&self) -> &[u8; 256] {
        &self.registers
    }

    /// Overwrites registers starting at `offset` with `data`.
    ///
    /// Bytes that would exceed the 256-byte boundary are silently
    /// ignored.
    pub fn set_registers(&mut self, offset: u8, data: &[u8]) {
        let start = offset as usize;
        let mut i = 0;
        while i < data.len() && start + i < 256 {
            self.registers[start + i] = data[i];
            i += 1;
        }
    }
}

impl I2cDevice for RuntimeDevice {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if let Some((&reg_addr, payload)) = data.split_first() {
                        self.pointer = reg_addr;
                        for &byte in payload {
                            self.registers[self.pointer as usize] = byte;
                            self.pointer = self.pointer.wrapping_add(1);
                        }
                    }
                }
                Operation::Read(buf) => {
                    for byte in buf.iter_mut() {
                        *byte = self.registers[self.pointer as usize];
                        self.pointer = self.pointer.wrapping_add(1);
                    }
                }
            }
        }
        Ok(())
    }
}

/// An array-backed I2C bus holding up to `MAX_DEVICES` runtime-configurable
/// devices.
///
/// Implements [`embedded_hal::i2c::I2c`] by scanning active device slots
/// for an address match on each transaction.
///
/// # Memory
///
/// Each device slot occupies approximately 292 bytes. With `MAX_DEVICES = 8`,
/// a single bus uses about 2.3 KB of RAM.
#[derive(Clone, Copy)]
pub struct RuntimeBus<const MAX_DEVICES: usize> {
    devices: [RuntimeDevice; MAX_DEVICES],
}

impl<const MAX_DEVICES: usize> Default for RuntimeBus<MAX_DEVICES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAX_DEVICES: usize> RuntimeBus<MAX_DEVICES> {
    /// Creates an empty bus with all device slots inactive.
    pub const fn new() -> Self {
        Self {
            devices: [RuntimeDevice::empty(); MAX_DEVICES],
        }
    }

    /// Adds a device to the first available inactive slot.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeDeviceError::DuplicateAddress`] if a device with the
    /// same address is already active on this bus, or
    /// [`RuntimeDeviceError::SlotsFull`] if all slots are occupied.
    pub fn add_device(
        &mut self,
        addr: Address,
        name: &[u8],
        registers: &[u8],
    ) -> Result<(), RuntimeDeviceError> {
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() && self.devices[i].address() == addr {
                return Err(RuntimeDeviceError::DuplicateAddress);
            }
            i += 1;
        }
        let mut j = 0;
        while j < MAX_DEVICES {
            if !self.devices[j].is_active() {
                self.devices[j].init(addr, name, registers);
                return Ok(());
            }
            j += 1;
        }
        Err(RuntimeDeviceError::SlotsFull)
    }

    /// Removes the active device at the given address.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeDeviceError::DeviceNotFound`] if no active device
    /// has that address.
    pub fn remove_device(&mut self, addr: Address) -> Result<(), RuntimeDeviceError> {
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() && self.devices[i].address() == addr {
                self.devices[i].deactivate();
                return Ok(());
            }
            i += 1;
        }
        Err(RuntimeDeviceError::DeviceNotFound)
    }

    /// Sets registers on the active device at the given address.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeDeviceError::DeviceNotFound`] if no active device
    /// has that address.
    pub fn set_registers(
        &mut self,
        addr: Address,
        offset: u8,
        data: &[u8],
    ) -> Result<(), RuntimeDeviceError> {
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() && self.devices[i].address() == addr {
                self.devices[i].set_registers(offset, data);
                return Ok(());
            }
            i += 1;
        }
        Err(RuntimeDeviceError::DeviceNotFound)
    }

    /// Returns the number of active devices on this bus.
    pub fn active_count(&self) -> u8 {
        let mut count = 0u8;
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() {
                count += 1;
            }
            i += 1;
        }
        count
    }

    /// Returns `(address, name)` for the `index`-th active device.
    ///
    /// Devices are enumerated in slot order, skipping inactive slots.
    /// Returns `None` if `index` is out of range.
    pub fn active_device_info(&self, index: u8) -> Option<(u8, &[u8])> {
        let mut count = 0u8;
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() {
                if count == index {
                    return Some((self.devices[i].address().raw(), self.devices[i].name()));
                }
                count += 1;
            }
            i += 1;
        }
        None
    }

    /// Returns the register map for the active device at `addr`.
    ///
    /// Returns `None` if no active device has that address.
    pub fn device_registers(&self, addr: u8) -> Option<&[u8; 256]> {
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() && self.devices[i].address().raw() == addr {
                return Some(self.devices[i].registers());
            }
            i += 1;
        }
        None
    }

    /// Returns a reference to the device at the given slot index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn device(&self, index: usize) -> Option<&RuntimeDevice> {
        if index < MAX_DEVICES {
            Some(&self.devices[index])
        } else {
            None
        }
    }

    /// Deactivates all devices on this bus.
    pub fn clear(&mut self) {
        let mut i = 0;
        while i < MAX_DEVICES {
            self.devices[i].deactivate();
            i += 1;
        }
    }
}

impl<const MAX_DEVICES: usize> ErrorType for RuntimeBus<MAX_DEVICES> {
    type Error = BusError;
}

impl<const MAX_DEVICES: usize> I2c for RuntimeBus<MAX_DEVICES> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut i = 0;
        while i < MAX_DEVICES {
            if self.devices[i].is_active() && self.devices[i].address().raw() == address {
                return self.devices[i].process(operations);
            }
            i += 1;
        }
        Err(BusError::NoDeviceAtAddress(address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_device_is_inactive() {
        let dev = RuntimeDevice::empty();
        assert!(!dev.is_active());
    }

    #[test]
    fn init_activates_device() {
        let mut dev = RuntimeDevice::empty();
        let addr = Address::new(0x48).unwrap();
        dev.init(addr, b"TMP1075", &[0xAA; 4]);
        assert!(dev.is_active());
        assert_eq!(dev.address(), addr);
        assert_eq!(dev.name(), b"TMP1075");
        assert_eq!(dev.registers()[0], 0xAA);
        assert_eq!(dev.registers()[3], 0xAA);
        assert_eq!(dev.registers()[4], 0);
    }

    #[test]
    fn deactivate_clears_active() {
        let mut dev = RuntimeDevice::empty();
        dev.init(Address::new(0x48).unwrap(), b"X", &[]);
        dev.deactivate();
        assert!(!dev.is_active());
    }

    #[test]
    fn set_registers_writes_at_offset() {
        let mut dev = RuntimeDevice::empty();
        dev.init(Address::new(0x48).unwrap(), b"X", &[0; 256]);
        dev.set_registers(10, &[0xDE, 0xAD]);
        assert_eq!(dev.registers()[10], 0xDE);
        assert_eq!(dev.registers()[11], 0xAD);
        assert_eq!(dev.registers()[12], 0x00);
    }

    #[test]
    fn set_registers_truncates_at_boundary() {
        let mut dev = RuntimeDevice::empty();
        dev.init(Address::new(0x48).unwrap(), b"X", &[0; 256]);
        dev.set_registers(254, &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(dev.registers()[254], 0x01);
        assert_eq!(dev.registers()[255], 0x02);
        assert_eq!(dev.registers()[0], 0x00);
    }

    #[test]
    fn name_truncated_to_32() {
        let mut dev = RuntimeDevice::empty();
        let long_name = [b'A'; 40];
        dev.init(Address::new(0x48).unwrap(), &long_name, &[]);
        assert_eq!(dev.name().len(), 32);
    }

    #[test]
    fn runtime_device_i2c_write_read() {
        let mut dev = RuntimeDevice::empty();
        dev.init(Address::new(0x48).unwrap(), b"test", &[0x10, 0x20, 0x30]);

        let mut read_buf = [0u8; 3];
        dev.process(&mut [Operation::Write(&[0x00]), Operation::Read(&mut read_buf)])
            .unwrap();
        assert_eq!(read_buf, [0x10, 0x20, 0x30]);
    }

    #[test]
    fn runtime_bus_add_remove_device() {
        let mut bus = RuntimeBus::<4>::new();
        assert_eq!(bus.active_count(), 0);

        let addr = Address::new(0x48).unwrap();
        bus.add_device(addr, b"TMP1075", &[0; 256]).unwrap();
        assert_eq!(bus.active_count(), 1);

        bus.remove_device(addr).unwrap();
        assert_eq!(bus.active_count(), 0);
    }

    #[test]
    fn runtime_bus_rejects_duplicate_address() {
        let mut bus = RuntimeBus::<4>::new();
        let addr = Address::new(0x48).unwrap();
        bus.add_device(addr, b"A", &[]).unwrap();
        assert!(bus.add_device(addr, b"B", &[]).is_err());
    }

    #[test]
    fn runtime_bus_full() {
        let mut bus = RuntimeBus::<2>::new();
        bus.add_device(Address::new(0x10).unwrap(), b"A", &[])
            .unwrap();
        bus.add_device(Address::new(0x20).unwrap(), b"B", &[])
            .unwrap();
        assert!(bus
            .add_device(Address::new(0x30).unwrap(), b"C", &[])
            .is_err());
    }

    #[test]
    fn runtime_bus_i2c_transaction() {
        let mut bus = RuntimeBus::<4>::new();
        let mut regs = [0u8; 256];
        regs[0] = 0xCA;
        regs[1] = 0xFE;
        bus.add_device(Address::new(0x48).unwrap(), b"test", &regs)
            .unwrap();

        let mut read_buf = [0u8; 2];
        bus.write_read(0x48, &[0x00], &mut read_buf).unwrap();
        assert_eq!(read_buf, [0xCA, 0xFE]);
    }

    #[test]
    fn runtime_bus_no_device_at_address() {
        let mut bus = RuntimeBus::<4>::new();
        let mut buf = [0u8; 1];
        let err = bus.write_read(0x48, &[0x00], &mut buf).unwrap_err();
        assert_eq!(err, BusError::NoDeviceAtAddress(0x48));
    }

    #[test]
    fn runtime_bus_set_registers() {
        let mut bus = RuntimeBus::<4>::new();
        let addr = Address::new(0x48).unwrap();
        bus.add_device(addr, b"test", &[0; 256]).unwrap();
        bus.set_registers(addr, 5, &[0xAB, 0xCD]).unwrap();

        let mut read_buf = [0u8; 2];
        bus.write_read(0x48, &[5], &mut read_buf).unwrap();
        assert_eq!(read_buf, [0xAB, 0xCD]);
    }

    #[test]
    fn runtime_bus_clear() {
        let mut bus = RuntimeBus::<4>::new();
        bus.add_device(Address::new(0x10).unwrap(), b"A", &[])
            .unwrap();
        bus.add_device(Address::new(0x20).unwrap(), b"B", &[])
            .unwrap();
        assert_eq!(bus.active_count(), 2);
        bus.clear();
        assert_eq!(bus.active_count(), 0);
    }

    #[test]
    fn active_device_info_returns_active_devices() {
        let mut bus = RuntimeBus::<4>::new();
        bus.add_device(Address::new(0x48).unwrap(), b"TMP1075", &[])
            .unwrap();
        bus.add_device(Address::new(0x50).unwrap(), b"EEPROM", &[])
            .unwrap();

        assert_eq!(
            bus.active_device_info(0),
            Some((0x48, b"TMP1075".as_slice()))
        );
        assert_eq!(
            bus.active_device_info(1),
            Some((0x50, b"EEPROM".as_slice()))
        );
        assert_eq!(bus.active_device_info(2), None);
    }

    #[test]
    fn device_registers_by_address() {
        let mut bus = RuntimeBus::<4>::new();
        let mut regs = [0u8; 256];
        regs[0] = 0x42;
        bus.add_device(Address::new(0x48).unwrap(), b"X", &regs)
            .unwrap();

        let found = bus.device_registers(0x48).unwrap();
        assert_eq!(found[0], 0x42);
        assert!(bus.device_registers(0x99).is_none());
    }

    #[test]
    fn remove_nonexistent_device_fails() {
        let mut bus = RuntimeBus::<4>::new();
        assert!(bus.remove_device(Address::new(0x48).unwrap()).is_err());
    }

    #[test]
    fn runtime_bus_device_accessor() {
        let mut bus = RuntimeBus::<4>::new();
        bus.add_device(Address::new(0x48).unwrap(), b"X", &[0xAA; 4])
            .unwrap();

        let dev = bus.device(0).unwrap();
        assert!(dev.is_active());
        assert_eq!(dev.name(), b"X");
        assert_eq!(dev.registers()[0], 0xAA);

        assert!(bus.device(99).is_none());
    }

    #[test]
    fn runtime_bus_default_trait() {
        let bus = RuntimeBus::<4>::default();
        assert_eq!(bus.active_count(), 0);
    }

    #[test]
    fn set_registers_nonexistent_device_fails() {
        let mut bus = RuntimeBus::<4>::new();
        assert!(bus
            .set_registers(Address::new(0x48).unwrap(), 0, &[1])
            .is_err());
    }
}
