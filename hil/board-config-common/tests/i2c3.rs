//! Integration tests for the I2C3 bus topology.

use board_config_common::i2c3;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c3::build();
}

#[test]
fn tmp1075_at_0x4a_reads_45c() {
    let mut bus = i2c3::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x4A, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 45 °C = 0x2D00 (45 * 256)
    assert_eq!(raw, 0x2D00);
}

#[test]
fn tmp1075_at_0x4b_reads_50c() {
    let mut bus = i2c3::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x4B, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 50 °C = 0x3200 (50 * 256)
    assert_eq!(raw, 0x3200);
}

#[test]
fn eeprom_write_and_readback() {
    let mut bus = i2c3::build();
    // Write 4 bytes at address 0x0000 in EEPROM at 0x51
    bus.write(0x51, &[0x00, 0x00, 0xCA, 0xFE, 0xBA, 0xBE])
        .unwrap();
    // Set read pointer back to 0x0000
    bus.write(0x51, &[0x00, 0x00]).unwrap();
    // Read back
    let mut buf = [0u8; 4];
    bus.read(0x51, &mut buf).unwrap();
    assert_eq!(buf, [0xCA, 0xFE, 0xBA, 0xBE]);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c3::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
