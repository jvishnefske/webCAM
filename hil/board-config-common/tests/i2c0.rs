//! Integration tests for the I2C0 bus topology.

use board_config_common::i2c0;
use embedded_hal::i2c::I2c;

#[test]
fn build_bus() {
    let _bus = i2c0::build();
}

#[test]
fn mux_channel_0_tmp1075_reads_25c() {
    let mut bus = i2c0::build();
    // Enable channel 0
    bus.write(0x70, &[0x01]).unwrap();
    // Read TMP1075 temperature register (pointer 0x00)
    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 25°C = 0x1900 (25 * 256)
    assert_eq!(raw, 0x1900);
}

#[test]
fn mux_channel_1_tmp1075_reads_30c() {
    let mut bus = i2c0::build();
    // Enable channel 1
    bus.write(0x70, &[0x02]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    let raw = u16::from_be_bytes(buf);
    // 30°C = 0x1E00 (30 * 256)
    assert_eq!(raw, 0x1E00);
}

#[test]
fn mux_channel_0_tca9555_accessible() {
    let mut bus = i2c0::build();
    bus.write(0x70, &[0x01]).unwrap();
    // Read TCA9555 configuration register (cmd 0x06)
    let mut buf = [0u8; 1];
    bus.write_read(0x20, &[0x06], &mut buf).unwrap();
    // Default: all inputs (0xFF)
    assert_eq!(buf[0], 0xFF);
}

#[test]
fn mux_channel_1_tca9555_accessible() {
    let mut bus = i2c0::build();
    bus.write(0x70, &[0x02]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(0x20, &[0x06], &mut buf).unwrap();
    assert_eq!(buf[0], 0xFF);
}

#[test]
fn address_isolation_no_channel_enabled() {
    let mut bus = i2c0::build();
    // No channel enabled — 0x48 should NAK
    let mut buf = [0u8; 2];
    let result = bus.write_read(0x48, &[0x00], &mut buf);
    assert!(result.is_err());
}

#[test]
fn mux_control_register_readback() {
    let mut bus = i2c0::build();
    // Write channel 1 enable
    bus.write(0x70, &[0x02]).unwrap();
    // Read back control register
    let mut buf = [0u8; 1];
    bus.read(0x70, &mut buf).unwrap();
    assert_eq!(buf[0], 0x02);
}
