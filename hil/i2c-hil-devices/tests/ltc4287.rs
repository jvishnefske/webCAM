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

// --- Getter methods ---

#[test]
fn getter_read_vin() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vin(0x1234);
    assert_eq!(dev.read_vin(), 0x1234);
}

#[test]
fn getter_read_vout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_vout(0x5678);
    assert_eq!(dev.read_vout(), 0x5678);
}

#[test]
fn getter_read_iout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_iout(0x9ABC);
    assert_eq!(dev.read_iout(), 0x9ABC);
}

#[test]
fn getter_read_temperature_1() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_temperature_1(0xDEF0);
    assert_eq!(dev.read_temperature_1(), 0xDEF0);
}

#[test]
fn getter_read_pin() {
    let mut dev = Ltc4287::new(addr());
    dev.set_read_pin(0xFACE);
    assert_eq!(dev.read_pin(), 0xFACE);
}

#[test]
fn getter_operation() {
    let dev = Ltc4287::new(addr());
    assert_eq!(dev.operation(), 0x00);
}

#[test]
fn getter_write_protect() {
    let dev = Ltc4287::new(addr());
    assert_eq!(dev.write_protect(), 0x00);
}

// --- STATUS_BYTE W1C cascade ---

#[test]
fn status_byte_w1c_cascade_iout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_iout(0x80);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 4 should cascade to clear status_iout bit 7
    write_byte(&mut bus, 0x44, 0x78, 1 << 4);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7B) & 0x80, 0);
}

#[test]
fn status_byte_w1c_cascade_input() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0x10);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 3 should cascade to clear status_input bit 4
    write_byte(&mut bus, 0x44, 0x78, 1 << 3);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7C) & 0x10, 0);
}

#[test]
fn status_byte_w1c_cascade_temperature() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_temperature(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 2 should cascade to clear all status_temperature
    write_byte(&mut bus, 0x44, 0x78, 1 << 2);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7D), 0x00);
}

#[test]
fn status_byte_w1c_cascade_cml() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_cml(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 1 should cascade to clear all status_cml
    write_byte(&mut bus, 0x44, 0x78, 1 << 1);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7E), 0x00);
}

#[test]
fn status_byte_w1c_cascade_other_and_mfr() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0xFF);
    dev.set_status_mfr_specific(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_BYTE bit 0 should cascade to clear status_other and status_mfr_specific
    write_byte(&mut bus, 0x44, 0x78, 1);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x00);
    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0x00);
}

// --- STATUS_WORD W1C cascade ---

#[test]
fn status_word_w1c_cascade_clears_vout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD high bit 7 (bit 15) should cascade to clear status_vout
    write_word_le(&mut bus, 0x44, 0x79, 0x8000);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7A), 0x00);
}

#[test]
fn status_word_w1c_cascade_clears_iout() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_iout(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD high bit 6 (bit 14) should cascade to clear status_iout
    write_word_le(&mut bus, 0x44, 0x79, 0x4000);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7B), 0x00);
}

#[test]
fn status_word_w1c_cascade_clears_input() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD high bit 5 (bit 13) should cascade to clear status_input
    write_word_le(&mut bus, 0x44, 0x79, 0x2000);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7C), 0x00);
}

#[test]
fn status_word_w1c_cascade_clears_mfr() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_mfr_specific(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD high bit 4 (bit 12) should cascade to clear status_mfr_specific
    write_word_le(&mut bus, 0x44, 0x79, 0x1000);

    assert_eq!(read_byte(&mut bus, 0x44, 0x80), 0x00);
}

#[test]
fn status_word_w1c_cascade_clears_other() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0xFF);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // W1C STATUS_WORD high bit 1 (bit 9) should cascade to clear status_other
    write_word_le(&mut bus, 0x44, 0x79, 0x0200);

    assert_eq!(read_byte(&mut bus, 0x44, 0x7F), 0x00);
}

// --- Extended prefix byte write ---

