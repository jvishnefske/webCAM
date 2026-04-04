//! Integration tests for the ADM1272 hot swap controller simulation.

use embedded_hal::i2c::I2c;
use i2c_hil_devices::Adm1272;
use i2c_hil_sim::{Address, PmBusEngine, SimBusBuilder};

const ADDR: u8 = 0x10;

fn make_bus() -> i2c_hil_sim::SimBus<(PmBusEngine<Adm1272>, ())> {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build()
}

#[test]
fn read_pmbus_revision() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x98], &mut buf).unwrap();
    assert_eq!(buf[0], 0x22);
}

#[test]
fn read_mfr_id_block() {
    let mut bus = make_bus();
    let mut buf = [0u8; 4];
    bus.write_read(ADDR, &[0x99], &mut buf).unwrap();
    assert_eq!(buf[0], 3); // length
    assert_eq!(&buf[1..4], b"ADI");
}

#[test]
fn telemetry_injection() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vin(0xABCD);
    engine.device_mut().set_read_vout(0x1234);
    engine.device_mut().set_read_iout(0x5678);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x88], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);

    bus.write_read(ADDR, &[0x8B], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);

    bus.write_read(ADDR, &[0x8C], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
}

#[test]
fn write_protect_wp1() {
    let mut bus = make_bus();
    // Enable WP1
    bus.write(ADDR, &[0x10, 0x80]).unwrap();
    // OPERATION write should fail
    assert!(bus.write(ADDR, &[0x01, 0x42]).is_err());
    // WRITE_PROTECT and PAGE still work
    bus.write(ADDR, &[0x10, 0x00]).unwrap();
    bus.write(ADDR, &[0x00, 0x01]).unwrap();
}

#[test]
fn status_w1c() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // W1C: clear lower nibble
    bus.write(ADDR, &[0x7A, 0x0F]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0xF0);
}

#[test]
fn computed_status_byte() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80); // IOUT_OC bit
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 4), 0); // STATUS_BYTE bit 4
}

#[test]
fn clear_faults() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    engine.device_mut().set_status_iout(0xFF);
    engine.device_mut().set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x03]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
}

#[test]
fn read_only_rejects_write() {
    let mut bus = make_bus();
    assert!(bus.write(ADDR, &[0x88, 0x00, 0x00]).is_err());
}

#[test]
fn limit_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x57, 0xCD, 0xAB]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x57], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn status_byte_w1c_cascade() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // W1C STATUS_BYTE bit 4 should cascade to clear STATUS_IOUT bit 7
    bus.write(ADDR, &[0x78, 1 << 4]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0] & 0x80, 0);
}

#[test]
fn status_word_w1c_cascade() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    engine.device_mut().set_status_iout(0xFF);
    engine.device_mut().set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // W1C STATUS_WORD: high byte 0x80 clears STATUS_VOUT,
    // high byte 0x40 clears STATUS_IOUT, high byte 0x20 clears STATUS_INPUT
    bus.write(ADDR, &[0x79, 0x00, 0xE0]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_VOUT should be cleared");

    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_IOUT should be cleared");

    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_INPUT should be cleared");
}
