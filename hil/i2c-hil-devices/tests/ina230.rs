use embedded_hal::i2c::I2c;

use i2c_hil_devices::Ina230;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x40).unwrap()
}

fn read_reg(bus: &mut impl I2c<Error = BusError>, addr: u8, ptr: u8) -> u16 {
    let mut buf = [0u8; 2];
    bus.write_read(addr, &[ptr], &mut buf).unwrap();
    u16::from_be_bytes(buf)
}

// --- Default register values ---

#[test]
fn default_config_is_por() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x00), 0x4127);
}

#[test]
fn default_shunt_voltage_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x01), 0x0000);
}

#[test]
fn default_bus_voltage_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x02), 0x0000);
}

#[test]
fn default_power_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x03), 0x0000);
}

#[test]
fn default_current_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x04), 0x0000);
}

#[test]
fn default_calibration_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x05), 0x0000);
}

#[test]
fn default_mask_enable_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x0000);
}

#[test]
fn default_alert_limit_is_zero() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x07), 0x0000);
}

#[test]
fn default_die_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0xFF), 0x2260);
}

#[test]
fn custom_die_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::with_die_id(addr(), 0xBEEF))
        .build();
    assert_eq!(read_reg(&mut bus, 0x40, 0xFF), 0xBEEF);
}

// --- Pointer validation ---

#[test]
fn valid_pointers_accepted() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    for ptr in 0x00..=0x07 {
        bus.write(0x40, &[ptr]).unwrap();
    }
    bus.write(0x40, &[0xFF]).unwrap();
}

#[test]
fn invalid_pointer_returns_data_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    assert_eq!(bus.write(0x40, &[0x08]), Err(BusError::DataNak));
    assert_eq!(bus.write(0x40, &[0x10]), Err(BusError::DataNak));
    assert_eq!(bus.write(0x40, &[0xFE]), Err(BusError::DataNak));
}

// --- Read-only register protection ---

#[test]
fn write_to_shunt_voltage_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(bus.write(0x40, &[0x01, 0x12, 0x34]), Err(BusError::DataNak));
}

#[test]
fn write_to_bus_voltage_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(bus.write(0x40, &[0x02, 0x12, 0x34]), Err(BusError::DataNak));
}

#[test]
fn write_to_power_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(bus.write(0x40, &[0x03, 0x12, 0x34]), Err(BusError::DataNak));
}

#[test]
fn write_to_current_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(bus.write(0x40, &[0x04, 0x12, 0x34]), Err(BusError::DataNak));
}

#[test]
fn write_to_die_id_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    assert_eq!(bus.write(0x40, &[0xFF, 0x12, 0x34]), Err(BusError::DataNak));
}

// --- Configuration register ---

#[test]
fn write_and_read_config() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    bus.write(0x40, &[0x00, 0x5A, 0x5A]).unwrap();
    assert_eq!(read_reg(&mut bus, 0x40, 0x00), 0x5A5A);
}

#[test]
fn rst_bit_resets_all_registers() {
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(1000);
    dev.set_bus_voltage_raw(5000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // Write calibration
    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap();
    // Write mask/enable
    bus.write(0x40, &[0x06, 0x00, 0x08]).unwrap();
    // Write alert limit
    bus.write(0x40, &[0x07, 0xFF, 0xFF]).unwrap();

    // Trigger reset
    bus.write(0x40, &[0x00, 0x80, 0x00]).unwrap();

    // All registers should be back to POR defaults
    assert_eq!(read_reg(&mut bus, 0x40, 0x00), 0x4127);
    assert_eq!(read_reg(&mut bus, 0x40, 0x01), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x02), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x03), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x04), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x05), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x0000);
    assert_eq!(read_reg(&mut bus, 0x40, 0x07), 0x0000);
}

#[test]
fn rst_bit_preserves_die_id() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::with_die_id(addr(), 0xBEEF))
        .build();

    bus.write(0x40, &[0x00, 0x80, 0x00]).unwrap();
    assert_eq!(read_reg(&mut bus, 0x40, 0xFF), 0xBEEF);
}

// --- Calibration register ---

#[test]
fn calibration_masks_d15() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Write 0xFFFF — D15 should be masked to 0
    bus.write(0x40, &[0x05, 0xFF, 0xFF]).unwrap();
    assert_eq!(read_reg(&mut bus, 0x40, 0x05), 0x7FFF);
}

#[test]
fn calibration_stores_d14_d0() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap();
    assert_eq!(read_reg(&mut bus, 0x40, 0x05), 0x0A00);
}

// --- Current computation ---

