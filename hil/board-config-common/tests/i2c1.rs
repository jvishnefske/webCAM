//! Integration tests for the I2C1 bus topology.

use board_config_common::i2c1;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c1::build();
}

#[test]
fn ina230_at_0x41_responds() {
    let mut bus = i2c1::build();
    // Read die ID register (0xFF) — default 0x2260
    let mut buf = [0u8; 2];
    bus.write_read(0x41, &[0xFF], &mut buf).unwrap();
    let die_id = u16::from_be_bytes(buf);
    assert_eq!(die_id, 0x2260);
}

#[test]
fn ina230_at_0x42_responds() {
    let mut bus = i2c1::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x42, &[0xFF], &mut buf).unwrap();
    let die_id = u16::from_be_bytes(buf);
    assert_eq!(die_id, 0x2260);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c1::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
