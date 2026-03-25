//! Integration tests for the I2C2 bus topology.

use board_config_common::i2c2;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c2::build();
}

#[test]
fn tmp1075_at_0x48_reads_35c() {
    let mut bus = i2c2::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 35 °C = 0x2300 (35 * 256)
    assert_eq!(raw, 0x2300);
}

#[test]
fn tmp1075_at_0x49_reads_40c() {
    let mut bus = i2c2::build();
    let mut buf = [0u8; 2];
    bus.write_read(0x49, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 40 °C = 0x2800 (40 * 256)
    assert_eq!(raw, 0x2800);
}

#[test]
fn eeprom_write_and_readback() {
    let mut bus = i2c2::build();
    // Write 4 bytes at address 0x0000 in EEPROM at 0x50
    bus.write(0x50, &[0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF])
        .unwrap();
    // Set read pointer back to 0x0000
    bus.write(0x50, &[0x00, 0x00]).unwrap();
    // Read back
    let mut buf = [0u8; 4];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c2::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
