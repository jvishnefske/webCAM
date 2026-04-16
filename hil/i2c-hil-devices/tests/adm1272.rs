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
fn telemetry_temperature_1() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_temperature_1(0xBEEF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8D], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xBEEF);
}

#[test]
fn telemetry_pin() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_pin(0xCAFE);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x97], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xCAFE);
}

// --- Computed STATUS_BYTE: all individual bit tests ---

#[test]
fn computed_status_byte_off_bit() {
    // Default OPERATION=0x80 (ON), so OFF bit should be clear
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_eq!(buf[0] & (1 << 6), 0, "OFF should be clear when ON");

    // Set OPERATION to 0x00 (OFF)
    bus.write(ADDR, &[0x01, 0x00]).unwrap();
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(
        buf[0] & (1 << 6),
        0,
        "OFF should be set when OPERATION=0x00"
    );
}

#[test]
fn computed_status_byte_input_bit() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0x10); // VIN_UV bit
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 3), 0, "VIN_UV should set STATUS_BYTE bit 3");
}

#[test]
fn computed_status_byte_temperature_bit() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[16] = PmBusValue::Byte(0x80); // STATUS_TEMPERATURE index
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(
        buf[0] & (1 << 2),
        0,
        "TEMPERATURE should set STATUS_BYTE bit 2"
    );
}

#[test]
fn computed_status_byte_cml_bit() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[17] = PmBusValue::Byte(0x02); // STATUS_CML index
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 1), 0, "CML should set STATUS_BYTE bit 1");
}

#[test]
fn computed_status_byte_mfr_bit() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[18] = PmBusValue::Byte(0x01); // STATUS_MFR index
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & 1, 0, "MFR should set STATUS_BYTE bit 0");
}

// --- Computed STATUS_WORD high byte: all individual bit tests ---

#[test]
fn computed_status_word_vout_aggregation() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(word & (1 << 15), 0, "STATUS_VOUT should set bit 15");
}

#[test]
fn computed_status_word_iout_aggregation() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x01); // any non-zero
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(word & (1 << 14), 0, "STATUS_IOUT should set bit 14");
}

#[test]
fn computed_status_word_input_aggregation() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0x01); // any non-zero
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(word & (1 << 13), 0, "STATUS_INPUT should set bit 13");
}

#[test]
fn computed_status_word_mfr_aggregation() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[18] = PmBusValue::Byte(0x01); // STATUS_MFR
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(word & (1 << 12), 0, "STATUS_MFR should set bit 12");
}

#[test]
fn computed_status_word_power_good_bit() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0x18); // bits 3 and 4 set
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(word & (1 << 11), 0, "POWER_GOOD# should set bit 11");
}

// --- STATUS_BYTE W1C cascade: all paths ---

#[test]
fn status_byte_w1c_cascade_input() {
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // W1C STATUS_BYTE bit 3 should cascade to clear STATUS_INPUT bit 4
    bus.write(ADDR, &[0x78, 1 << 3]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0] & 0x10, 0, "STATUS_INPUT bit 4 should be cleared");
}

#[test]
fn status_byte_w1c_cascade_temperature() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[16] = PmBusValue::Byte(0xFF); // STATUS_TEMPERATURE
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 2]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7D], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_TEMPERATURE should be cleared");
}

#[test]
fn status_byte_w1c_cascade_cml() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[17] = PmBusValue::Byte(0xFF); // STATUS_CML
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 1]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7E], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_CML should be cleared");
}

#[test]
fn status_byte_w1c_cascade_mfr() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[18] = PmBusValue::Byte(0xFF); // STATUS_MFR
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_MFR should be cleared");
}

// --- STATUS_WORD W1C cascade: mfr ---

#[test]
fn status_word_w1c_cascade_mfr() {
    use i2c_hil_sim::pmbus::{PmBusDevice, PmBusValue};
    let dev = Adm1272::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().values_mut()[18] = PmBusValue::Byte(0xFF); // STATUS_MFR
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // high byte 0x10 = clear MFR
    bus.write(ADDR, &[0x79, 0x00, 0x10]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_MFR should be cleared");
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
