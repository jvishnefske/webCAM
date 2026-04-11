//! Integration tests for the BMR4696001 dual-output PoL DC-DC converter simulation.

use embedded_hal::i2c::I2c;
use i2c_hil_devices::Bmr4696001;
use i2c_hil_sim::{Address, PmBusEngine, SimBusBuilder};

const ADDR: u8 = 0x20;

fn make_bus() -> i2c_hil_sim::SimBus<(PmBusEngine<Bmr4696001>, ())> {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
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
    let mut buf = [0u8; 5];
    bus.write_read(ADDR, &[0x99], &mut buf).unwrap();
    assert_eq!(buf[0], 4); // length
    assert_eq!(&buf[1..5], b"FLEX");
}

#[test]
fn read_mfr_model_block() {
    let mut bus = make_bus();
    let mut buf = [0u8; 11];
    bus.write_read(ADDR, &[0x9A], &mut buf).unwrap();
    assert_eq!(buf[0], 10); // length
    assert_eq!(&buf[1..11], b"BMR4696001");
}

#[test]
fn read_mfr_revision_block() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x9B], &mut buf).unwrap();
    assert_eq!(buf[0], 1); // length
    assert_eq!(buf[1], b'A');
}

#[test]
fn default_operation() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x01], &mut buf).unwrap();
    assert_eq!(buf[0], 0x40);
}

#[test]
fn default_on_off_config() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x02], &mut buf).unwrap();
    assert_eq!(buf[0], 0x17);
}

#[test]
fn default_vout_mode() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x20], &mut buf).unwrap();
    assert_eq!(buf[0], 0x13);
}

#[test]
fn default_ot_fault_limit() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x4F], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xEBE8);
}

#[test]
fn default_user_config() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0xD1], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x10A4);
}

#[test]
fn telemetry_injection() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
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
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_temperature_1(0xDEAD);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8D], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xDEAD);
}

#[test]
fn telemetry_temperature_3() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_temperature_3(0xBEEF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8F], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xBEEF);
}

#[test]
fn telemetry_duty_cycle_and_frequency() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_duty_cycle(0x1111);
    engine.device_mut().set_read_frequency(0x2222);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x94], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1111);

    bus.write_read(ADDR, &[0x95], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x2222);
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
fn write_protect_wp2_allows_operation() {
    let mut bus = make_bus();
    // Enable WP2
    bus.write(ADDR, &[0x10, 0x40]).unwrap();
    // OPERATION write should succeed
    bus.write(ADDR, &[0x01, 0x80]).unwrap();
    // Other registers should fail
    assert!(bus.write(ADDR, &[0x21, 0x00, 0x10]).is_err());
}

#[test]
fn status_w1c() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
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
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80); // IOUT_OC bit
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    // Bit 4 (IOUT) and bit 6 (OFF — OPERATION=0x40, bit 7 clear)
    assert_ne!(buf[0] & (1 << 4), 0, "IOUT_OC should set STATUS_BYTE bit 4");
    assert_ne!(
        buf[0] & (1 << 6),
        0,
        "OFF bit should be set (OPERATION=0x40)"
    );
}

#[test]
fn computed_status_word() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    // High byte bit 7 (bit 15) = VOUT
    assert_ne!(
        word & (1 << 15),
        0,
        "STATUS_VOUT should set STATUS_WORD bit 15"
    );
}

#[test]
fn clear_faults() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    engine.device_mut().set_status_iout(0xFF);
    engine.device_mut().set_status_input(0xFF);
    engine.device_mut().set_status_temperature(0xFF);
    engine.device_mut().set_status_cml(0xFF);
    engine.device_mut().set_status_mfr_specific(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x03]).unwrap();

    let mut buf = [0u8; 1];
    for cmd in [0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x80] {
        bus.write_read(ADDR, &[cmd], &mut buf).unwrap();
        assert_eq!(buf[0], 0x00, "Sub-status 0x{cmd:02X} should be cleared");
    }
}

#[test]
fn read_only_rejects_write() {
    let mut bus = make_bus();
    assert!(bus.write(ADDR, &[0x88, 0x00, 0x00]).is_err());
}

#[test]
fn limit_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x55, 0xCD, 0xAB]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x55], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn status_byte_w1c_cascade() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
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
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // W1C STATUS_WORD high bit 7 (bit 15) should cascade to clear STATUS_VOUT
    bus.write(ADDR, &[0x79, 0x00, 0x80]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
}

#[test]
fn store_default_all_does_not_nak() {
    let mut bus = make_bus();
    // Send-byte 0x11 should be accepted (not NAK)
    bus.write(ADDR, &[0x11]).unwrap();
}

#[test]
fn vout_command_read_write() {
    let mut bus = make_bus();
    // Default is 0x2000
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x21], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x2000);

    // Write new value
    bus.write(ADDR, &[0x21, 0x34, 0x12]).unwrap();
    bus.write_read(ADDR, &[0x21], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

#[test]
fn mfr_fault_response_defaults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    // MFR_IOUT_OC_FAULT_RESPONSE (0xE5) default = 0x80
    bus.write_read(ADDR, &[0xE5], &mut buf).unwrap();
    assert_eq!(buf[0], 0x80);
}

