//! Integration tests for the I2C6 bus topology.

use board_config_common::i2c6;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c6::build();
}

#[test]
fn isl68224_at_0x60_responds() {
    let mut bus = i2c6::build();
    // Read PMBUS_REVISION (cmd 0x98)
    let mut buf = [0u8; 1];
    bus.write_read(0x60, &[0x98], &mut buf).unwrap();
    assert_ne!(buf[0], 0xFF);
}

#[test]
fn raa228926_at_0x61_responds() {
    let mut bus = i2c6::build();
    let mut buf = [0u8; 1];
    bus.write_read(0x61, &[0x98], &mut buf).unwrap();
    assert_ne!(buf[0], 0xFF);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c6::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x50, &mut buf);
    assert!(result.is_err());
}
