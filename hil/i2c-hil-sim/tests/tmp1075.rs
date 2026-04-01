use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::Tmp1075;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x48).unwrap()
}

#[test]
fn default_registers() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    // Temp register (pointer defaults to 0)
    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0x00]);

    // Config register
    bus.write_read(0x48, &[0x01], &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0xFF]);

    // T_LOW register
    bus.write_read(0x48, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0x4B, 0x00]);

    // T_HIGH register
    bus.write_read(0x48, &[0x03], &mut buf).unwrap();
    assert_eq!(buf, [0x50, 0x00]);
}

#[test]
fn read_temperature_25c() {
    let raw = Tmp1075::celsius_to_raw(25.0);
    assert_eq!(raw, 0x1900);

    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::with_temperature(addr(), raw))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);
}

#[test]
fn read_temperature_negative() {
    let raw = Tmp1075::celsius_to_raw(-25.0);
    assert_eq!(raw, 0xE700);

    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::with_temperature(addr(), raw))
        .build();

    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0xE7, 0x00]);
}

#[test]
fn celsius_to_raw_zero() {
    assert_eq!(Tmp1075::celsius_to_raw(0.0), 0x0000);
}

#[test]
fn celsius_to_raw_fractional() {
    // 0.0625 °C = 1 LSB = 0x0010
    assert_eq!(Tmp1075::celsius_to_raw(0.0625), 0x0010);
}

#[test]
fn write_config_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    // Write 0x1234 to config register (pointer 0x01)
    bus.write(0x48, &[0x01, 0x12, 0x34]).unwrap();

    // Read it back
    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x01], &mut buf).unwrap();
    assert_eq!(buf, [0x12, 0x34]);
}

#[test]
fn write_to_temp_register_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    let result = bus.write(0x48, &[0x00, 0x12, 0x34]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn invalid_pointer_returns_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    let result = bus.write(0x48, &[0x04]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn read_repeats_register_bytes() {
    let raw = Tmp1075::celsius_to_raw(25.0); // 0x1900
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::with_temperature(addr(), raw))
        .build();

    // Read 4 bytes — should repeat the 2-byte register value
    let mut buf = [0u8; 4];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00, 0x19, 0x00]);
}

#[test]
fn set_pointer_only_does_not_write_data() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    // Write just pointer byte to config register
    bus.write(0x48, &[0x01]).unwrap();

    // Config should still be default
    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0xFF]);
}

#[test]
fn set_temperature_raw_updates_register() {
    let mut sensor = Tmp1075::new(addr());
    sensor.set_temperature_raw(0x3200);
    assert_eq!(sensor.registers()[0], 0x3200);
}

#[test]
fn with_temperature_reads_through_bus() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::with_temperature(addr(), 0x3200))
        .build();

    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn write_t_low_and_t_high() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    // Write T_LOW = 0x1900 (25 °C)
    bus.write(0x48, &[0x02, 0x19, 0x00]).unwrap();
    // Write T_HIGH = 0x3200 (50 °C)
    bus.write(0x48, &[0x03, 0x32, 0x00]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x48, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0x19, 0x00]);

    bus.write_read(0x48, &[0x03], &mut buf).unwrap();
    assert_eq!(buf, [0x32, 0x00]);
}

#[test]
fn pointer_persists_across_transactions() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tmp1075::new(addr()))
        .build();

    // Set pointer to config register
    bus.write(0x48, &[0x01]).unwrap();

    // Read without setting pointer — should still read config
    let mut buf = [0u8; 2];
    bus.read(0x48, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0xFF]);
}

#[test]
fn pointer_accessor() {
    let dev = Tmp1075::new(addr());
    assert_eq!(dev.pointer(), 0);
}

#[test]
fn registers_accessor() {
    let dev = Tmp1075::new(addr());
    let regs = dev.registers();
    assert_eq!(regs[0], 0x0000); // Temp
    assert_eq!(regs[1], 0x00FF); // Config
    assert_eq!(regs[2], 0x4B00); // T_LOW
    assert_eq!(regs[3], 0x5000); // T_HIGH
}
