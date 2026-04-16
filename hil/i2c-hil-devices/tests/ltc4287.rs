use embedded_hal::i2c::I2c;

use i2c_hil_devices::Ltc4287;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x44).unwrap()
}

fn read_word_le(bus: &mut impl I2c<Error = BusError>, addr: u8, cmd: u8) -> u16 {
    let mut buf = [0u8; 2];
    bus.write_read(addr, &[cmd], &mut buf).unwrap();
    u16::from_le_bytes(buf)
}

fn read_byte(bus: &mut impl I2c<Error = BusError>, addr: u8, cmd: u8) -> u8 {
    let mut buf = [0u8; 1];
    bus.write_read(addr, &[cmd], &mut buf).unwrap();
    buf[0]
}

fn write_word_le(bus: &mut impl I2c<Error = BusError>, addr: u8, cmd: u8, val: u16) {
    let le = val.to_le_bytes();
    bus.write(addr, &[cmd, le[0], le[1]]).unwrap();
}

fn write_byte(bus: &mut impl I2c<Error = BusError>, addr: u8, cmd: u8, val: u8) {
    bus.write(addr, &[cmd, val]).unwrap();
}

// --- Default register values ---

#[test]
fn default_page_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x00), 0x00);
}

#[test]
fn default_operation() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x01), 0x00);
}

#[test]
fn default_capability() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x19), 0xD0);
}

#[test]
fn default_pmbus_revision() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x98), 0x33);
}

#[test]
fn default_mfr_special_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE7), 0x7020);
}

#[test]
fn default_mfr_config1() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xF2), 0x5572);
}

#[test]
fn default_mfr_config2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xF3), 0x00EF);
}

// --- Write and read back ---

#[test]
fn write_and_read_operation() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x01, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x01), 0x80);
}

#[test]
fn write_and_read_mfr_config1() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xF2, 0xABCD);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xF2), 0xABCD);
}

#[test]
fn word_byte_order_is_little_endian() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    // Write [cmd, 0x34, 0x12] — LE means value is 0x1234
    bus.write(0x44, &[0xB3, 0x34, 0x12]).unwrap();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xB3), 0x1234);
}

// --- CLEAR_FAULTS ---

#[test]
fn clear_faults_clears_all_status() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0xFF);
    dev.set_status_iout(0xFF);
    dev.set_status_input(0xFF);
    dev.set_status_temperature(0xFF);
    dev.set_status_cml(0xFF);
    dev.set_status_other(0xFF);
    dev.set_status_mfr_specific(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // Send CLEAR_FAULTS (send-byte: just the command code)
    bus.write(0x44, &[0x03]).unwrap();

    assert_eq!(read_byte(&mut bus, 0x44, 0x7A), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7B), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7C), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7D), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7E), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0x00);
}

// --- W1C status ---

#[test]
fn status_write_one_to_clear() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_iout(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // Write 0x80 to STATUS_IOUT — should clear bit 7 only
    write_byte(&mut bus, 0x44, 0x7B, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7B), 0x7F);
}

// --- Computed status ---

#[test]
fn status_byte_computed_from_sub_status() {
    let mut dev = Ltc4287::new(addr());
    // Set IOUT OC fault (status_iout bit 7) → STATUS_BYTE bit 4
    dev.set_status_iout(0x80);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sb = read_byte(&mut bus, 0x44, 0x78);
    // Bit 6 (OFF) should be set since OPERATION=0x00 (ON bit clear)
    // Bit 4 (IOUT_OC_FAULT) should be set
    assert_eq!(sb & (1 << 4), 1 << 4, "IOUT_OC_FAULT bit should be set");
    assert_eq!(sb & (1 << 6), 1 << 6, "OFF bit should be set");
}

#[test]
fn status_word_aggregates_sub_status() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0x80);
    dev.set_status_iout(0x80);
    dev.set_status_temperature(0x40);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    let low = sw as u8;

    // High byte: VOUT (bit 7), IOUT (bit 6)
    assert_ne!(high & 0x80, 0, "VOUT aggregated bit should be set");
    assert_ne!(high & 0x40, 0, "IOUT aggregated bit should be set");

    // Low byte: IOUT_OC_FAULT (bit 4), TEMPERATURE (bit 2), OFF (bit 6)
    assert_ne!(low & (1 << 4), 0, "IOUT_OC_FAULT in status_byte");
    assert_ne!(low & (1 << 2), 0, "TEMPERATURE in status_byte");
}