// --- Individual status setter tests ---

#[test]
fn status_vout_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_vout(0xAA);
    let engine = PmBusEngine::new(dev);
    let mut bus = SimBusBuilder::new().with_device(engine).build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0xAA);
}

#[test]
fn status_iout_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_iout(0xBB);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0], 0xBB);
}

#[test]
fn status_input_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_input(0xCC);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0], 0xCC);
}

#[test]
fn status_temperature_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_temperature(0xDD);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7D], &mut buf).unwrap();
    assert_eq!(buf[0], 0xDD);
}

#[test]
fn status_cml_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_cml(0xEE);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7E], &mut buf).unwrap();
    assert_eq!(buf[0], 0xEE);
}

#[test]
fn status_mfr_specific_setter() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_mfr_specific(0x55);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x55);
}

// --- Individual computed STATUS_BYTE bit tests ---

#[test]
fn computed_status_byte_temperature() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_temperature(0x01);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 2), 0, "TEMPERATURE bit should be set");
}

#[test]
fn computed_status_byte_vin_uv() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_input(0x10);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 3), 0, "VIN_UV bit should be set");
}

#[test]
fn computed_status_byte_cml() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_cml(0x01);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 1), 0, "CML bit should be set");
}

#[test]
fn computed_status_byte_mfr_specific() {
    let mut dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    dev.set_status_mfr_specific(0x01);
    let mut bus = SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & 1, 0, "NONE_OF_THE_ABOVE bit should be set");
}

#[test]
fn computed_status_word_iout_aggregation() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(
        word & (1 << 14),
        0,
        "STATUS_IOUT should set STATUS_WORD bit 14"
    );
}

#[test]
fn computed_status_word_input_aggregation() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0x10);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(
        word & (1 << 13),
        0,
        "STATUS_INPUT should set STATUS_WORD bit 13"
    );
}

#[test]
fn computed_status_word_mfr_aggregation() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_mfr_specific(0x01);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(
        word & (1 << 12),
        0,
        "STATUS_MFR should set STATUS_WORD bit 12"
    );
}

// --- STATUS_BYTE W1C cascade: all individual paths ---

#[test]
fn status_byte_w1c_cascade_input() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 3]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0] & 0x10, 0, "STATUS_INPUT bit 4 should be cleared");
}

#[test]
fn status_byte_w1c_cascade_temperature() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_temperature(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 2]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7D], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_TEMPERATURE should be cleared");
}

#[test]
fn status_byte_w1c_cascade_cml() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_cml(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 1]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7E], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_CML should be cleared");
}

#[test]
fn status_byte_w1c_cascade_mfr() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_mfr_specific(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_MFR should be cleared");
}

// --- STATUS_WORD W1C cascade: high byte individual paths ---

#[test]
fn status_word_w1c_cascade_iout() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // high byte 0x40 = clear IOUT
    bus.write(ADDR, &[0x79, 0x00, 0x40]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_IOUT should be cleared");
}

#[test]
fn status_word_w1c_cascade_input() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // high byte 0x20 = clear INPUT
    bus.write(ADDR, &[0x79, 0x00, 0x20]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7C], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_INPUT should be cleared");
}

#[test]
fn status_word_w1c_cascade_mfr() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_mfr_specific(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    // high byte 0x10 = clear MFR
    bus.write(ADDR, &[0x79, 0x00, 0x10]).unwrap();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_MFR should be cleared");
}

// --- ON state with OPERATION=0x80: all false paths ---

#[test]
fn computed_status_byte_on_no_faults() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let engine = PmBusEngine::new(dev);
    // Set OPERATION to 0x80 (ON) via bus write
    let mut bus = SimBusBuilder::new().with_device(engine).build();
    bus.write(ADDR, &[0x01, 0x80]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_BYTE should be 0 when ON and no faults");
}

#[test]
fn computed_status_word_on_no_faults() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x01, 0x80]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x0000, "STATUS_WORD should be 0 when ON and no faults");
}

#[test]
fn telemetry_setters_via_bus() {
    let dev = Bmr4696001::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vin(0x1234);
    engine.device_mut().set_read_vout(0x5678);
    engine.device_mut().set_read_iout(0x9ABC);
    engine.device_mut().set_read_temperature_1(0xDEF0);
    engine.device_mut().set_read_temperature_3(0x1111);
    engine.device_mut().set_read_duty_cycle(0x2222);
    engine.device_mut().set_read_frequency(0x3333);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x88], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
    bus.write_read(ADDR, &[0x8B], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
    bus.write_read(ADDR, &[0x8C], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x9ABC);
    bus.write_read(ADDR, &[0x8D], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xDEF0);
    bus.write_read(ADDR, &[0x8F], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1111);
    bus.write_read(ADDR, &[0x94], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x2222);
    bus.write_read(ADDR, &[0x95], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x3333);
}