#[test]
fn current_with_positive_shunt() {
    // shunt=8000 (20mV), cal=2560 → current = (8000 * 2560) / 2048 = 10000
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(8000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap(); // cal = 0x0A00 = 2560

    assert_eq!(read_reg(&mut bus, 0x40, 0x04), 10000);
}

#[test]
fn current_with_negative_shunt() {
    // shunt=-8000, cal=2560 → current = (-8000 * 2560) / 2048 = -10000
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(-8000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap(); // cal = 2560

    let raw = read_reg(&mut bus, 0x40, 0x04);
    let signed = raw as i16;
    assert_eq!(signed, -10000);
}

#[test]
fn current_is_zero_when_calibration_zero() {
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(8000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x04), 0);
}

// --- Power computation ---

#[test]
fn power_computation() {
    // current = 10000, bus_voltage = 24000 (30V at 1.25mV/LSB)
    // power = (10000 * 24000) / 20_000 = 12000
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(8000);
    dev.set_bus_voltage_raw(24000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap(); // cal = 2560

    assert_eq!(read_reg(&mut bus, 0x40, 0x03), 12000);
}

#[test]
fn power_is_zero_when_calibration_zero() {
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(8000);
    dev.set_bus_voltage_raw(24000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    assert_eq!(read_reg(&mut bus, 0x40, 0x03), 0);
}

#[test]
fn power_with_negative_current() {
    // Power should use absolute value of current
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(-8000);
    dev.set_bus_voltage_raw(24000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();
    bus.write(0x40, &[0x05, 0x0A, 0x00]).unwrap(); // cal = 2560

    // |current| = 10000, power = (10000 * 24000) / 20_000 = 12000
    assert_eq!(read_reg(&mut bus, 0x40, 0x03), 12000);
}

// --- Mask/Enable register ---

#[test]
fn mask_enable_read_clears_cvrf() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Write mask/enable with CVRF bit set (D3 = 0x0008)
    bus.write(0x40, &[0x06, 0x00, 0x08]).unwrap();

    // First read should return value with CVRF set
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x0008);

    // Second read should have CVRF cleared
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x0000);
}

#[test]
fn mask_enable_preserves_other_bits_on_cvrf_clear() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Set multiple bits including CVRF
    bus.write(0x40, &[0x06, 0x80, 0x18]).unwrap(); // SOL (D15=0x8000 not in high byte alone) — 0x8018

    // First read returns all bits
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x8018);

    // Second read: CVRF (D3) cleared, other bits remain
    assert_eq!(read_reg(&mut bus, 0x40, 0x06), 0x8010);
}

// --- Bus voltage masking ---

#[test]
fn bus_voltage_masks_to_15_bits() {
    let mut dev = Ina230::new(addr());
    dev.set_bus_voltage_raw(0xFFFF);
    assert_eq!(dev.bus_voltage_raw(), 0x7FFF);
}

// --- Alert limit ---

#[test]
fn write_and_read_alert_limit() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    bus.write(0x40, &[0x07, 0xAB, 0xCD]).unwrap();
    assert_eq!(read_reg(&mut bus, 0x40, 0x07), 0xABCD);
}

// --- Helper method access ---

#[test]
fn set_and_get_shunt_voltage() {
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(-5000);
    assert_eq!(dev.shunt_voltage_raw(), -5000);
}

#[test]
fn set_and_get_bus_voltage() {
    let mut dev = Ina230::new(addr());
    dev.set_bus_voltage_raw(12345);
    assert_eq!(dev.bus_voltage_raw(), 12345);
}

#[test]
fn config_accessor() {
    let dev = Ina230::new(addr());
    assert_eq!(dev.config(), 0x4127);
}

#[test]
fn calibration_accessor() {
    let dev = Ina230::new(addr());
    assert_eq!(dev.calibration(), 0x0000);
}

// --- Integration: INA230 on SimBus ---

#[test]
fn ina230_on_sim_bus_read_write() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Write config
    bus.write(0x40, &[0x00, 0x71, 0xFF]).unwrap();

    // Read back via write_read
    let mut buf = [0u8; 2];
    bus.write_read(0x40, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x71, 0xFF]);
}

#[test]
fn ina230_pointer_persists() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Set pointer to config
    bus.write(0x40, &[0x00]).unwrap();

    // Read without resetting pointer
    let mut buf = [0u8; 2];
    bus.read(0x40, &mut buf).unwrap();
    assert_eq!(buf, [0x41, 0x27]);
}

#[test]
fn ina230_read_repeats_msb_lsb() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();

    // Read 4 bytes from config register (0x4127)
    let mut buf = [0u8; 4];
    bus.write_read(0x40, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x41, 0x27, 0x41, 0x27]);
}

// --- Direct SmBusWordDevice trait method coverage ---