// --- Telemetry ---

#[test]
fn read_vin_returns_injected_value() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vin(1234);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x88), 1234);
}

#[test]
fn read_vout_returns_injected_value() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vout(5678);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x8B), 5678);
}

#[test]
fn read_iout_returns_injected_value() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_iout(9012);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x8C), 9012);
}

#[test]
fn read_temperature_returns_injected_value() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_temperature_1(3456);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x8D), 3456);
}

#[test]
fn read_pin_returns_injected_value() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_pin(7890);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x97), 7890);
}

// --- Block reads ---

#[test]
fn block_read_mfr_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let mut buf = [0u8; 4];
    bus.write_read(0x44, &[0x99], &mut buf).unwrap();
    assert_eq!(buf, [3, b'L', b'T', b'C']);
}

#[test]
fn block_read_mfr_model() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let mut buf = [0u8; 8];
    bus.write_read(0x44, &[0x9A], &mut buf).unwrap();
    assert_eq!(buf, [7, b'L', b'T', b'C', b'4', b'2', b'8', b'7']);
}

#[test]
fn block_read_ic_device_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let mut buf = [0u8; 8];
    bus.write_read(0x44, &[0xAD], &mut buf).unwrap();
    assert_eq!(buf, [7, b'L', b'T', b'C', b'4', b'2', b'8', b'7']);
}

// --- Read-only protection ---

#[test]
fn write_to_read_only_register_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Attempt to write to READ_VIN (0x88) — should NAK
    let result = bus.write(0x44, &[0x88, 0x34, 0x12]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn unknown_command_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Set pointer to an unused command, then try to read
    bus.write(0x44, &[0x30]).unwrap();
    let mut buf = [0u8; 1];
    let result = bus.read(0x44, &mut buf);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- User scratch ---

#[test]
fn user_scratch_write_and_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    write_word_le(&mut bus, 0x44, 0xB3, 0x1111);
    write_word_le(&mut bus, 0x44, 0xB4, 0x2222);
    write_word_le(&mut bus, 0x44, 0xB6, 0x3333);
    write_word_le(&mut bus, 0x44, 0xB7, 0x4444);

    assert_eq!(read_word_le(&mut bus, 0x44, 0xB3), 0x1111);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xB4), 0x2222);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xB6), 0x3333);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xB7), 0x4444);
}

// --- Write protect ---

#[test]
fn write_protect_wp1_blocks_writes() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Enable WP1 (bit 7)
    write_byte(&mut bus, 0x44, 0x10, 0x80);

    // OPERATION write should be blocked
    let result = bus.write(0x44, &[0x01, 0x80]);
    assert_eq!(result, Err(BusError::DataNak));

    // But WRITE_PROTECT itself should still be writable
    write_byte(&mut bus, 0x44, 0x10, 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x10), 0x00);
}

// --- Pointer persistence ---

#[test]
fn pointer_persists_across_reads() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vin(0xABCD);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // Set pointer to READ_VIN
    bus.write(0x44, &[0x88]).unwrap();

    // Read multiple times without setting pointer again
    let mut buf = [0u8; 2];
    bus.read(0x44, &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);

    bus.read(0x44, &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

// --- Extended command prefix ---

#[test]
fn extended_prefix_read_mfr_config1() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Write via extended prefix: [0xFE, cmd]
    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xF2], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5572);
}

#[test]
fn extended_prefix_write_and_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Write via extended prefix: [0xFE, cmd, low, high]
    let val: u16 = 0xDEAD;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xF2, le[0], le[1]]).unwrap();

    // Read back via extended prefix
    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xF2], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xDEAD);
}

// --- WP2 allows OPERATION but blocks other writes ---

#[test]
fn write_protect_wp2_allows_operation() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Enable WP2 (bit 6)
    write_byte(&mut bus, 0x44, 0x10, 0x40);

    // OPERATION should be allowed under WP2
    write_byte(&mut bus, 0x44, 0x01, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x01), 0x80);

    // But config writes should be blocked
    let result = bus.write(0x44, &[0xF2, 0x00, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- MFR_COMMON constant ---

#[test]
fn default_mfr_common() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xEF), 0xEE);
}

// --- Public getter methods ---

#[test]
fn read_vout_getter() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vout(0xCAFE);
    assert_eq!(dev.read_vout(), 0xCAFE);
}

#[test]
fn read_iout_getter() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_iout(0xBEEF);
    assert_eq!(dev.read_iout(), 0xBEEF);
}

