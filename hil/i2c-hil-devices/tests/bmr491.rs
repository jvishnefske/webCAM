//! Integration tests for the BMR491 DC-DC converter simulation.

use embedded_hal::i2c::I2c;
use i2c_hil_devices::Bmr491;
use i2c_hil_sim::{Address, PmBusEngine, SimBusBuilder};

const ADDR: u8 = 0x30;

fn make_bus() -> i2c_hil_sim::SimBus<(PmBusEngine<Bmr491>, ())> {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    SimBusBuilder::new()
        .with_device(PmBusEngine::new(dev))
        .build()
}

// --- Identification ---

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
    let mut buf = [0u8; 7];
    bus.write_read(ADDR, &[0x9A], &mut buf).unwrap();
    assert_eq!(buf[0], 6); // length
    assert_eq!(&buf[1..7], b"BMR491");
}

#[test]
fn read_mfr_revision_block() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x9B], &mut buf).unwrap();
    assert_eq!(buf[0], 1); // length
    assert_eq!(buf[1], b'A');
}

// --- Default register values ---

#[test]
fn default_operation() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x01], &mut buf).unwrap();
    assert_eq!(buf[0], 0x80);
}

#[test]
fn default_capability() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x19], &mut buf).unwrap();
    assert_eq!(buf[0], 0xB0);
}

// --- Telemetry injection ---

#[test]
fn telemetry_vin() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vin(0xABCD);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x88], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn telemetry_vout() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vout(0x1234);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8B], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

#[test]
fn telemetry_iout() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_iout(0x5678);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8C], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
}

#[test]
fn telemetry_temperature() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_temperature_1(0xBEEF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8D], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xBEEF);
}

#[test]
fn telemetry_pin() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_pin(0xCAFE);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x97], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xCAFE);
}

// --- Computed status ---

#[test]
fn computed_status_byte_off_bit() {
    // Default OPERATION=0x80 (ON), so OFF bit (bit 6) should NOT be set
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_eq!(
        buf[0] & (1 << 6),
        0,
        "OFF bit should be clear when OPERATION=0x80"
    );

    // Set OPERATION to 0x00 (OFF)
    bus.write(ADDR, &[0x01, 0x00]).unwrap();
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(
        buf[0] & (1 << 6),
        0,
        "OFF bit should be set when OPERATION=0x00"
    );
}

#[test]
fn computed_status_word_off_bit() {
    // Default OPERATION=0x80, so OFF bit in low byte should NOT be set
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_eq!(
        word & (1 << 6),
        0,
        "OFF bit should be clear when OPERATION=0x80"
    );

    // Set OPERATION to 0x00 (OFF)
    bus.write(ADDR, &[0x01, 0x00]).unwrap();
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(
        word & (1 << 6),
        0,
        "OFF bit should be set when OPERATION=0x00"
    );
}

// --- Clear faults ---

#[test]
fn clear_faults_accepted() {
    let mut bus = make_bus();
    // CLEAR_FAULTS send-byte should succeed
    bus.write(ADDR, &[0x03]).unwrap();
}

// --- Write protect ---

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

// --- Read-only register protection ---

#[test]
fn read_only_rejects_write() {
    let mut bus = make_bus();
    assert!(bus.write(ADDR, &[0x88, 0x00, 0x00]).is_err());
}

// --- Limit read/write ---

#[test]
fn limit_vin_ov_warn_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x57, 0xCD, 0xAB]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x57], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn limit_vin_uv_warn_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x58, 0x34, 0x12]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x58], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

#[test]
fn limit_iout_oc_fault_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x46, 0x78, 0x56]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x46], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
}

// --- Computed status ---

#[test]
fn computed_status_byte_defaults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    // With default OPERATION, OFF bit may or may not be set
    // Just verify it reads without error
    let _ = buf[0];
}

#[test]
fn computed_status_word_defaults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let _word = u16::from_le_bytes(buf);
}

#[test]
fn clear_faults_clears_all_status() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x03]).unwrap();
    // Verify sub-status registers are 0
    let mut buf = [0u8; 1];
    for cmd in [0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x80] {
        bus.write_read(ADDR, &[cmd], &mut buf).unwrap();
        assert_eq!(buf[0], 0x00, "status cmd 0x{:02X} not cleared", cmd);
    }
}

// --- Status W1C ---

#[test]
fn status_vout_w1c() {
    let mut bus = make_bus();
    // STATUS_VOUT (0x7A) is W1C — cannot set from bus, but writing 0 is a no-op
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
}

// --- Computed STATUS_BYTE with injected faults ---

#[test]
fn computed_status_byte_iout_bit() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 4), 0, "IOUT_OC should set STATUS_BYTE bit 4");
}

#[test]
fn computed_status_byte_input_bit() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0x10);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 3), 0, "INPUT should set STATUS_BYTE bit 3");
}

#[test]
fn computed_status_byte_temperature_bit() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_temperature(0x01);
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
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_cml(0x01);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 1), 0, "CML should set STATUS_BYTE bit 1");
}

#[test]
fn computed_status_byte_mfr_bit() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_mfr_specific(0x01);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & 1, 0, "MFR_SPECIFIC should set STATUS_BYTE bit 0");
}

// --- Computed STATUS_WORD high byte ---

#[test]
fn computed_status_word_vout_aggregation() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    assert_ne!(
        word & (1 << 15),
        0,
        "STATUS_VOUT should set STATUS_WORD bit 15"
    );
}

#[test]
fn computed_status_word_iout_aggregation() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
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
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
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
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
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

// --- CLEAR_FAULTS with injected faults ---

#[test]
fn clear_faults_clears_injected_faults() {
    let dev = Bmr491::new(Address::new(ADDR).unwrap());
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
