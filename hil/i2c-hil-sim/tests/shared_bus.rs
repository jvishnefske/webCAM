use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::RegisterDevice;
use i2c_hil_sim::{Address, BusError, SharedBus, SimBusBuilder};

#[test]
fn two_handles_talk_to_different_devices() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0xBB; 4]))
        .build_shared();

    let mut h1 = shared.handle();
    let mut h2 = shared.handle();

    let mut buf1 = [0u8; 2];
    let mut buf2 = [0u8; 2];

    h1.read(0x48, &mut buf1).unwrap();
    h2.read(0x50, &mut buf2).unwrap();

    assert_eq!(buf1, [0xAA, 0xAA]);
    assert_eq!(buf2, [0xBB, 0xBB]);
}

#[test]
fn device_state_visible_through_shared() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 8]))
        .build_shared();

    let mut handle = shared.handle();
    handle.write(0x50, &[0x02, 0xFF]).unwrap();

    let devices = shared.devices();
    assert_eq!(devices.0.registers()[2], 0xFF);
}

#[test]
fn write_via_one_handle_read_via_another() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 8]))
        .build_shared();

    let mut writer = shared.handle();
    writer.write(0x50, &[0x03, 0xDE, 0xAD]).unwrap();

    let mut reader = shared.handle();
    let mut buf = [0u8; 2];
    reader.write_read(0x50, &[0x03], &mut buf).unwrap();

    assert_eq!(buf, [0xDE, 0xAD]);
}

#[test]
fn handle_dropped_new_handle_still_works() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0x00; 4]))
        .build_shared();

    {
        let mut h = shared.handle();
        h.write(0x48, &[0x00, 0x42]).unwrap();
        // h dropped here
    }

    let mut h2 = shared.handle();
    let mut buf = [0u8; 1];
    h2.write_read(0x48, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x42]);
}

#[test]
fn error_from_one_handle_does_not_poison_bus() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0xCC; 4]))
        .build_shared();

    let mut h1 = shared.handle();
    let mut h2 = shared.handle();

    // h1 tries a nonexistent address — should error
    let mut buf = [0u8; 1];
    let err = h1.read(0x99, &mut buf).unwrap_err();
    assert_eq!(err, BusError::NoDeviceAtAddress(0x99));

    // h2 should still work fine
    let mut buf2 = [0u8; 2];
    h2.read(0x48, &mut buf2).unwrap();
    assert_eq!(buf2, [0xCC, 0xCC]);
}

#[test]
fn build_shared_constructs_shared_bus() {
    let shared = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0x11; 4]))
        .build_shared();

    let mut h = shared.handle();
    let mut buf = [0u8; 1];
    h.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x11]);
}

#[test]
fn shared_bus_from_new() {
    let bus = SimBusBuilder::new()
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0x22; 4]))
        .build();

    let shared = SharedBus::new(bus);
    let mut h = shared.handle();
    let mut buf = [0u8; 1];
    h.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x22]);
}