#[test]
fn read_temperature_1_getter() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_temperature_1(0xDEAD);
    assert_eq!(dev.read_temperature_1(), 0xDEAD);
}

#[test]
fn read_pin_getter() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_pin(0xF00D);
    assert_eq!(dev.read_pin(), 0xF00D);
}

#[test]
fn read_vin_getter() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vin(0x1234);
    assert_eq!(dev.read_vin(), 0x1234);
}

#[test]
fn operation_getter() {
    let dev = Ltc4287::new(addr());
    assert_eq!(dev.operation(), 0x00);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x01, 0x80);

    // Re-read via bus to verify the state
    assert_eq!(read_byte(&mut bus, 0x44, 0x01), 0x80);
}

#[test]
fn write_protect_getter() {
    let dev = Ltc4287::new(addr());
    assert_eq!(dev.write_protect(), 0x00);
}

// --- STATUS_WORD W1C ---

#[test]
fn status_word_w1c() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0xFF);
    dev.set_status_iout(0xFF);
    dev.set_status_input(0xFF);
    dev.set_status_mfr_specific(0xFF);
    dev.set_status_other(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD: high byte 0x80 = clear VOUT, 0x40 = clear IOUT,
    // 0x20 = clear INPUT, 0x10 = clear MFR, 0x02 = clear OTHER
    let val: u16 = 0xF2_00; // high=0xF2, low=0x00
    let le = val.to_le_bytes();
    bus.write(0x44, &[0x79, le[0], le[1]]).unwrap();

    assert_eq!(read_byte(&mut bus, 0x44, 0x7A), 0x00, "VOUT cleared");
    assert_eq!(read_byte(&mut bus, 0x44, 0x7B), 0x00, "IOUT cleared");
    assert_eq!(read_byte(&mut bus, 0x44, 0x7C), 0x00, "INPUT cleared");
    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0x00, "MFR cleared");
    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x00, "OTHER cleared");
}

// --- STATUS_BYTE W1C cascade ---

#[test]
fn status_byte_w1c_cascade_iout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_iout(0x80);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 4 should cascade to clear STATUS_IOUT bit 7
    write_byte(&mut bus, 0x44, 0x78, 1 << 4);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7B) & 0x80, 0);
}

#[test]
fn status_byte_w1c_cascade_input() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0x10);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 3 should cascade to clear STATUS_INPUT bit 4
    write_byte(&mut bus, 0x44, 0x78, 1 << 3);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7C) & 0x10, 0);
}

#[test]
fn status_byte_w1c_cascade_temperature() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_temperature(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 2 should cascade to clear all STATUS_TEMPERATURE
    write_byte(&mut bus, 0x44, 0x78, 1 << 2);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7D), 0x00);
}

#[test]
fn status_byte_w1c_cascade_cml() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_cml(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 1 should cascade to clear all STATUS_CML
    write_byte(&mut bus, 0x44, 0x78, 1 << 1);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7E), 0x00);
}

#[test]
fn status_byte_w1c_cascade_other_and_mfr() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0xFF);
    dev.set_status_mfr_specific(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 0 should cascade to clear STATUS_OTHER and STATUS_MFR
    write_byte(&mut bus, 0x44, 0x78, 1);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x00, "OTHER cleared");
    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0x00, "MFR cleared");
}

// --- Extended prefix: just the prefix byte alone ---

#[test]
fn extended_prefix_alone_sets_mode() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Send just the 0xFE prefix, then the command byte separately
    bus.write(0x44, &[0xFE]).unwrap();
    bus.write(0x44, &[0xF2]).unwrap();
    let mut buf = [0u8; 2];
    bus.read(0x44, &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x5572);
}

// --- Computed STATUS_BYTE: individual branch coverage ---

#[test]
fn status_byte_input_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0x10); // VIN_UV bit
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & (1 << 3), 0, "VIN_UV should set STATUS_BYTE bit 3");
}

#[test]
fn status_byte_temperature_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_temperature(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & (1 << 2), 0, "TEMPERATURE should set STATUS_BYTE bit 2");
}

#[test]
fn status_byte_cml_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_cml(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & (1 << 1), 0, "CML should set STATUS_BYTE bit 1");
}

