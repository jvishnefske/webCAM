//! TCA9555 16-bit I/O expander simulation.
//!
//! Models a TI TCA9555 with eight 8-bit registers accessed via a command
//! byte protocol:
//!
//! | Command | Register              | Access | Default |
//! |---------|-----------------------|--------|---------|
//! | 0x00    | Input Port 0          | R      | 0xFF    |
//! | 0x01    | Input Port 1          | R      | 0xFF    |
//! | 0x02    | Output Port 0         | R/W    | 0xFF    |
//! | 0x03    | Output Port 1         | R/W    | 0xFF    |
//! | 0x04    | Polarity Inversion 0  | R/W    | 0x00    |
//! | 0x05    | Polarity Inversion 1  | R/W    | 0x00    |
//! | 0x06    | Configuration 0       | R/W    | 0xFF    |
//! | 0x07    | Configuration 1       | R/W    | 0xFF    |
//!
//! # Protocol
//!
//! - **Write 1 byte**: Sets the command byte (register pointer).
//! - **Write 2+ bytes**: Sets the command byte, then writes data bytes
//!   to the selected register. The register pointer toggles bit 0 after
//!   each data byte, alternating between port 0 and port 1 of the same
//!   register pair.
//! - **Read**: Returns the register at the current command pointer,
//!   toggling bit 0 after each byte for paired port access.
//!
//! # Pin State Model
//!
//! Each pin's observed state depends on its configuration:
//! - **Output** (config bit = 0): Driven by the output port register.
//!   The input port register reflects the output register value.
//! - **Input** (config bit = 1): High-impedance with internal 100 kΩ
//!   pull-up. The input port register reflects the external pin state
//!   (from [`set_external_input`](Tca9555::set_external_input)) XOR the
//!   polarity inversion bit.
//!
//! # Pin Abstraction
//!
//! The struct is generic over [`Tca9555Pins`], which controls how
//! external input is sourced and how output/config writes are propagated.
//! The default [`SimPins`] implementation uses [`Cell`](core::cell::Cell)
//! for loopback testing, while firmware crates can provide an
//! `AtomicU16`-based implementation for real hardware synchronisation.
//!
//! # Loopback
//!
//! Two `Tca9555` devices can simulate a physical loopback connection
//! where one device's output pins drive another's input pins. The
//! external input uses [`Cell`](core::cell::Cell) for interior
//! mutability, allowing propagation through shared references:
//!
//! ```rust
//! use i2c_hil_sim::{SimBusBuilder, Address};
//! use i2c_hil_sim::devices::Tca9555;
//! use embedded_hal::i2c::I2c;
//!
//! let mut bus = SimBusBuilder::new()
//!     .with_device(Tca9555::new(Address::new(0x20).unwrap()))
//!     .with_device(Tca9555::new(Address::new(0x21).unwrap()))
//!     .build();
//!
//! // Configure device A (0x20) port 0 as all outputs
//! bus.write(0x20, &[0x06, 0x00]).unwrap();
//! // Write 0xAA to device A output port 0
//! bus.write(0x20, &[0x02, 0xAA]).unwrap();
//!
//! // Device set is (0x21, (0x20, ())):
//! //   devs.0   = B @ 0x21
//! //   devs.1.0 = A @ 0x20
//! let devs = bus.devices();
//! devs.0.set_external_input(devs.1.0.output_port());
//!
//! // Read device B input port 0 — sees 0xAA
//! let mut buf = [0u8];
//! bus.write_read(0x21, &[0x00], &mut buf).unwrap();
//! assert_eq!(buf[0], 0xAA);
//! ```

use core::cell::Cell;

use embedded_hal::i2c::Operation;

use crate::device::{Address, I2cDevice};
use crate::error::BusError;

/// Number of addressable command byte values (registers 0x00–0x07).
const REGISTER_COUNT: u8 = 8;

/// Abstraction over the TCA9555 external pin interface.
///
/// Implementations provide the source of external input pin state and
/// receive callbacks when output or configuration registers are written.
/// All methods take `&self` — implementations use interior mutability
/// ([`Cell`] for simulation, `AtomicU16` for firmware).
pub trait Tca9555Pins {
    /// Returns the current 16-bit external input pin state.
    ///
    /// Port 0 is the low byte, port 1 is the high byte.
    fn read_input(&self) -> u16;

