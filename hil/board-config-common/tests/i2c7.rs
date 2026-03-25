//! Integration tests for the I2C7 bus topology.

use board_config_common::i2c7;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c7::build();
}

#[test]
fn emc2305_at_0x2c_product_id() {
    let mut bus = i2c7::build();
    // Product ID register (0xFD) — EMC2305 returns 0x34
    let mut buf = [0u8; 1];
    bus.write_read(0x2C, &[0xFD], &mut buf).unwrap();
    assert_eq!(buf[0], 0x34);
}

#[test]
fn tmp1075_at_0x48_reads_55c() {
    let mut bus = i2c7::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 55 °C = 0x3700 (55 * 256)
    assert_eq!(raw, 0x3700);
}

#[test]
fn tmp1075_at_0x49_reads_60c() {
    let mut bus = i2c7::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x49, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 60 °C = 0x3C00 (60 * 256)
    assert_eq!(raw, 0x3C00);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c7::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
