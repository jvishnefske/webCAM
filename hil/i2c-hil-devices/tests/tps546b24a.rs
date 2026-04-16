//! Integration tests for the TPS546B24A buck converter simulation.

use embedded_hal::i2c::I2c;
use i2c_hil_devices::Tps546b24a;
use i2c_hil_sim::{Address, PmBusEngine, SimBusBuilder};

const ADDR: u8 = 0x24;

fn make_bus() -> i2c_hil_sim::SimBus<(PmBusEngine<Tps546b24a>, ())> {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let mut buf = [0u8; 3];
    bus.write_read(ADDR, &[0x99], &mut buf).unwrap();
    assert_eq!(buf[0], 2); // length
    assert_eq!(&buf[1..3], b"TI");
}

#[test]
fn read_mfr_model_block() {
    let mut bus = make_bus();
    let mut buf = [0u8; 11];
    bus.write_read(ADDR, &[0x9A], &mut buf).unwrap();
    assert_eq!(buf[0], 10); // length
    assert_eq!(&buf[1..11], b"TPS546B24A");
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

#[test]
fn default_vout_mode() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x20], &mut buf).unwrap();
    assert_eq!(buf[0], 0x17);
}

#[test]
fn default_vout_command() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x21], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x0199);
}

// --- Telemetry injection ---

#[test]
fn telemetry_vin() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vin(0xABCD);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x88], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn telemetry_vout() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vout(0x1234);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8B], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

#[test]
fn telemetry_iout() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_iout(0x5678);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8C], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
}

#[test]
fn telemetry_temperature() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_temperature_1(0xBEEF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x8D], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xBEEF);
}

// --- Computed status ---

#[test]
fn computed_status_byte_off_bit() {
    // OPERATION=0x80 (ON), so OFF bit (bit 6) should NOT be set
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
    let mut bus = make_bus();
    let mut buf = [0u8; 2];

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
    bus.write(ADDR, &[0x03]).unwrap();
}

// --- Write protect ---

#[test]
fn write_protect_wp1() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x10, 0x80]).unwrap();
    assert!(bus.write(ADDR, &[0x01, 0x42]).is_err());
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
fn limit_vout_ov_warn_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x42, 0xCD, 0xAB]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x42], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

#[test]
fn limit_vin_ov_warn_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x57, 0x78, 0x56]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x57], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5678);
}

#[test]
fn vout_command_read_write() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x21, 0x34, 0x12]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x21], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

// --- Computed status ---

#[test]
fn computed_status_byte_defaults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
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
fn clear_faults_clears_status() {
    let mut bus = make_bus();
    bus.write(ADDR, &[0x03]).unwrap();
}

// --- Status W1C ---

#[test]
fn status_vout_w1c() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7A], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00);
}

// --- Computed STATUS_BYTE with injected faults ---

#[test]
fn computed_status_byte_input_bit() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_input(0x10);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 3), 0, "INPUT should set STATUS_BYTE bit 3");
}

#[test]
fn computed_status_byte_iout_bit() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 4), 0, "IOUT_OC should set STATUS_BYTE bit 4");
}

#[test]
fn computed_status_byte_temperature_bit() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_cml(0x01);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_ne!(buf[0] & (1 << 1), 0, "CML should set STATUS_BYTE bit 1");
}

#[test]
fn computed_status_byte_mfr_bit() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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

// --- STATUS_BYTE W1C cascade ---

#[test]
fn status_byte_w1c_cascade_iout() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_iout(0x80);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1 << 4]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x7B], &mut buf).unwrap();
    assert_eq!(buf[0] & 0x80, 0, "STATUS_IOUT bit 7 should be cleared");
}

#[test]
fn status_byte_w1c_cascade_input() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
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
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_mfr_specific(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    bus.write(ADDR, &[0x78, 1]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x80], &mut buf).unwrap();
    assert_eq!(buf[0], 0x00, "STATUS_MFR_SPECIFIC should be cleared");
}

// --- ON state: no OFF bit, no faults ---

#[test]
fn computed_status_byte_on_no_faults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 1];
    bus.write_read(ADDR, &[0x78], &mut buf).unwrap();
    assert_eq!(
        buf[0], 0x00,
        "STATUS_BYTE should be 0 when ON and no faults"
    );
}

#[test]
fn computed_status_word_on_no_faults() {
    let mut bus = make_bus();
    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    assert_eq!(
        u16::from_le_bytes(buf),
        0x0000,
        "STATUS_WORD should be 0 when ON and no faults"
    );
}

#[test]
fn telemetry_setters_via_bus() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_read_vin(0x1234);
    engine.device_mut().set_read_vout(0x5678);
    engine.device_mut().set_read_iout(0x9ABC);
    engine.device_mut().set_read_temperature_1(0xDEF0);
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
}

// --- Additional computed status coverage ---

#[test]
fn computed_status_word_all_faults() {
    let dev = Tps546b24a::new(Address::new(ADDR).unwrap());
    let mut engine = PmBusEngine::new(dev);
    engine.device_mut().set_status_vout(0xFF);
    engine.device_mut().set_status_iout(0xFF);
    engine.device_mut().set_status_input(0xFF);
    engine.device_mut().set_status_mfr_specific(0xFF);
    engine.device_mut().set_status_temperature(0xFF);
    engine.device_mut().set_status_cml(0xFF);
    let mut bus = SimBusBuilder::new().with_device(engine).build();

    let mut buf = [0u8; 2];
    bus.write_read(ADDR, &[0x79], &mut buf).unwrap();
    let word = u16::from_le_bytes(buf);
    // All high byte bits should be set
    assert_ne!(word & (1 << 15), 0, "VOUT bit");
    assert_ne!(word & (1 << 14), 0, "IOUT bit");
    assert_ne!(word & (1 << 13), 0, "INPUT bit");
    assert_ne!(word & (1 << 12), 0, "MFR bit");
    // All low byte bits should be set too
    assert_ne!(word & (1 << 4), 0, "IOUT_OC bit");
    assert_ne!(word & (1 << 3), 0, "VIN_UV bit");
    assert_ne!(word & (1 << 2), 0, "TEMP bit");
    assert_ne!(word & (1 << 1), 0, "CML bit");
    assert_ne!(word & 1, 0, "MFR_SPECIFIC bit");
}