#[test]
fn status_byte_other_mfr_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & 1, 0, "OTHER should set STATUS_BYTE bit 0");

    // Also test with mfr_specific alone
    let mut dev2 = Ltc4287::new(addr());
    dev2.set_status_mfr_specific(0x01);
    let mut bus2 = SimBusBuilder::new().with_device(dev2).build();
    let sb2 = read_byte(&mut bus2, 0x44, 0x78);
    assert_ne!(sb2 & 1, 0, "MFR_SPECIFIC should set STATUS_BYTE bit 0");
}

// --- Computed STATUS_WORD: additional high byte coverage ---

#[test]
fn status_word_input_aggregation() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(high & (1 << 5), 0, "INPUT should set STATUS_WORD bit 13");
}

#[test]
fn status_word_mfr_aggregation() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_mfr_specific(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(high & (1 << 4), 0, "MFR should set STATUS_WORD bit 12");
}

#[test]
fn status_word_power_good_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0x18); // bits 3 and 4 set
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(
        high & (1 << 3),
        0,
        "POWER_GOOD# should set STATUS_WORD bit 11"
    );
}

#[test]
fn status_word_other_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0x01);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(high & (1 << 1), 0, "OTHER should set STATUS_WORD bit 9");
}

// --- Read register coverage: byte registers ---

#[test]
fn read_iout_oc_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x47), 0xC0);
}

#[test]
fn read_ot_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x50), 0x80);
}

#[test]
fn read_vin_ov_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x56), 0xB8);
}

#[test]
fn read_vin_uv_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x5A), 0xB8);
}

#[test]
fn read_status_other() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0xAA);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0xAA);
}

#[test]
fn read_mfr_flt_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xD2), 0x00);
}

#[test]
fn read_mfr_adc_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xD8), 0x01);
}

#[test]
fn read_mfr_avg_sel() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xD9), 0x85);
}

#[test]
fn read_mfr_loff() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xDC), 0x00);
}

#[test]
fn read_mfr_pmb_stat() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xE2), 0x00);
}

#[test]
fn read_mfr_sd_cause() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xF1), 0x00);
}

#[test]
fn read_mfr_reboot_control() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xFD), 0x00);
}

// --- Read register coverage: word registers ---

#[test]
fn read_word_limits() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0x42), 0x7FFF); // VOUT_OV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x43), 0x0000); // VOUT_UV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4A), 0x7FFF); // IOUT_OC_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4F), 0x7FFF); // OT_FAULT
    assert_eq!(read_word_le(&mut bus, 0x44, 0x51), 0x7FFF); // OT_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x52), 0x0000); // UT_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x57), 0x7FFF); // VIN_OV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x58), 0x0000); // VIN_UV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x6B), 0x7FFF); // PIN_OP_WARN
}

#[test]
fn read_mfr_op_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xD7), 0xFFE0);
}

#[test]
fn read_mfr_system_status() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE0), 0x0000);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE1), 0x0000);
}

#[test]
fn read_mfr_on_off_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xFC), 0x001D);
}

// --- Write register coverage: byte registers ---

#[test]
fn write_iout_oc_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x47, 0x55);
    assert_eq!(read_byte(&mut bus, 0x44, 0x47), 0x55);
}

#[test]
fn write_ot_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x50, 0x44);
    assert_eq!(read_byte(&mut bus, 0x44, 0x50), 0x44);
}

#[test]
fn write_vin_ov_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x56, 0x33);
    assert_eq!(read_byte(&mut bus, 0x44, 0x56), 0x33);
}

#[test]
fn write_vin_uv_fault_response() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x5A, 0x22);
    assert_eq!(read_byte(&mut bus, 0x44, 0x5A), 0x22);
}

#[test]
fn write_mfr_flt_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xD2, 0xAB);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD2), 0xAB);
}

#[test]
fn write_mfr_adc_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xD8, 0x77);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD8), 0x77);
}

#[test]
fn write_mfr_avg_sel() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xD9, 0x66);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD9), 0x66);
}

#[test]
fn write_mfr_reboot_control() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xFD, 0x11);
    assert_eq!(read_byte(&mut bus, 0x44, 0xFD), 0x11);
}

// --- Write register coverage: W1C byte ---

#[test]
fn w1c_status_vout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x7A, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7A), 0x7F);
}

#[test]
fn w1c_status_input() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x7C, 0x10);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7C), 0xEF);
}

#[test]
fn w1c_status_temperature() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_temperature(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x7D, 0x01);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7D), 0xFE);
}

#[test]
fn w1c_status_cml() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_cml(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x7E, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7E), 0x7F);
}

#[test]
fn w1c_status_other() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x7F, 0x80);
    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x7F);
}

