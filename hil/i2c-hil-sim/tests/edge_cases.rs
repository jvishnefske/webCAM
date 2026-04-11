use embedded_hal::i2c::{ErrorKind, I2c, NoAcknowledgeSource, Operation};

use i2c_hil_sim::devices::RegisterDevice;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

#[test]
fn address_max_valid() {
    assert!(Address::new(0x7F).is_some());
}

#[test]
fn address_min_valid() {
    assert!(Address::new(0x00).is_some());
}

#[test]
fn address_just_over_max_is_none() {
    assert!(Address::new(0x80).is_none());
}

#[test]
fn address_max_u8_is_none() {
    assert!(Address::new(0xFF).is_none());
}

#[test]
fn address_raw_roundtrip() {
    let addr = Address::new(0x42).unwrap();
    assert_eq!(addr.raw(), 0x42);
}

#[test]
fn bus_error_display_no_device() {
    let err = BusError::NoDeviceAtAddress(0x48);
    let msg = format!("{err}");
    assert_eq!(msg, "no device at address 0x48");
}

#[test]
fn bus_error_display_data_nak() {
    let msg = format!("{}", BusError::DataNak);
    assert_eq!(msg, "data not acknowledged");
}

#[test]
fn bus_error_display_device_error() {
    let msg = format!("{}", BusError::DeviceError);
    assert_eq!(msg, "device processing error");
}

#[test]
fn bus_error_kind_no_device() {
    let err = BusError::NoDeviceAtAddress(0x10);
    assert_eq!(
        embedded_hal::i2c::Error::kind(&err),
        ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
    );
}

#[test]
fn bus_error_kind_data_nak() {
    assert_eq!(
        embedded_hal::i2c::Error::kind(&BusError::DataNak),
        ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
    );
}

#[test]
fn bus_error_kind_device_error() {
    assert_eq!(
        embedded_hal::i2c::Error::kind(&BusError::DeviceError),
        ErrorKind::Other
    );
}

#[test]
fn empty_operations_succeeds() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xFF; 4]))
        .build();

    let mut ops: [Operation<'_>; 0] = [];
    bus.transaction(0x48, &mut ops).unwrap();

    // Registers unchanged
    assert_eq!(*bus.devices().0.registers(), [0xFF; 4]);
}

#[test]
fn empty_operations_to_missing_address_still_naks() {
    let mut bus = SimBusBuilder::new().build();

    let mut ops: [Operation<'_>; 0] = [];
    let result = bus.transaction(0x10, &mut ops);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x10)));
}

#[test]
fn register_device_small_register_space() {
    // Only 2 registers -- pointer wraps quickly
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(
            Address::new(0x20).unwrap(),
            [0xAA, 0xBB],
        ))
        .build();

    let mut buf = [0u8; 4];
    bus.read(0x20, &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0xBB, 0xAA, 0xBB]);
}

#[test]
fn large_transaction_many_operations() {
    let mut bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 16]))
        .build();

    // Write to several registers in separate operations
    let mut read_buf = [0u8; 4];
    let mut ops = [
        Operation::Write(&[0x00, 0x11]),
        Operation::Write(&[0x01, 0x22]),
        Operation::Write(&[0x02, 0x33]),
        Operation::Write(&[0x03, 0x44]),
        Operation::Write(&[0x00]),
        Operation::Read(&mut read_buf),
    ];
    bus.transaction(0x50, &mut ops).unwrap();

    assert_eq!(read_buf, [0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn bus_error_clone_and_eq() {
    let err1 = BusError::NoDeviceAtAddress(0x48);
    let err2 = err1;
    assert_eq!(err1, err2);
}

#[test]
fn bus_error_debug() {
    let err = BusError::NoDeviceAtAddress(0x48);
    let debug = format!("{err:?}");
    assert!(debug.contains("NoDeviceAtAddress"));
    assert!(debug.contains("72")); // 0x48 = 72
}

#[test]
fn device_set_contains_address_for_switch_on_bus() {
    use i2c_hil_sim::devices::{I2cSwitchBuilder, Tmp1075};
    use i2c_hil_sim::device_set::DeviceSet;

    let switch = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    // Place the switch on a bus (I2cSwitch, ()) DeviceSet
    let mut devices: (_, ()) = (switch, ());

    // The switch's own address should be found
    assert!(devices.contains_address(0x70));
    // Downstream addresses are isolated and NOT found
    assert!(!devices.contains_address(0x48));
    // Nonexistent address is not found
    assert!(!devices.contains_address(0x99));

    // Exercise dispatch to switch's own address (control register)
    let mut buf = [0u8; 1];
    let mut ops = [Operation::Read(&mut buf)];
    devices.dispatch(0x70, &mut ops).unwrap();
    assert_eq!(buf[0], 0x00); // control register default
}
