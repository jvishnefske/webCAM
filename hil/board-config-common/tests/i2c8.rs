//! Integration tests for the I2C8 bus topology.

use board_config_common::i2c8;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c8::build();
}

#[test]
fn ltc4287_at_0x44_responds() {
    let mut bus = i2c8::build();
    // Read PMBUS_REVISION (cmd 0x98)
    let mut buf = [0u8; 1];
    bus.write_read(0x44, &[0x98], &mut buf).unwrap();
    assert_ne!(buf[0], 0xFF);
}

#[test]
fn ina230_at_0x45_responds() {
    let mut bus = i2c8::build();
    // Read die ID register (0xFF) — default 0x2260
    let mut buf = [0u8; 2];
    bus.write_read(0x45, &[0xFF], &mut buf).unwrap();
    let die_id = u16::from_be_bytes(buf);
    assert_eq!(die_id, 0x2260);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c8::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
