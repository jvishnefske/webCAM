use embedded_hal::i2c::{I2c, Operation};

use i2c_hil_sim::devices::Eeprom256k;
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x50).unwrap()
}

// --- Construction ---

#[test]
fn new_eeprom_memory_is_all_0xff() {
    let bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    let mem = bus.devices().0.memory();
    assert!(mem.iter().all(|&b| b == 0xFF));
}

#[test]
fn with_data_preloads_memory() {
    let mut data = [0u8; 32_768];
    data[0] = 0xDE;
    data[1] = 0xAD;
    data[32_767] = 0x42;

    let bus = SimBusBuilder::new()
        .with_device(Eeprom256k::with_data(addr(), data))
        .build();

    let mem = bus.devices().0.memory();
    assert_eq!(mem[0], 0xDE);
    assert_eq!(mem[1], 0xAD);
    assert_eq!(mem[32_767], 0x42);
}

#[test]
fn pointer_starts_at_zero() {
    let bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();
    assert_eq!(bus.devices().0.pointer(), 0);
}

// --- Write operations ---

#[test]
fn write_single_byte() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write 0xAB to address 0x0010
    bus.write(0x50, &[0x00, 0x10, 0xAB]).unwrap();

    assert_eq!(bus.devices().0.memory()[0x0010], 0xAB);
}

#[test]
fn write_multiple_bytes() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write 4 bytes starting at address 0x0100
    bus.write(0x50, &[0x01, 0x00, 0x11, 0x22, 0x33, 0x44])
        .unwrap();

    let mem = bus.devices().0.memory();
    assert_eq!(mem[0x0100], 0x11);
    assert_eq!(mem[0x0101], 0x22);
    assert_eq!(mem[0x0102], 0x33);
    assert_eq!(mem[0x0103], 0x44);
}

#[test]
fn write_address_only_sets_pointer_no_data() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write just the 2-byte address, no data
    bus.write(0x50, &[0x00, 0x42]).unwrap();

    assert_eq!(bus.devices().0.pointer(), 0x0042);
    // Memory unchanged (still 0xFF)
    assert_eq!(bus.devices().0.memory()[0x0042], 0xFF);
}

#[test]
fn write_fewer_than_two_bytes_returns_data_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    let result = bus.write(0x50, &[0x00]);
    assert_eq!(result, Err(BusError::DataNak));

    let result = bus.write(0x50, &[]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Page write wrapping ---

#[test]
fn page_write_wraps_within_page_boundary() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Start writing at offset 62 within page 0 (addresses 0x003E, 0x003F)
    // Page 0 spans 0x0000–0x003F (64 bytes)
    // Writing 4 bytes should wrap: 0x003E, 0x003F, 0x0000, 0x0001
    bus.write(0x50, &[0x00, 0x3E, 0xAA, 0xBB, 0xCC, 0xDD])
        .unwrap();

    let mem = bus.devices().0.memory();
    assert_eq!(mem[0x003E], 0xAA);
    assert_eq!(mem[0x003F], 0xBB);
    assert_eq!(mem[0x0000], 0xCC); // Wrapped to page start
    assert_eq!(mem[0x0001], 0xDD);
    // Next page untouched
    assert_eq!(mem[0x0040], 0xFF);
}

#[test]
fn page_write_wraps_on_non_zero_page() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Page 2 spans 0x0080–0x00BF
    // Start at offset 63 within page 2 (address 0x00BF)
    // Writing 3 bytes: 0x00BF, 0x0080, 0x0081 (wraps within page 2)
    bus.write(0x50, &[0x00, 0xBF, 0x11, 0x22, 0x33]).unwrap();

    let mem = bus.devices().0.memory();
    assert_eq!(mem[0x00BF], 0x11);
    assert_eq!(mem[0x0080], 0x22); // Wrapped to page 2 start
    assert_eq!(mem[0x0081], 0x33);
    // Adjacent pages untouched
    assert_eq!(mem[0x007F], 0xFF);
    assert_eq!(mem[0x00C0], 0xFF);
}

#[test]
fn full_page_write() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write exactly 64 bytes starting at page-aligned address 0x0100
    let mut payload = [0u8; 66]; // 2 addr bytes + 64 data bytes
    payload[0] = 0x01;
    payload[1] = 0x00;
    for i in 0..64 {
        payload[i + 2] = i as u8;
    }
    bus.write(0x50, &payload).unwrap();

    let mem = bus.devices().0.memory();
    for i in 0..64 {
        assert_eq!(mem[0x0100 + i], i as u8);
    }
}

// --- Read operations ---

#[test]
fn read_from_current_address() {
    let mut data = [0xFF; 32_768];
    data[0] = 0x11;
    data[1] = 0x22;
    data[2] = 0x33;

    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::with_data(addr(), data))
        .build();

    // Pointer starts at 0
    let mut buf = [0u8; 3];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0x11, 0x22, 0x33]);
}