#[test]
fn extended_prefix_write_reboot_control() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Write MFR_REBOOT_CONTROL via extended prefix: [0xFE, 0xFD, val]
    bus.write(0x44, &[0xFE, 0xFD, 0x42]).unwrap();

    // Read back via extended prefix
    let mut buf = [0u8; 1];
    bus.write_read(0x44, &[0xFE, 0xFD], &mut buf).unwrap();
    assert_eq!(buf[0], 0x42);
}

// --- Extended prefix for on_off_config ---

#[test]
fn extended_prefix_write_on_off_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let val: u16 = 0x1234;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xFC, le[0], le[1]]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xFC], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0x1234);
}

// --- MFR W1C word registers ---

#[test]
fn mfr_system_status1_w1c() {
    // MFR_SYSTEM_STATUS1 is at 0xE0 — can be cleared via W1C
    // Since we cannot inject bits directly, verify default is 0
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE0), 0x0000);
}

#[test]
fn mfr_system_status2_w1c() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_word_le(&mut bus, 0x44, 0xE1), 0x0000);
}

// --- STATUS_BYTE computed with all sub-status bits ---

#[test]
fn status_byte_input_fault_bit() {
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
fn status_byte_mfr_and_other_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_mfr_specific(0x01);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & 1, 0, "MFR_SPECIFIC should set STATUS_BYTE bit 0");
}

// --- Extended prefix: just the prefix alone ---

#[test]
fn extended_prefix_alone_sets_mode() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    // Sending just the 0xFE prefix byte should not cause an error
    bus.write(0x44, &[0xFE]).unwrap();
}

// --- Warning limit write/read ---

#[test]
fn warning_limit_vout_ov_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x42, 0x1234);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x42), 0x1234);
}

#[test]
fn warning_limit_vout_uv_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x43, 0x5678);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x43), 0x5678);
}

#[test]
fn warning_limit_iout_oc_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x4A, 0xABCD);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4A), 0xABCD);
}

#[test]
fn warning_limit_ot_fault_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x4F, 0x1111);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x4F), 0x1111);
}

#[test]
fn warning_limit_ot_warn_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x51, 0x2222);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x51), 0x2222);
}

#[test]
fn warning_limit_ut_warn_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x52, 0x3333);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x52), 0x3333);
}

#[test]
fn warning_limit_vin_ov_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x57, 0x4444);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x57), 0x4444);
}

#[test]
fn warning_limit_vin_uv_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x58, 0x5555);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x58), 0x5555);
}

#[test]
fn warning_limit_pin_op_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0x6B, 0x6666);
    assert_eq!(read_word_le(&mut bus, 0x44, 0x6B), 0x6666);
}

// --- Fault response write/read ---

#[test]
fn fault_response_iout_oc_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x47, 0xAA);
    assert_eq!(read_byte(&mut bus, 0x44, 0x47), 0xAA);
}

#[test]
fn fault_response_ot_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x50, 0xBB);
    assert_eq!(read_byte(&mut bus, 0x44, 0x50), 0xBB);
}

#[test]
fn fault_response_vin_ov_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x56, 0xCC);
    assert_eq!(read_byte(&mut bus, 0x44, 0x56), 0xCC);
}

#[test]
fn fault_response_vin_uv_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0x5A, 0xDD);
    assert_eq!(read_byte(&mut bus, 0x44, 0x5A), 0xDD);
}

// --- MFR word config write/read ---

#[test]
fn mfr_op_fault_response_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xD7, 0x1234);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xD7), 0x1234);
}

#[test]
fn mfr_on_off_config_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_word_le(&mut bus, 0x44, 0xFC, 0xBEEF);
    assert_eq!(read_word_le(&mut bus, 0x44, 0xFC), 0xBEEF);
}

// --- MFR byte registers ---

#[test]
fn mfr_flt_config_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xD2, 0x55);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD2), 0x55);
}

#[test]
fn mfr_loff_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xDC), 0x00);
}

