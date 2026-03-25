use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::RegisterDevice;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

#[test]
fn empty_bus_naks_all_addresses() {
    let mut bus = SimBusBuilder::new().build();

    let mut buf = [0u8; 1];
    let result = bus.read(0x00, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x00)));

    let result = bus.read(0x7F, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x7F)));
}

#[test]
fn single_device_bus() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xDE; 4]))
        .build();

    let mut buf = [0u8; 1];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xDE]);
}

#[test]
fn multi_device_bus() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .with_device(RegisterDevice::new(Address::new(0x68).unwrap(), [0xBB; 4]))
        .with_device(RegisterDevice::new(Address::new(0x76).unwrap(), [0xCC; 4]))
        .build();

    let mut buf = [0u8; 1];

    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0xAA]);

    bus.read(0x68, &mut buf).unwrap();
    assert_eq!(buf, [0xBB]);

    bus.read(0x76, &mut buf).unwrap();
    assert_eq!(buf, [0xCC]);
}

#[test]
#[should_panic(expected = "duplicate I2C address 0x48 on bus")]
fn duplicate_address_panics() {
    let _ = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0x00; 4]))
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0x00; 4]))
        .build();
}

#[test]
fn multiple_independent_busses() {
    let mut bus0 = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .build();

    let mut bus1 = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xBB; 4]))
        .build();

    let mut buf0 = [0u8; 1];
    let mut buf1 = [0u8; 1];

    bus0.read(0x48, &mut buf0).unwrap();
    bus1.read(0x48, &mut buf1).unwrap();

    // Same address, different busses, different data
    assert_eq!(buf0, [0xAA]);
    assert_eq!(buf1, [0xBB]);
}

#[test]
fn default_builder() {
    let mut bus = SimBusBuilder::default().build();

    let mut buf = [0u8; 1];
    let result = bus.read(0x10, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x10)));
}
