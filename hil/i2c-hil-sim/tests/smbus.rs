use embedded_hal::i2c::I2c;

use i2c_hil_sim::smbus::SmBusWordDevice;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

/// Minimal test device implementing SmBusWordDevice with 4 registers.
struct TestWordDevice {
    address: Address,
    pointer: u8,
    registers: [u16; 4],
}

impl TestWordDevice {
    fn new(address: Address) -> Self {
        Self {
            address,
            pointer: 0,
            registers: [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD],
        }
    }
}

impl SmBusWordDevice for TestWordDevice {
    fn address(&self) -> Address {
        self.address
    }

    fn pointer(&self) -> u8 {
        self.pointer
    }

    fn set_pointer(&mut self, ptr: u8) -> Result<(), BusError> {
        if ptr < 4 {
            self.pointer = ptr;
            Ok(())
        } else {
            Err(BusError::DataNak)
        }
    }

    fn read_register(&mut self, ptr: u8) -> u16 {
        self.registers[ptr as usize]
    }

    fn write_register(&mut self, ptr: u8, value: u16) -> Result<(), BusError> {
        self.registers[ptr as usize] = value;
        Ok(())
    }
}

fn addr() -> Address {
    Address::new(0x50).unwrap()
}

#[test]
fn write_one_byte_sets_pointer() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Write 1 byte sets pointer to register 2
    bus.write(0x50, &[0x02]).unwrap();

    // Read should return register 2's value (0xCCCC)
    let mut buf = [0u8; 2];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xCC, 0xCC]);
}

#[test]
fn write_three_bytes_sets_pointer_and_writes_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Write pointer=1, value=0x1234
    bus.write(0x50, &[0x01, 0x12, 0x34]).unwrap();

    // Read back register 1
    let mut buf = [0u8; 2];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0x12, 0x34]);
}

#[test]
fn read_returns_msb_lsb_repeating() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Default pointer is 0, register 0 = 0xAAAA
    let mut buf = [0u8; 6];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
}

#[test]
fn write_read_transaction() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // write_read: set pointer to 3, then read
    let mut buf = [0u8; 2];
    bus.write_read(0x50, &[0x03], &mut buf).unwrap();
    assert_eq!(buf, [0xDD, 0xDD]);
}

#[test]
fn empty_write_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Write register 1 = 0x5678
    bus.write(0x50, &[0x01, 0x56, 0x78]).unwrap();

    // Empty write should not change pointer or register
    use embedded_hal::i2c::Operation;
    bus.transaction(0x50, &mut [Operation::Write(&[])]).unwrap();

    // Pointer should still be 1
    let mut buf = [0u8; 2];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0x56, 0x78]);
}

#[test]
fn invalid_pointer_returns_data_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    let result = bus.write(0x50, &[0x04]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn data_nak_from_set_pointer_propagates() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Attempt write with invalid pointer
    let result = bus.write(0x50, &[0xFF, 0x12, 0x34]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn empty_read_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    use embedded_hal::i2c::Operation;
    let result = bus.transaction(0x50, &mut [Operation::Read(&mut [])]);
    assert!(result.is_ok());
}

#[test]
fn read_single_byte_returns_msb() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Register 0 = 0xAAAA, single byte read returns MSB
    let mut buf = [0u8; 1];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xAA]);
}

#[test]
fn pointer_persists_across_transactions() {
    let mut bus = SimBusBuilder::new()
        .with_device(TestWordDevice::new(addr()))
        .build();

    // Set pointer to 2
    bus.write(0x50, &[0x02]).unwrap();

    // Read without setting pointer — should still be register 2
    let mut buf = [0u8; 2];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xCC, 0xCC]);
}