    /// Called when the output port register is written.
    ///
    /// `value` is the full 16-bit output register (port 0 low byte,
    /// port 1 high byte). Firmware implementations use this to
    /// synchronise an `AtomicU16` shared with the async task.
    fn on_output_write(&self, value: u16);

    /// Called when the configuration register is written.
    ///
    /// `value` is the full 16-bit config register (port 0 low byte,
    /// port 1 high byte). Firmware implementations use this to
    /// synchronise an `AtomicU16` shared with the async task.
    fn on_config_write(&self, value: u16);
}

/// Simulation pin backend using [`Cell`] for loopback testing.
///
/// External input defaults to `0xFFFF` (all pins pulled high by
/// internal pull-ups). Output and config writes are no-ops since the
/// sim accesses register state directly via accessor methods.
pub struct SimPins {
    input: Cell<u16>,
}

impl SimPins {
    /// Sets the external pin state for input-configured pins.
    ///
    /// Port 0 is the low byte, port 1 is the high byte. Uses
    /// [`Cell`] for interior mutability — no `&mut` required. This
    /// enables loopback propagation through shared references obtained
    /// from [`SimBus::devices`](crate::SimBus::devices).
    ///
    /// Default is `0xFFFF` (all pins pulled high by internal pull-ups).
    pub fn set_external_input(&self, value: u16) {
        self.input.set(value);
    }

    /// Returns the current external input value.
    pub fn external_input(&self) -> u16 {
        self.input.get()
    }
}

impl Default for SimPins {
    fn default() -> Self {
        Self {
            input: Cell::new(0xFFFF),
        }
    }
}

impl Tca9555Pins for SimPins {
    fn read_input(&self) -> u16 {
        self.input.get()
    }

    fn on_output_write(&self, _value: u16) {}

    fn on_config_write(&self, _value: u16) {}
}

/// Simulated TI TCA9555 16-bit I/O expander.
///
/// Generic over a [`Tca9555Pins`] implementation that controls how
/// external input is sourced and how output/config writes propagate.
/// Defaults to [`SimPins`] for loopback simulation testing.
///
/// Stores the output, polarity inversion, and configuration registers
/// as byte arrays. The input port registers (0x00, 0x01) are computed
/// on read from configuration, output, polarity, and external pin state.
///
/// # Construction
///
/// ```rust
/// use i2c_hil_sim::Address;
/// use i2c_hil_sim::devices::Tca9555;
///
/// let gpio = Tca9555::new(Address::new(0x20).unwrap());
/// ```
pub struct Tca9555<P: Tca9555Pins = SimPins> {
    address: Address,
    command: u8,
    output: [u8; 2],
    polarity: [u8; 2],
    config: [u8; 2],
    pins: P,
}

impl<P: Tca9555Pins> Tca9555<P> {
    /// Creates a new TCA9555 at the given address with a custom pin
    /// backend and power-on register defaults.
    ///
    /// Defaults match the datasheet: output ports = 0xFF, polarity
    /// inversion = 0x00, configuration = 0xFF (all inputs).
    pub fn with_pins(address: Address, pins: P) -> Self {
        Self {
            address,
            command: 0,
            output: [0xFF, 0xFF],
            polarity: [0x00, 0x00],
            config: [0xFF, 0xFF],
            pins,
        }
    }

    /// Returns a reference to the pin backend.
    pub fn pins(&self) -> &P {
        &self.pins
    }

    /// Returns the 16-bit output port register value.
    ///
    /// Port 0 occupies the low byte, port 1 the high byte.
    pub fn output_port(&self) -> u16 {
        u16::from_le_bytes(self.output)
    }

    /// Returns the 16-bit configuration register value.
    ///
    /// Bit = 1 means input, bit = 0 means output. Port 0 occupies the
    /// low byte, port 1 the high byte.
    pub fn config_port(&self) -> u16 {
        u16::from_le_bytes(self.config)
    }

    /// Returns the 16-bit polarity inversion register value.
    ///
    /// Port 0 occupies the low byte, port 1 the high byte.
    pub fn polarity_port(&self) -> u16 {
        u16::from_le_bytes(self.polarity)
    }

