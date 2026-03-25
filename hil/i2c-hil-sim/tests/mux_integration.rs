//! End-to-end integration test: I2cSwitch (TCA9543A-style) with TMP1075
//! sensors on each channel, matching the firmware's target topology.
//!
//! ```text
//! I2cSwitch @ 0x70
//! +-- channel 0 -> Tmp1075 @ 0x4E (25 C)
//! +-- channel 1 -> Tmp1075 @ 0x4E (50 C)
//! ```

use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::{I2cSwitch, I2cSwitchBuilder, Tmp1075};
use i2c_hil_sim::{Address, BusError};

#[allow(clippy::type_complexity)]
fn build_mux() -> I2cSwitch<((Tmp1075, ()), ((Tmp1075, ()), ()))> {
    I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            Tmp1075::celsius_to_raw(25.0),
        ))
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            Tmp1075::celsius_to_raw(50.0),
        ))
        .build()
}

#[test]
fn select_channel0_read_25c() {
    let mut mux = build_mux();

    mux.write(0x70, &[0x01]).unwrap();

    let mut buf = [0u8; 2];
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]); // 25.0 C
}

#[test]
fn select_channel1_read_50c() {
    let mut mux = build_mux();

    mux.write(0x70, &[0x02]).unwrap();

    let mut buf = [0u8; 2];
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]); // 50.0 C
}

#[test]
fn switch_channels_reads_different_temperatures() {
    let mut mux = build_mux();

    let mut buf = [0u8; 2];

    // Channel 0 -> 25 C
    mux.write(0x70, &[0x01]).unwrap();
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);

    // Channel 1 -> 50 C
    mux.write(0x70, &[0x02]).unwrap();
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn no_channel_selected_returns_error() {
    let mut mux = build_mux();

    let mut buf = [0u8; 2];
    let result = mux.read(0x4E, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x4E)));
}

#[test]
fn write_config_through_mux() {
    let mut mux = build_mux();

    // Select channel 0
    mux.write(0x70, &[0x01]).unwrap();

    // Write config register on channel 0 TMP1075
    mux.write(0x4E, &[0x01, 0xAB, 0xCD]).unwrap();

    // Read config back
    let mut buf = [0u8; 2];
    mux.write_read(0x4E, &[0x01], &mut buf).unwrap();
    assert_eq!(buf, [0xAB, 0xCD]);

    // Channel 1's config should still be default
    mux.write(0x70, &[0x02]).unwrap();
    mux.write_read(0x4E, &[0x01], &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0xFF]);
}

#[test]
fn read_control_register_reflects_channel_selection() {
    let mut mux = build_mux();

    // Initially 0
    let mut ctrl = [0u8; 1];
    mux.read(0x70, &mut ctrl).unwrap();
    assert_eq!(ctrl[0], 0x00);

    // Select channel 0
    mux.write(0x70, &[0x01]).unwrap();
    mux.read(0x70, &mut ctrl).unwrap();
    assert_eq!(ctrl[0], 0x01);

    // Select channel 1
    mux.write(0x70, &[0x02]).unwrap();
    mux.read(0x70, &mut ctrl).unwrap();
    assert_eq!(ctrl[0], 0x02);

    // Both channels
    mux.write(0x70, &[0x03]).unwrap();
    mux.read(0x70, &mut ctrl).unwrap();
    assert_eq!(ctrl[0], 0x03);
}

#[test]
fn build_with_temperature_then_read_through_mux() {
    let raw_100c = Tmp1075::celsius_to_raw(100.0);
    let mut mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            raw_100c,
        ))
        .channel(Tmp1075::with_temperature(
            Address::new(0x4E).unwrap(),
            Tmp1075::celsius_to_raw(50.0),
        ))
        .build();

    // Read through mux
    mux.write(0x70, &[0x01]).unwrap();
    let mut buf = [0u8; 2];
    mux.write_read(0x4E, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, raw_100c.to_be_bytes());
}

#[test]
fn address_not_on_any_channel_returns_error() {
    let mut mux = build_mux();

    mux.write(0x70, &[0x01]).unwrap();

    let mut buf = [0u8; 2];
    let result = mux.read(0x50, &mut buf);
    assert_eq!(result, Err(BusError::NoDeviceAtAddress(0x50)));
}
