use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::{I2cSwitchBuilder, RegisterDevice, Tmp1075};
use i2c_hil_sim::{Address, BusError};

fn mux_addr() -> Address {
    Address::new(0x70).unwrap()
}

#[test]
fn control_register_defaults_to_zero() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    let mut buf = [0u8; 1];
    mux.read(0x70, &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
}

#[test]
fn write_and_read_control_register() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    // Enable channel 0
    mux.write(0x70, &[0x01]).unwrap();
    assert_eq!(mux.control(), 0x01);

    let mut buf = [0u8; 1];
    mux.read(0x70, &mut buf).unwrap();
    assert_eq!(buf[0], 0x01);
}

#[test]
fn control_register_masks_upper_bits() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .channel(Tmp1075::new(Address::new(0x49).unwrap()))
        .build();

    // 2-channel switch: only bits 0-1 valid
    mux.write(0x70, &[0xFF]).unwrap();
    assert_eq!(mux.control(), 0x03);
}

#[test]
fn control_register_masks_to_channel_count() {
    // 8-channel switch: all 8 bits valid
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x40).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x41).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x42).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x43).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x44).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x45).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x46).unwrap(), [0u8; 4]))
        .channel(RegisterDevice::new(Address::new(0x47).unwrap(), [0u8; 4]))
        .build();

    mux.write(0x70, &[0xFF]).unwrap();
    assert_eq!(mux.control(), 0xFF);
}

#[test]
fn no_channel_enabled_returns_no_device() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    // Control is 0 by default -- no channels enabled
    let mut buf = [0u8; 2];
    let result = mux.read(0x48, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x48)));
}

#[test]
fn channel0_routing() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::with_temperature(
            Address::new(0x48).unwrap(),
            Tmp1075::celsius_to_raw(25.0),
        ))
        .build();

    // Enable channel 0
    mux.write(0x70, &[0x01]).unwrap();

    let mut buf = [0u8; 2];
    mux.write_read(0x48, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);
}

#[test]
fn channel1_routing() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .channel(Tmp1075::with_temperature(
            Address::new(0x49).unwrap(),
            Tmp1075::celsius_to_raw(50.0),
        ))
        .build();

    // Enable channel 1
    mux.write(0x70, &[0x02]).unwrap();

    let mut buf = [0u8; 2];
    mux.write_read(0x49, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn switching_channels_routes_differently() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0xAA; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0xBB; 4]))
        .build();

    // Channel 0
    mux.write(0x70, &[0x01]).unwrap();
    let mut buf = [0u8; 2];
    mux.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0xAA]);

    // Switch to channel 1
    mux.write(0x70, &[0x02]).unwrap();
    mux.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xBB, 0xBB]);
}

#[test]
fn both_channels_enabled_prefers_channel0() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0xAA; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0xBB; 4]))
        .build();

    // Enable both channels
    mux.write(0x70, &[0x03]).unwrap();
    let mut buf = [0u8; 2];
    mux.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0xAA]);
}

#[test]
fn both_channels_enabled_falls_through_to_channel1() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x48).unwrap(), [0xAA; 4]))
        .channel(RegisterDevice::new(Address::new(0x49).unwrap(), [0xBB; 4]))
        .build();

    // Enable both channels, access device only on channel 1
    mux.write(0x70, &[0x03]).unwrap();
    let mut buf = [0u8; 2];
    mux.read(0x49, &mut buf).unwrap();
    assert_eq!(buf, [0xBB, 0xBB]);
}

#[test]
fn disable_channels_after_use() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    // Enable, then disable
    mux.write(0x70, &[0x01]).unwrap();
    mux.write(0x70, &[0x00]).unwrap();

    let mut buf = [0u8; 2];
    let result = mux.read(0x48, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x48)));
}

#[test]
fn duplicate_address_on_same_channel_builds_separate_channels() {
    // Each .channel() call creates a new channel, so same address on
    // different channels is fine (that's the point of a mux)
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .build();

    // Both channels have 0x48, enable channel 0
    mux.write(0x70, &[0x01]).unwrap();
    let mut buf = [0u8; 2];
    mux.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0x00]);
}

#[test]
fn same_address_on_different_channels_is_ok() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            Tmp1075::celsius_to_raw(25.0),
        ))
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            Tmp1075::celsius_to_raw(50.0),
        ))
        .build();

    // Channel 0
    mux.write(0x70, &[0x01]).unwrap();
    let mut buf = [0u8; 2];
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);

    // Channel 1
    mux.write(0x70, &[0x02]).unwrap();
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn eight_channel_select_each() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x40).unwrap(), [0x00; 4]))
        .channel(RegisterDevice::new(Address::new(0x41).unwrap(), [0x11; 4]))
        .channel(RegisterDevice::new(Address::new(0x42).unwrap(), [0x22; 4]))
        .channel(RegisterDevice::new(Address::new(0x43).unwrap(), [0x33; 4]))
        .channel(RegisterDevice::new(Address::new(0x44).unwrap(), [0x44; 4]))
        .channel(RegisterDevice::new(Address::new(0x45).unwrap(), [0x55; 4]))
        .channel(RegisterDevice::new(Address::new(0x46).unwrap(), [0x66; 4]))
        .channel(RegisterDevice::new(Address::new(0x47).unwrap(), [0x77; 4]))
        .build();

    // Select each channel individually and verify correct device
    for ch in 0u8..8 {
        mux.write(0x70, &[1 << ch]).unwrap();
        let mut buf = [0u8; 1];
        let addr = 0x40 + ch;
        mux.read(addr, &mut buf).unwrap();
        assert_eq!(buf[0], ch * 0x11, "channel {ch} data mismatch");
    }
}

#[test]
fn eight_channel_multiple_enabled() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x00; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x11; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x22; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x33; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x44; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x55; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x66; 4]))
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0x77; 4]))
        .build();

    // Enable channels 0 and 7; channel 0 should be preferred
    mux.write(0x70, &[0x81]).unwrap(); // bit 0 + bit 7
    let mut buf = [0u8; 1];
    mux.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0x00); // channel 0 data

    // Enable only channel 7
    mux.write(0x70, &[0x80]).unwrap();
    mux.read(0x50, &mut buf).unwrap();
    assert_eq!(buf[0], 0x77); // channel 7 data
}

#[test]
fn three_channel_switch() {
    // 3-channel switch: mask should be 0x07
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(RegisterDevice::new(Address::new(0x50).unwrap(), [0xAA; 4]))
        .channel(RegisterDevice::new(Address::new(0x51).unwrap(), [0xBB; 4]))
        .channel(RegisterDevice::new(Address::new(0x52).unwrap(), [0xCC; 4]))
        .build();

    // Upper bits should be masked
    mux.write(0x70, &[0xFF]).unwrap();
    assert_eq!(mux.control(), 0x07);

    // All three channels enabled, access channel 2
    let mut buf = [0u8; 1];
    mux.read(0x52, &mut buf).unwrap();
    assert_eq!(buf[0], 0xCC);
}

#[test]
fn empty_channel() {
    let mut mux = I2cSwitchBuilder::new(mux_addr())
        .channel(Tmp1075::new(Address::new(0x48).unwrap()))
        .empty_channel()
        .build();

    // Enable channel 1 (empty) and try to access device
    mux.write(0x70, &[0x02]).unwrap();
    let mut buf = [0u8; 2];
    let result = mux.read(0x48, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x48)));

    // Channel 0 works
    mux.write(0x70, &[0x01]).unwrap();
    mux.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0x00]);
}