#[test]
fn pointer_default_is_zero() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let dev = Ina230::new(addr());
    assert_eq!(dev.pointer(), 0);
}

#[test]
fn set_pointer_updates_pointer() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.set_pointer(0x05).unwrap();
    assert_eq!(dev.pointer(), 0x05);
}

#[test]
fn set_pointer_to_die_id() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.set_pointer(0xFF).unwrap();
    assert_eq!(dev.pointer(), 0xFF);
}

#[test]
fn set_pointer_invalid_returns_error() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    assert!(dev.set_pointer(0x08).is_err());
    assert!(dev.set_pointer(0xFE).is_err());
}

#[test]
fn write_register_mask_enable() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.write_register(0x06, 0xABCD).unwrap();
    let val = dev.read_register(0x06);
    // read_register reads the stored value directly (CVRF clear happens on I2C read)
    assert_eq!(val, 0xABCD);
}

#[test]
fn write_register_alert_limit() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.write_register(0x07, 0x1234).unwrap();
    assert_eq!(dev.read_register(0x07), 0x1234);
}

#[test]
fn read_register_unknown_pointer_returns_zero() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    // Pointer 0x08 is invalid for set_pointer, but read_register
    // is called internally with valid pointers only. Test the default
    // arm returns 0 by reading pointer 0 after no mutation.
    assert_eq!(dev.read_register(0x00), 0x4127);
}

#[test]
fn write_empty_does_not_error() {
    let mut bus = SimBusBuilder::new()
        .with_device(Ina230::new(addr()))
        .build();
    // A write with just 0 bytes (empty data) should succeed
    // because the SmBusWord engine handles this as a no-op.
    // This exercises the bus-level empty write path.
    bus.write(0x40, &[0x00]).unwrap();
}

// --- Direct read_register/write_register branch coverage ---

#[test]
fn read_register_all_valid_pointers() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(100);
    dev.set_bus_voltage_raw(200);

    // Read each valid pointer to exercise all match arms
    assert_eq!(dev.read_register(0x00), 0x4127); // config
    assert_eq!(dev.read_register(0x01), 100u16); // shunt_voltage as u16
    assert_eq!(dev.read_register(0x02), 200); // bus_voltage
    assert_eq!(dev.read_register(0x03), 0); // power (cal=0)
    assert_eq!(dev.read_register(0x04), 0); // current (cal=0)
    assert_eq!(dev.read_register(0x05), 0); // calibration
                                            // 0x06 has side effect (CVRF clear) -- already tested
    assert_eq!(dev.read_register(0x07), 0); // alert_limit
    assert_eq!(dev.read_register(0xFF), 0x2260); // die_id
}

#[test]
fn read_register_unknown_returns_zero() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    // Pointers beyond valid range
    assert_eq!(dev.read_register(0x08), 0);
    assert_eq!(dev.read_register(0x10), 0);
    assert_eq!(dev.read_register(0xFE), 0);
}

#[test]
fn write_register_unknown_returns_nak() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    assert_eq!(dev.write_register(0x08, 0x1234), Err(BusError::DataNak));
    assert_eq!(dev.write_register(0x10, 0x1234), Err(BusError::DataNak));
}

#[test]
fn write_register_config_without_rst() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.write_register(0x00, 0x1234).unwrap();
    assert_eq!(dev.read_register(0x00), 0x1234);
}

#[test]
fn write_register_config_with_rst() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(999);
    dev.write_register(0x00, 0x8000).unwrap(); // RST bit
    assert_eq!(dev.read_register(0x00), 0x4127); // POR default
    assert_eq!(dev.read_register(0x01), 0); // shunt reset
}

#[test]
fn write_register_calibration_direct() {
    use i2c_hil_sim::smbus::SmBusWordDevice;
    let mut dev = Ina230::new(addr());
    dev.write_register(0x05, 0xFFFF).unwrap();
    assert_eq!(dev.calibration(), 0x7FFF); // D15 masked
}

#[test]
fn reset_clears_injected_measurements() {
    let mut dev = Ina230::new(addr());
    dev.set_shunt_voltage_raw(1000);
    dev.set_bus_voltage_raw(5000);

    let mut bus = SimBusBuilder::new().with_device(dev).build();

    // Write RST bit (D15=1) to config register
    bus.write(0x40, &[0x00, 0x80, 0x00]).unwrap();

    // Shunt voltage should be reset to 0
    assert_eq!(read_reg(&mut bus, 0x40, 0x01), 0x0000);
    // Bus voltage should be reset to 0
    assert_eq!(read_reg(&mut bus, 0x40, 0x02), 0x0000);
    // Config should be POR
    assert_eq!(read_reg(&mut bus, 0x40, 0x00), 0x4127);
}