#[test]
fn w1c_status_mfr_specific() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_mfr_specific(0xFF);
    let mut bus = SimBusBuilder::new().with_device(dev).build();
    write_byte(&mut bus, 0x44, 0x80, 0x0F);
    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0xF0);
}

#[test]
fn w1c_mfr_pmb_stat() {
    // mfr_pmb_stat is W1C byte
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    // POR is 0x00, writing 0xFF is a no-op
    write_byte(&mut bus, 0x44, 0xE2, 0xFF);
    assert_eq!(read_byte(&mut bus, 0x44, 0xE2), 0x00);
}

#[test]
fn w1c_mfr_loff() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xDC, 0xFF);
    assert_eq!(read_byte(&mut bus, 0x44, 0xDC), 0x00);
}

// --- Write register coverage: word registers ---

#[test]
fn write_word_limits() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x42, 0x1111); // VOUT_OV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x42), 0x1111);
    write_word_le(&mut bus, 0x44, 0x43, 0x2222); // VOUT_UV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x43), 0x2222);
    write_word_le(&mut bus, 0x44, 0x4A, 0x3333); // IOUT_OC_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4A), 0x3333);
    write_word_le(&mut bus, 0x44, 0x4F, 0x4444); // OT_FAULT
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4F), 0x4444);
    write_word_le(&mut bus, 0x44, 0x51, 0x5555); // OT_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x51), 0x5555);
    write_word_le(&mut bus, 0x44, 0x52, 0x6666); // UT_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x52), 0x6666);
    write_word_le(&mut bus, 0x44, 0x57, 0x7777); // VIN_OV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x57), 0x7777);
    write_word_le(&mut bus, 0x44, 0x58, 0x8888); // VIN_UV_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x58), 0x8888);
    write_word_le(&mut bus, 0x44, 0x6B, 0x9999); // PIN_OP_WARN
    assert_eq!(read_word_le(&mut bus, 0x44, 0x6B), 0x9999);
}

#[test]
fn write_mfr_op_fault_response_word() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xD7, 0xABCD);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xD7), 0xABCD);
}

#[test]
fn write_mfr_config2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xF3, 0x1234);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xF3), 0x1234);
}

#[test]
fn write_mfr_on_off_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xFC, 0x5678);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xFC), 0x5678);
}

#[test]
fn w1c_mfr_system_status1() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xE0, 0xFFFF);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE0), 0x0000);
}

#[test]
fn w1c_mfr_system_status2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xE1, 0xFFFF);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE1), 0x0000);
}

// --- Read-only register writes fail ---

#[test]
fn write_to_capability_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0x19, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn write_to_pmbus_revision_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0x98, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn write_to_mfr_common_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xEF, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn write_to_mfr_special_id_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xE7, 0x00, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Extended prefix: write byte register ---

#[test]
fn extended_write_reboot_control() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    // Extended write: [0xFE, cmd, data]
    bus.write(0x44, &[0xFE, 0xFD, 0x42]).unwrap();
    // Read back via extended
    let mut buf = [0u8; 1];
    bus.write_read(0x44, &[0xFE, 0xFD], &mut buf).unwrap();
    assert_eq!(buf[0], 0x42);
}

#[test]
fn extended_write_on_off_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let val: u16 = 0x9999;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xFC, le[0], le[1]]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xFC], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x9999);
}

#[test]
fn extended_write_config2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let val: u16 = 0xBEEF;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xF3, le[0], le[1]]).unwrap();
    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xF3], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xBEEF);
}

#[test]
fn extended_read_unknown_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    // Unknown extended register should NAK on read
    bus.write(0x44, &[0xFE, 0x01]).unwrap(); // set pointer to unknown extended
    let mut buf = [0u8; 1];
    let result = bus.read(0x44, &mut buf);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn extended_write_unknown_byte_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xFE, 0x01, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn extended_write_unknown_word_fails() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xFE, 0x01, 0x00, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- MFR revision block read ---

#[test]
fn block_read_mfr_revision() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0x9B], &mut buf).unwrap();
    assert_eq!(buf[0], 1); // length
    assert_eq!(buf[1], 0x11);
}

#[test]
fn block_read_ic_device_rev() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xAE], &mut buf).unwrap();
    assert_eq!(buf[0], 1); // length
    assert_eq!(buf[1], 0x11);
}

// --- ON state with OPERATION=0x80: all false paths ---

