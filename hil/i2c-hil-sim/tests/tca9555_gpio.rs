use embedded_hal::i2c::I2c;

use i2c_hil_sim::devices::{I2cSwitchBuilder, Tca9555, Tmp1075};
use i2c_hil_sim::{Address, BusError, SimBusBuilder};

fn addr() -> Address {
    Address::new(0x20).unwrap()
}

// --- Default register values ---

#[test]
fn default_input_port_reads_0xff() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    // Read input port 0 and 1 via auto-increment
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0xFF, 0xFF]);
}

#[test]
fn default_output_port_reads_0xff() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0xFF, 0xFF]);
}

#[test]
fn default_polarity_reads_0x00() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x04], &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0x00]);
}

#[test]
fn default_config_reads_0xff() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x06], &mut buf).unwrap();
    assert_eq!(buf, [0xFF, 0xFF]);
}

// --- Write and read back registers ---

#[test]
fn write_read_output_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x02, 0xAB, 0xCD]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0xAB, 0xCD]);
}

#[test]
fn write_read_config_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x06, 0x0F, 0xF0]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x06], &mut buf).unwrap();
    assert_eq!(buf, [0x0F, 0xF0]);
}

#[test]
fn write_read_polarity_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x04, 0xAA, 0x55]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x04], &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0x55]);
}

// --- Input port is read-only ---

#[test]
fn write_to_input_port_is_ignored() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Attempt to write to input port 0
    bus.write(0x20, &[0x00, 0x12, 0x34]).unwrap();

    // Input port should still reflect external state (0xFF default)
    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0xFF, 0xFF]);
}

// --- Invalid command byte ---

#[test]
fn invalid_command_byte_returns_data_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let result = bus.write(0x20, &[0x08]);
    assert_eq!(result, Err(BusError::DataNak));
}

#[test]
fn invalid_command_0xff_returns_data_nak() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    let result = bus.write(0x20, &[0xFF]);
    assert_eq!(result, Err(BusError::DataNak));
}

// --- Input port computation ---

#[test]
fn input_all_inputs_reflects_external_state() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Config defaults to 0xFF (all input), external defaults to 0xFF
    // Set external input: port 0 = 0xA5, port 1 = 0xB6
    bus.devices().0.set_external_input(0xB6A5);

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0xA5, 0xB6]);
}

#[test]
fn input_all_outputs_reflects_output_register() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Set all pins as output
    bus.write(0x20, &[0x06, 0x00, 0x00]).unwrap();
    // Set output value
    bus.write(0x20, &[0x02, 0xDE, 0xAD]).unwrap();

    // Input port should reflect output register for output pins
    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    assert_eq!(buf, [0xDE, 0xAD]);
}

#[test]
fn input_mixed_pins() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Port 0: bits 7-4 input, bits 3-0 output (config = 0xF0)
    // Port 1: all output (config = 0x00)
    bus.write(0x20, &[0x06, 0xF0, 0x00]).unwrap();

    // Output: port 0 = 0xAB, port 1 = 0xCD
    bus.write(0x20, &[0x02, 0xAB, 0xCD]).unwrap();

    // External input: port 0 = 0x78, port 1 = 0x56
    bus.devices().0.set_external_input(0x5678);

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    // Port 0: upper nibble from external (0x7_), lower nibble from output (0x_B) = 0x7B
    // Port 1: all from output = 0xCD
    assert_eq!(buf[0], 0x7B);
    assert_eq!(buf[1], 0xCD);
}

#[test]
fn polarity_inversion_on_inputs_only() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // All pins as input (default config = 0xFF)
    // Set polarity inversion on port 0 only
    bus.write(0x20, &[0x04, 0xFF, 0x00]).unwrap();

    // External input: port 0 = 0x55, port 1 = 0x00
    bus.devices().0.set_external_input(0x0055);

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    // Port 0: 0x55 XOR 0xFF = 0xAA (inverted)
    // Port 1: 0x00 XOR 0x00 = 0x00 (no inversion)
    assert_eq!(buf, [0xAA, 0x00]);
}