#[test]
fn sequential_read_auto_increments() {
    let mut data = [0xFF; 32_768];
    data[0x10] = 0xAA;
    data[0x11] = 0xBB;
    data[0x12] = 0xCC;

    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::with_data(addr(), data))
        .build();

    // Set address to 0x0010
    bus.write(0x50, &[0x00, 0x10]).unwrap();

    // Read 1 byte at a time
    let mut b1 = [0u8; 1];
    let mut b2 = [0u8; 1];
    let mut b3 = [0u8; 1];
    bus.read(0x50, &mut b1).unwrap();
    bus.read(0x50, &mut b2).unwrap();
    bus.read(0x50, &mut b3).unwrap();
    assert_eq!(b1, [0xAA]);
    assert_eq!(b2, [0xBB]);
    assert_eq!(b3, [0xCC]);
}

#[test]
fn sequential_read_wraps_at_memory_boundary() {
    let mut data = [0x00; 32_768];
    data[32_766] = 0xEE;
    data[32_767] = 0xFF;
    data[0] = 0x00;
    data[1] = 0x01;

    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::with_data(addr(), data))
        .build();

    // Set address to 0x7FFE (32766)
    bus.write(0x50, &[0x7F, 0xFE]).unwrap();

    let mut buf = [0u8; 4];
    bus.read(0x50, &mut buf).unwrap();
    assert_eq!(buf, [0xEE, 0xFF, 0x00, 0x01]); // Wraps around
}

#[test]
fn empty_read_is_noop() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    bus.write(0x50, &[0x00, 0x10]).unwrap();
    let ptr_before = bus.devices().0.pointer();

    let mut buf = [0u8; 0];
    bus.read(0x50, &mut buf).unwrap();

    assert_eq!(bus.devices().0.pointer(), ptr_before);
}

// --- Write-then-Read (random read) ---

#[test]
fn write_read_random_access() {
    let mut data = [0xFF; 32_768];
    data[0x1234] = 0xBE;
    data[0x1235] = 0xEF;

    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::with_data(addr(), data))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x50, &[0x12, 0x34], &mut buf).unwrap();
    assert_eq!(buf, [0xBE, 0xEF]);
}

// --- Transaction operations ---

#[test]
fn transaction_write_then_read_back() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write data, then set address, then read back
    let mut read_buf = [0u8; 3];
    let mut ops = [
        // Write 3 bytes starting at 0x0200
        Operation::Write(&[0x02, 0x00, 0xAA, 0xBB, 0xCC]),
        // Set address back to 0x0200
        Operation::Write(&[0x02, 0x00]),
        // Read 3 bytes
        Operation::Read(&mut read_buf),
    ];
    bus.transaction(0x50, &mut ops).unwrap();

    assert_eq!(read_buf, [0xAA, 0xBB, 0xCC]);
}

// --- Address masking ---

#[test]
fn address_high_bit_is_masked() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Address 0x8010 should be masked to 0x0010 (bit 15 ignored)
    bus.write(0x50, &[0x80, 0x10, 0x42]).unwrap();

    assert_eq!(bus.devices().0.memory()[0x0010], 0x42);
}

// --- Write followed by read-back (round-trip) ---

#[test]
fn write_and_read_back_round_trip() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write pattern
    bus.write(0x50, &[0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF])
        .unwrap();

    // Read back
    let mut buf = [0u8; 4];
    bus.write_read(0x50, &[0x00, 0x00], &mut buf).unwrap();
    assert_eq!(buf, [0xDE, 0xAD, 0xBE, 0xEF]);
}

// --- High address range ---

#[test]
fn write_at_end_of_memory() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Write to last byte at address 0x7FFF
    bus.write(0x50, &[0x7F, 0xFF, 0x42]).unwrap();

    assert_eq!(bus.devices().0.memory()[0x7FFF], 0x42);
}

#[test]
fn write_at_last_page_wraps_within_last_page() {
    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(addr()))
        .build();

    // Last page: 0x7FC0–0x7FFF
    // Start at 0x7FFF (last byte of last page), write 2 bytes
    bus.write(0x50, &[0x7F, 0xFF, 0xAA, 0xBB]).unwrap();

    let mem = bus.devices().0.memory();
    assert_eq!(mem[0x7FFF], 0xAA);
    assert_eq!(mem[0x7FC0], 0xBB); // Wrapped to start of last page
}

// --- Multiple devices on bus ---

#[test]
fn eeprom_coexists_with_other_devices() {
    use i2c_hil_sim::devices::RegisterDevice;

    let mut bus = SimBusBuilder::new()
        .with_device(Eeprom256k::new(Address::new(0x50).unwrap()))
        .with_device(RegisterDevice::new(Address::new(0x48).unwrap(), [0u8; 4]))
        .build();

    // Write to EEPROM
    bus.write(0x50, &[0x00, 0x00, 0x42]).unwrap();
    // Write to register device
    bus.write(0x48, &[0x00, 0x99]).unwrap();

    // Read back from EEPROM
    let mut eeprom_buf = [0u8; 1];
    bus.write_read(0x50, &[0x00, 0x00], &mut eeprom_buf)
        .unwrap();
    assert_eq!(eeprom_buf, [0x42]);

    // Read back from register device
    let mut reg_buf = [0u8; 1];
    bus.write_read(0x48, &[0x00], &mut reg_buf).unwrap();
    assert_eq!(reg_buf, [0x99]);
}
