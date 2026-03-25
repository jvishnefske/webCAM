use embedded_hal::i2c::{ErrorKind, I2c, Operation};

use i2c_hil_sim::devices::RegisterDevice;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

#[test]
fn read_routes_to_correct_device() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .build();

    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0xAA]);
}

#[test]
fn write_routes_to_correct_device() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 8]))
        .build();

    // Write register 0x02 = 0xFF
    bus.write(0x50, &[0x02, 0xFF]).unwrap();

    let regs = bus.devices().0.registers();
    assert_eq!(regs[2], 0xFF);
}

#[test]
fn missing_address_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
        .build();

    let mut buf = [0u8; 1];
    let result = bus.read(0x50, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x50)));
}

#[test]
fn missing_address_error_kind_is_no_acknowledge() {
    let mut bus = SimBusBuilder::new().build();

    let mut buf = [0u8; 1];
    let err = bus.read(0x10, &mut buf).unwrap_err();
    assert_eq!(
        embedded_hal::i2c::Error::kind(&err),
        ErrorKind::NoAcknowledge(embedded_hal::i2c::NoAcknowledgeSource::Address)
    );
}

#[test]
fn multiple_devices_each_receive_own_transactions() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .with_device(RegisterDevice::new(Address::new(0x68).unwrap(), [0xBB; 4]))
        .build();

    let mut buf_a = [0u8; 2];
    let mut buf_b = [0u8; 2];

    bus.read(0x48, &mut buf_a).unwrap();
    bus.read(0x68, &mut buf_b).unwrap();

    assert_eq!(buf_a, [0xAA, 0xAA]);
    assert_eq!(buf_b, [0xBB, 0xBB]);
}

#[test]
fn write_read_routes_to_correct_device() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(
            Address::new(0x48).unwrap(),
            [0x10, 0x20, 0x30, 0x40],
        ))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x01], &mut buf).unwrap();
    assert_eq!(buf, [0x20, 0x30]);
}

#[test]
fn transaction_with_multiple_operations() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 8]))
        .build();

    let mut read_buf = [0u8; 2];
    let mut ops = [
        Operation::Write(&[0x03, 0xAA, 0xBB]),
        Operation::Write(&[0x03]),
        Operation::Read(&mut read_buf),
    ];
    bus.transaction(0x50, &mut ops).unwrap();

    assert_eq!(read_buf, [0xAA, 0xBB]);
}