#[test]
fn polarity_inversion_does_not_affect_output_pins() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Port 0: all output (config = 0x00)
    bus.write(0x20, &[0x06, 0x00]).unwrap();
    // Set polarity inversion on port 0
    bus.write(0x20, &[0x04, 0xFF]).unwrap();
    // Output = 0x55
    bus.write(0x20, &[0x02, 0x55]).unwrap();

    let mut buf = [0u8; 1];
    bus.write_read(0x20, &[0x00], &mut buf).unwrap();
    // Output pins reflect output register directly, not inverted
    assert_eq!(buf[0], 0x55);
}

// --- Auto-increment ---

#[test]
fn auto_increment_read_toggles_port() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Write distinct output values
    bus.write(0x20, &[0x02, 0xAA, 0x55]).unwrap();

    // Read 4 bytes from output port 0: should alternate port 0/1/0/1
    let mut buf = [0u8; 4];
    bus.write_read(0x20, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0x55, 0xAA, 0x55]);
}

#[test]
fn auto_increment_write_toggles_port() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Write polarity port 0 and 1 in a single transaction
    bus.write(0x20, &[0x04, 0xAA, 0x55]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x04], &mut buf).unwrap();
    assert_eq!(buf, [0xAA, 0x55]);
}

#[test]
fn auto_increment_stays_within_register_pair() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Write config port 0 and 1
    bus.write(0x20, &[0x06, 0x0F, 0xF0]).unwrap();

    // Read 4 bytes from config — should toggle between 0x06 and 0x07
    let mut buf = [0u8; 4];
    bus.write_read(0x20, &[0x06], &mut buf).unwrap();
    assert_eq!(buf, [0x0F, 0xF0, 0x0F, 0xF0]);
}

// --- Accessor methods ---

#[test]
fn output_port_accessor() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x02, 0xAB, 0xCD]).unwrap();
    assert_eq!(bus.devices().0.output_port(), 0xCDAB);
}

#[test]
fn config_port_accessor() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x06, 0x0F, 0xF0]).unwrap();
    assert_eq!(bus.devices().0.config_port(), 0xF00F);
}

#[test]
fn polarity_port_accessor() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x04, 0xAA, 0x55]).unwrap();
    assert_eq!(bus.devices().0.polarity_port(), 0x55AA);
}

#[test]
fn input_port_accessor() {
    let dev = Tca9555::new(addr());
    // Default: all input, no polarity, external = 0xFFFF
    assert_eq!(dev.input_port(), 0xFFFF);
}

#[test]
fn command_accessor() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    bus.write(0x20, &[0x04]).unwrap();
    assert_eq!(bus.devices().0.command(), 0x04);
}

#[test]
fn external_input_accessor() {
    let dev = Tca9555::new(addr());
    assert_eq!(dev.external_input(), 0xFFFF);
    dev.set_external_input(0x1234);
    assert_eq!(dev.external_input(), 0x1234);
}

#[test]
fn pins_accessor() {
    let dev = Tca9555::new(addr());
    // pins() should return a reference to the SimPins backend.
    // Default external input is 0xFFFF.
    assert_eq!(dev.pins().external_input(), 0xFFFF);
    dev.pins().set_external_input(0xABCD);
    assert_eq!(dev.pins().external_input(), 0xABCD);
}

// --- Loopback: A's outputs → B's inputs ---