#[test]
fn mfr_pmb_stat_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xE2), 0x00);
}

#[test]
fn mfr_sd_cause_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    assert_eq!(read_byte(&mut bus, 0x44, 0xF1), 0x00);
}

#[test]
fn mfr_reboot_control_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    write_byte(&mut bus, 0x44, 0xFD, 0x99);
    assert_eq!(read_byte(&mut bus, 0x44, 0xFD), 0x99);
}

// --- STATUS_BYTE: other bit alone (no mfr) ---

#[test]
fn status_byte_other_bit_alone() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0x01);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sb = read_byte(&mut bus, 0x44, 0x78);
    assert_ne!(sb & 1, 0, "STATUS_OTHER alone should set STATUS_BYTE bit 0");
}

// --- STATUS_WORD: OTHER aggregation (bit 9) ---

#[test]
fn status_word_other_aggregation_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_other(0x01);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(
        high & (1 << 1),
        0,
        "STATUS_OTHER should set STATUS_WORD bit 9"
    );
}

// --- STATUS_WORD: INPUT aggregation (bit 13) ---

#[test]
fn status_word_input_aggregation_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_input(0x10);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(
        high & (1 << 5),
        0,
        "STATUS_INPUT should set STATUS_WORD bit 13"
    );
}

// --- STATUS_WORD: MFR aggregation (bit 12) ---

#[test]
fn status_word_mfr_aggregation_bit() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_mfr_specific(0x01);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(
        high & (1 << 4),
        0,
        "STATUS_MFR should set STATUS_WORD bit 12"
    );
}

// --- STATUS_WORD: POWER_GOOD via status_vout bit 3 ---

#[test]
fn status_word_power_good_via_vout_bit3() {
    let mut dev = Ltc4287::new(addr());
    dev.set_status_vout(0x08); // bit 3

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(
        high & (1 << 3),
        0,
        "POWER_GOOD# should be set via status_vout bit 3"
    );
}

// --- Extended prefix config2 write ---

#[test]
fn extended_prefix_write_config2() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let val: u16 = 0xCAFE;
    let le = val.to_le_bytes();
    bus.write(0x44, &[0xFE, 0xF3, le[0], le[1]]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x44, &[0xFE, 0xF3], &mut buf).unwrap();
    assert_eq!(u16::from_le_bytes(buf), 0xCAFE);
}

// --- STATUS_WORD POWER_GOOD aggregation ---

#[test]
fn status_word_power_good_bit() {
    let mut dev = Ltc4287::new(addr());
    // status_vout bit 4 (UV) sets POWER_GOOD# (high byte bit 3)
    dev.set_status_vout(0x10);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    let sw = read_word_le(&mut bus, 0x44, 0x79);
    let high = (sw >> 8) as u8;
    assert_ne!(high & (1 << 3), 0, "POWER_GOOD# should be set");
}

// --- Read-only write rejection ---

#[test]
fn write_to_capability_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0x19, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn write_to_mfr_special_id_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();
    let result = bus.write(0x44, &[0xE7, 0x00, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Extended prefix unknown command rejection ---

#[test]
fn extended_prefix_unknown_read_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    bus.write(0x44, &[0xFE, 0x01]).unwrap();
    let mut buf = [0u8; 1];
    let result = bus.read(0x44, &mut buf);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn extended_prefix_unknown_write_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    let result = bus.write(0x44, &[0xFE, 0x01, 0x00]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- MFR config byte write/read ---

#[test]
fn mfr_adc_config_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    write_byte(&mut bus, 0x44, 0xD8, 0x42);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD8), 0x42);
}

#[test]
fn mfr_avg_sel_write_read() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ltc4287::new(addr()))
        .build();

    write_byte(&mut bus, 0x44, 0xD9, 0x77);
    assert_eq!(read_byte(&mut bus, 0x44, 0xD9), 0x77);
}

// --- Block read MFR_REVISION ---

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

// --- Block read IC_DEVICE_REV ---

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
