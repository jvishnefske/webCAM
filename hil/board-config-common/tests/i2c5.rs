//! Integration tests for the I2C5 bus topology.

use board_config_common::i2c5;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c5::build();
}

#[test]
fn adm1272_at_0x10_responds() {
    let mut bus = i2c5::build();
    // Read PMBUS_REVISION (cmd 0x98) — expect 1 byte
    let mut buf = [0u8; 1];
    bus.write_read(0x10, &[0x98], &mut buf).unwrap();
    assert_ne!(buf[0], 0xFF);
}

#[test]
fn bmr4696001_at_0x20_responds() {
    let mut bus = i2c5::build();
    let mut buf = [0u8; 1];
    bus.write_read(0x20, &[0x98], &mut buf).unwrap();
    assert_eq!(buf[0], 0x22);
}

#[test]
fn bmr491_at_0x54_responds() {
    let mut bus = i2c5::build();
    let mut buf = [0u8; 1];
    bus.write_read(0x54, &[0x98], &mut buf).unwrap();
    assert_ne!(buf[0], 0xFF);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c5::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