#[test]
fn status_byte_on_no_faults() {
    // No faults, but OPERATION defaults to 0x00 so OFF bit is set.
    // We need to write OPERATION=0x80 to clear OFF bit.
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x01, 0x80);

    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_eq!(sb, 0x00, "STATUS_BYTE should be 0 when ON and no faults");
}

#[test]
fn status_word_on_no_faults() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x01, 0x80);

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    assert_eq!(sw, 0x0000, "STATUS_WORD should be 0 when ON and no faults");
}

// --- Extended register write/read for MFR_CONFIG2 ---

#[test]
fn extended_write_read_mfr_config2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let val: u16 = 0x1234;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xF3, le[0], le[1]]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xF3], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

// --- Extended register write/read for MFR_ON_OFF_CONFIG ---

#[test]
fn extended_write_read_mfr_on_off_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let val: u16 = 0xABCD;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xFC, le[0], le[1]]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xFC], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xABCD);
}

// --- Extended register write/read for MFR_REBOOT_CONTROL (byte) ---

#[test]
fn extended_write_read_mfr_reboot_control() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    bus.write(0x44, &[0xFE, 0xFD, 0x42]).unwrap();
    assert_eq!(read_byte(&mut bus, 0x44, 0xFD), 0x42);
}

// --- Extended read for unknown register returns NAK ---

#[test]
fn extended_read_unknown_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    bus.write(0x44, &[0xFE, 0x30]).unwrap();
    let mut buf = [0u8; 1];
    let result = bus.read(0x44, &mut buf);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Extended write for unknown byte register returns NAK ---

#[test]
fn extended_write_unknown_byte_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xFE, 0x30, 0x01]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Extended write for unknown word register returns NAK ---

#[test]
fn extended_write_unknown_word_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xFE, 0x30, 0x01, 0x02]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Write byte register tests for all writable byte registers ---

#[test]
fn write_fault_response_registers() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // IOUT_OC_FAULT_RESPONSE
    write_byte(&mut bus, 0x44, 0x47, 0x55);
    assert_eq!(read_byte(&mut bus, 0x44, 0x47), 0x55);

    // OT_FAULT_RESPONSE
    write_byte(&mut bus, 0x44, 0x50, 0x66);
    assert_eq!(read_byte(&mut bus, 0x44, 0x50), 0x66);

    // VIN_OV_FAULT_RESPONSE
    write_byte(&mut bus, 0x44, 0x56, 0x77);
    assert_eq!(read_byte(&mut bus, 0x44, 0x56), 0x77);

    // VIN_UV_FAULT_RESPONSE
    write_byte(&mut bus, 0x44, 0x5A, 0x88);
    assert_eq!(read_byte(&mut bus, 0x44, 0x5A), 0x88);
}

#[test]
fn write_mfr_byte_registers() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // MFR_FLT_CONFIG
    write_byte(&mut bus, 0x44, 0xD2, 0x33);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD2), 0x33);

    // MFR_ADC_CONFIG
    write_byte(&mut bus, 0x44, 0xD8, 0x44);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD8), 0x44);

    // MFR_AVG_SEL
    write_byte(&mut bus, 0x44, 0xD9, 0x55);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD9), 0x55);

    // MFR_REBOOT_CONTROL
    write_byte(&mut bus, 0x44, 0xFD, 0x66);
    assert_eq!(read_byte(&mut bus, 0x44, 0xFD), 0x66);
}

// --- Write word limit registers ---

#[test]
fn write_word_limit_registers() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // UT_WARN_LIMIT
    write_word_le(&mut bus, 0x44, 0x52, 0x1111);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x52), 0x1111);

    // PIN_OP_WARN_LIMIT
    write_word_le(&mut bus, 0x44, 0x6B, 0x2222);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x6B), 0x2222);

    // MFR_OP_FAULT_RESPONSE
    write_word_le(&mut bus, 0x44, 0xD7, 0x3333);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xD7), 0x3333);
}

// --- Read-only byte register writes return NAK ---

#[test]
fn write_to_capability_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0x19, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn write_to_mfr_common_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xEF, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Read-only word register writes return NAK ---

#[test]
fn write_to_mfr_special_id_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xE7, 0x00, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Empty write is ok ---

#[test]
fn empty_write_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    // Write with data.len() == 0 via operations
    use embedded_hal::i2c::I2c;
    use embedded_hal::i2c::Operation;
    let result = bus.transaction(0x44, &mut [Operation::Write(&[])]);
    assert!(result.is_ok());
}