    /// Returns the computed 16-bit input port value.
    ///
    /// For each bit:
    /// - Output-configured (config = 0): reflects the output register.
    /// - Input-configured (config = 1): reflects external input XOR
    ///   polarity inversion.
    pub fn input_port(&self) -> u16 {
        let lo = self.compute_input_port(0);
        let hi = self.compute_input_port(1);
        u16::from_le_bytes([lo, hi])
    }

    /// Returns the current command byte (register pointer).
    pub fn command(&self) -> u8 {
        self.command
    }

    /// Computes the input port register value for a single port.
    ///
    /// `port` is 0 or 1. For input-configured bits, the external pin
    /// state is XOR'd with the polarity inversion register. For
    /// output-configured bits, the output register value is returned
    /// directly (polarity inversion does not apply to outputs).
    fn compute_input_port(&self, port: usize) -> u8 {
        let ext_all = self.pins.read_input();
        let ext = if port == 0 {
            ext_all as u8
        } else {
            (ext_all >> 8) as u8
        };
        let config = self.config[port];
        let polarity = self.polarity[port];
        let output = self.output[port];

        let input_bits = (ext ^ polarity) & config;
        let output_bits = output & !config;
        input_bits | output_bits
    }

    /// Reads a single register by command byte index.
    fn read_register(&self, reg: u8) -> u8 {
        match reg {
            0 => self.compute_input_port(0),
            1 => self.compute_input_port(1),
            2 => self.output[0],
            3 => self.output[1],
            4 => self.polarity[0],
            5 => self.polarity[1],
            6 => self.config[0],
            7 => self.config[1],
            _ => 0xFF,
        }
    }

    /// Writes a single register by command byte index.
    ///
    /// Input port registers (0x00, 0x01) are read-only; writes to them
    /// are silently ignored per the datasheet. Output and config writes
    /// trigger the corresponding [`Tca9555Pins`] callback.
    fn write_register(&mut self, reg: u8, value: u8) {
        match reg {
            0 | 1 => {} // Input port registers are read-only
            2 => {
                self.output[0] = value;
                self.pins.on_output_write(u16::from_le_bytes(self.output));
            }
            3 => {
                self.output[1] = value;
                self.pins.on_output_write(u16::from_le_bytes(self.output));
            }
            4 => self.polarity[0] = value,
            5 => self.polarity[1] = value,
            6 => {
                self.config[0] = value;
                self.pins.on_config_write(u16::from_le_bytes(self.config));
            }
            7 => {
                self.config[1] = value;
                self.pins.on_config_write(u16::from_le_bytes(self.config));
            }
            _ => {}
        }
    }
}

impl Tca9555<SimPins> {
    /// Creates a new TCA9555 at the given address with power-on defaults.
    ///
    /// Defaults match the datasheet: output ports = 0xFF, polarity
    /// inversion = 0x00, configuration = 0xFF (all inputs). External
    /// input defaults to 0xFFFF (internal pull-ups drive all pins high).
    pub fn new(address: Address) -> Self {
        Self::with_pins(address, SimPins::default())
    }

    /// Sets the external pin state for input-configured pins.
    ///
    /// Port 0 is the low byte, port 1 is the high byte. Uses
    /// [`Cell`] for interior mutability — no `&mut` required. This
    /// enables loopback propagation through shared references obtained
    /// from [`SimBus::devices`](crate::SimBus::devices).
    ///
    /// Default is `0xFFFF` (all pins pulled high by internal pull-ups).
    pub fn set_external_input(&self, value: u16) {
        self.pins.set_external_input(value);
    }

    /// Returns the current external input value.
    pub fn external_input(&self) -> u16 {
        self.pins.external_input()
    }
}

impl<P: Tca9555Pins> I2cDevice for Tca9555<P> {
    fn address(&self) -> Address {
        self.address
    }

    fn process(&mut self, operations: &mut [Operation<'_>]) -> Result<(), BusError> {
        for op in operations {
            match op {
                Operation::Write(data) => {
                    if data.is_empty() {
                        continue;
                    }
                    let cmd = data[0];
                    if cmd >= REGISTER_COUNT {
                        return Err(BusError::DataNak);
                    }
                    self.command = cmd;
                    for &byte in &data[1..] {
                        self.write_register(self.command, byte);
                        self.command ^= 1;
                    }
                }
                Operation::Read(buf) => {
                    for byte in buf.iter_mut() {
                        *byte = self.read_register(self.command);
                        self.command ^= 1;
                    }
                }
            }
        }
        Ok(())
    }
}