#[test]
fn loopback_two_devices_on_same_bus() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(Address::new(0x20).unwrap()))
        .with_device(Tca9555::new(Address::new(0x21).unwrap()))
        .build();

    // Device set is (0x21, (0x20, ())):
    //   bus.devices().0  = B @ 0x21
    //   bus.devices().1.0 = A @ 0x20

    // Configure device A (0x20): port 0 all output
    bus.write(0x20, &[0x06, 0x00]).unwrap();
    // Write output value on A
    bus.write(0x20, &[0x02, 0xAB]).unwrap();

    // Propagate A's output to B's external input
    let a_output = bus.devices().1 .0.output_port();
    bus.devices().0.set_external_input(a_output);

    // Read B's input port 0 (default: all input)
    let mut buf = [0u8; 1];
    bus.write_read(0x21, &[0x00], &mut buf).unwrap();
    assert_eq!(buf[0], 0xAB);
}

#[test]
fn loopback_through_mux_channels() {
    let mut mux = I2cSwitchBuilder::new(Address::new(0x70).unwrap())
        .channel_with_devices((
            Tmp1075::with_temperature(Address::new(0x48).unwrap(), Tmp1075::celsius_to_raw(25.0)),
            (Tca9555::new(Address::new(0x20).unwrap()), ()),
        ))
        .channel_with_devices((
            Tmp1075::with_temperature(Address::new(0x48).unwrap(), Tmp1075::celsius_to_raw(30.0)),
            (Tca9555::new(Address::new(0x20).unwrap()), ()),
        ))
        .build();

    // Select channel 0, configure TCA9555 as all output on port 0
    mux.write(0x70, &[0x01]).unwrap();
    mux.write(0x20, &[0x06, 0x00]).unwrap();
    // Write output value
    mux.write(0x20, &[0x02, 0x42]).unwrap();

    // Read A's output port via accessor — channel layout:
    // channels is ((ch1_devices, (ch0_devices, ())))
    // ch0 = channels.1.0 = (Tmp1075, (Tca9555, ()))
    // ch1 = channels.0   = (Tmp1075, (Tca9555, ()))
    let ch0_tca = &mux.channels().1 .0 .1 .0;
    let ch1_tca = &mux.channels().0 .1 .0;
    let a_output = ch0_tca.output_port();
    ch1_tca.set_external_input(a_output);

    // Select channel 1, read TCA9555 input
    mux.write(0x70, &[0x02]).unwrap();
    let mut buf = [0u8; 1];
    mux.write_read(0x20, &[0x00], &mut buf).unwrap();
    assert_eq!(buf[0], 0x42);

    // Also verify TMP1075 still works on both channels
    mux.write(0x70, &[0x01]).unwrap();
    let mut temp = [0u8; 2];
    mux.write_read(0x48, &[0x00], &mut temp).unwrap();
    assert_eq!(temp, [0x19, 0x00]); // 25 C

    mux.write(0x70, &[0x02]).unwrap();
    mux.write_read(0x48, &[0x00], &mut temp).unwrap();
    assert_eq!(temp, [0x1E, 0x00]); // 30 C
}

// --- Edge cases ---

#[test]
fn empty_write_is_no_op() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Empty write should not change anything
    bus.write(0x20, &[]).unwrap();

    let mut buf = [0u8; 2];
    bus.write_read(0x20, &[0x02], &mut buf).unwrap();
    assert_eq!(buf, [0xFF, 0xFF]);
}

#[test]
fn write_command_only_sets_pointer() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Write only command byte (no data) to polarity register
    bus.write(0x20, &[0x04]).unwrap();
    assert_eq!(bus.devices().0.command(), 0x04);

    // Polarity should still be default
    let mut buf = [0u8; 2];
    bus.read(0x20, &mut buf).unwrap();
    assert_eq!(buf, [0x00, 0x00]);
}

#[test]
fn command_persists_across_transactions() {
    let mut bus = SimBusBuilder::new()
        .with_device(Tca9555::new(addr()))
        .build();

    // Write distinct outputs
    bus.write(0x20, &[0x02, 0x12, 0x34]).unwrap();

    // After the previous write of 2 data bytes, command toggled twice:
    // 0x02 → 0x03 → 0x02
    let mut buf = [0u8; 2];
    bus.read(0x20, &mut buf).unwrap();
    assert_eq!(buf, [0x12, 0x34]);
}
