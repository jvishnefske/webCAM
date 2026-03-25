use embedded_hal::i2c::{I2c, Operation};

use i2c_hil_sim::devices::RegisterDevice;
use i2c_hil_sim::{Address, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x48).unwrap()
}

#[test]
fn write_sets_pointer_and_writes_data() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x00; 8]))
        .build();

    bus.write(0x48, &[0x02, 0xAA, 0xBB]).unwrap();

    let regs = bus.devices().0.registers();
    assert_eq!(regs[0], 0x00);
    assert_eq!(regs[1], 0x00);
    assert_eq!(regs[2], 0xAA);
    assert_eq!(regs[3], 0xBB);
    assert_eq!(regs[4], 0x00);
}

#[test]
fn read_returns_data_from_current_pointer() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(
            addr(),
            [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
        ))
        .build();

    // Pointer starts at 0
    let mut buf = [0u8; 3];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x10, 0x20, 0x30]);
}

#[test]
fn write_read_sets_pointer_then_reads() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x10, 0x20, 0x30, 0x40]))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0x30, 0x40]);
}

#[test]
fn pointer_auto_increments_across_reads() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x10, 0x20, 0x30, 0x40]))
        .build();

    // Set pointer to 1
    bus.write(0x48, &[0x01]).unwrap();

    let mut buf1 = [0u8; 1];
    let mut buf2 = [0u8; 1];
    bus.read(0x48, &mut buf1).unwrap();
    bus.read(0x48, &mut buf2).unwrap();

    assert_eq!(buf1, [0x20]);
    assert_eq!(buf2, [0x30]);
}

#[test]
fn pointer_wraps_at_register_boundary() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0xAA, 0xBB, 0xCC, 0xDD]))
        .build();

    // Set pointer to register 3 (last)
    bus.write(0x48, &[0x03]).unwrap();

    // Read 3 bytes: should wrap around
    let mut buf = [0u8; 3];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0xDD, 0xAA, 0xBB]);
}

#[test]
fn empty_write_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0xFF; 4]))
        .build();

    bus.write(0x48, &[]).unwrap();

    let regs = bus.devices().0.registers();
    assert_eq!(*regs, [0xFF; 4]);
}

#[test]
fn empty_read_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0xFF; 4]))
        .build();

    let mut buf = [0u8; 0];
    bus.read(0x48, &mut buf).unwrap();
}

#[test]
fn write_only_register_address_sets_pointer_without_data() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x10, 0x20, 0x30, 0x40]))
        .build();

    // Write just the register address byte, no data
    bus.write(0x48, &[0x02]).unwrap();

    let dev = &bus.devices().0;
    assert_eq!(dev.pointer(), 0x02);
    // Registers unchanged
    assert_eq!(*dev.registers(), [0x10, 0x20, 0x30, 0x40]);
}

#[test]
fn registers_accessor_reflects_writes() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x00; 4]))
        .build();

    bus.write(0x48, &[0x00, 0x11, 0x22, 0x33, 0x44]).unwrap();

    assert_eq!(*bus.devices().0.registers(), [0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn multi_operation_transaction() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(addr(), [0x00; 8]))
        .build();

    let mut read_buf = [0u8; 2];
    let mut ops = [
        // Write 0xAA to register 0x04
        Operation::Write(&[0x04, 0xAA]),
        // Set pointer back to 0x04
        Operation::Write(&[0x04]),
        // Read from 0x04
        Operation::Read(&mut read_buf),
    ];
    bus.transaction(0x48, &mut ops).unwrap();

    assert_eq!(read_buf[0], 0xAA);
}
