use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::{I2cSwitchBuilder, RegisterDevice, Tmp1075};
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

#[test]
fn switch_and_device_coexist_on_bus() {
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xAA; 4]))
        .with_switch(switch)
        .build();

    // Access the regular device
    let mut buf = [0u8; 1];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0xAA);

    // Access the switch's control register
    let mut ctrl = [0u8; 1];
    bus.read(0x70, &mut ctrl).unwrap();
    assert_eq!(ctrl[0], 0x00);
}

#[test]
fn switch_routes_through_channels_on_bus() {
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::with_temperature(
            Address::new(0x48).unwrap(),
            Tmp1075::celsius_to_raw(25.0),
        ))
        .channel(Tmp1075::with_temperature(
            Address::new(0x49).unwrap(),
            Tmp1075::celsius_to_raw(50.0),
        ))
        .build();

    let mut bus = SimBusBuilder::new().with_switch(switch).build();

    // Enable channel 0
    bus.write(0x70, &[0x01]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);

    // Switch to channel 1
    bus.write(0x70, &[0x02]).unwrap();
    bus.write_read(0x49, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn device_accessible_when_switch_channels_disabled() {
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xBB; 4]))
        .with_switch(switch)
        .build();

    // Switch channels disabled (default), regular device still works
    let mut buf = [0u8; 1];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0xBB);

    // Downstream device not reachable
    let mut buf2 = [0u8; 2];
    let result = bus.read(0x48, &mut buf2);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x48)));
}

#[test]
fn switch_channel_device_and_bus_device_at_different_addresses() {
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(RegisterDevice::new(Address::new(0x48).unwrap(), [0xCC; 4]))
        .build();

    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xDD; 4]))
        .with_switch(switch)
        .build();

    // Enable switch channel 0
    bus.write(0x70, &[0x01]).unwrap();

    // Access switch downstream device
    let mut buf = [0u8; 1];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf[0], 0xCC);

    // Access bus-level device
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0xDD);
}

#[test]
fn switch_downstream_falls_through_to_bus() {
    // When a switch channel is enabled but doesn't have the requested address,
    // the transaction should fall through to other devices on the bus.
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .build();

    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xBB; 4]))
        .with_switch(switch)
        .build();

    // Enable switch channel
    bus.write(0x70, &[0x01]).unwrap();

    // 0x50 not on switch channel, should fall through to bus device
    let mut buf = [0u8; 1];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0xBB);
}

#[test]
#[should_panic(expected = "duplicate I2C address 0x70 on bus")]
fn duplicate_switch_address_panics() {
    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    let _bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x70).unwrap(), [0u8; 4]))
        .with_switch(switch)
        .build();
}
