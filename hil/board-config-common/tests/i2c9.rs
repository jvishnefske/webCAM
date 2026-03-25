//! Integration tests for the I2C9 bus topology.

use board_config_common::i2c9;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c9::build();
}

#[test]
fn eeprom_write_and_readback() {
    let mut bus = i2c9::build();
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
fn register_device_at_0x30_write_and_read() {
    let mut bus = i2c9::build();
    // Write to register 0 at address 0x30
    bus.write(0x30, &[0x00, 0xAB]).unwrap();
    // Read back
    let mut buf = [0u8; 1];
    bus.write_read(0x30, &[0x00], &mut buf).unwrap();
    assert_eq!(buf[0], 0xAB);
}

#[test]
fn unknown_address_naks() {
    let mut bus = i2c9::build();
    let mut buf = [0u8; 1];
    let result = bus.read(0x60, &mut buf);
    assert!(result.is_err());
}
