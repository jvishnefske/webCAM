//! GPIO state manager.
//!
//! Tracks the direction and output value of all 32 GPIO pins (16 bank A + 16 bank B)
//! and bridges between the USB protocol and physical pins via `embedded-hal` traits.

use embedded_hal::digital::{ErrorType, InputPin, OutputPin, PinState};

use crate::protocol::*;

/// Error type for GPIO operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GpioError {
    /// Pin offset out of range.
    InvalidPin,
    /// Pin is configured as input but was written to (or vice versa).
    WrongDirection,
    /// The underlying HAL pin returned an error.
    HalError,
    /// The USB message was malformed.
    ProtocolError,
}

/// Direction of a GPIO pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PinDirection {
    Input,
    Output,
}

/// Per-pin state for GPIO bank A.
#[derive(Debug, Clone, Copy)]
struct PinAState {
    direction: PinDirection,
    output_val: bool,
}

impl Default for PinAState {
    fn default() -> Self {
        Self {
            direction: PinDirection::Input,
            output_val: false,
        }
    }
}

/// State of GPIO bank A (16 individually-addressable pins).
pub struct GpioAState {
    pins: [PinAState; 16],
}

impl Default for GpioAState {
    fn default() -> Self {
        Self::new()
    }
}

impl GpioAState {
    pub const NUM_PINS: u8 = 16;

    pub fn new() -> Self {
        Self {
            pins: [PinAState::default(); 16],
        }
    }

    /// Process a GPIO-A message from the host and produce a response.
    ///
    /// The `read_pin` and `write_pin` callbacks connect to physical hardware.
    /// - `read_pin(offset) -> bool`: read the physical pin level
    /// - `write_pin(offset, value)`: drive the physical pin
    /// - `set_direction(offset, dir)`: configure pin direction on hardware
    pub fn process_msg<R, W, D>(
        &mut self,
        msg: &GpioAMsg,
        read_pin: &mut R,
        write_pin: &mut W,
        set_direction: &mut D,
    ) -> Result<GpioAMsg, GpioError>
    where
        R: FnMut(u8) -> Result<bool, GpioError>,
        W: FnMut(u8, bool) -> Result<(), GpioError>,
        D: FnMut(u8, PinDirection) -> Result<(), GpioError>,
    {
        let offset = msg.offset;
        if offset >= Self::NUM_PINS {
            return Err(GpioError::InvalidPin);
        }
        let idx = offset as usize;

        match msg.cmd {
            GPIOA_CMD_SETIN => {
                self.pins[idx].direction = PinDirection::Input;
                set_direction(offset, PinDirection::Input)?;
                let mut resp = *msg;
                resp.answer = 0;
                Ok(resp)
            }
            GPIOA_CMD_SETOUT => {
                self.pins[idx].direction = PinDirection::Output;
                set_direction(offset, PinDirection::Output)?;
                // Apply initial output value
                let val = msg.outval != 0;
                self.pins[idx].output_val = val;
                write_pin(offset, val)?;
                let mut resp = *msg;
                resp.answer = 0;
                Ok(resp)
            }
            GPIOA_CMD_GETIN => {
                let val = read_pin(offset)?;
                let mut resp = *msg;
                resp.answer = if val { 1 } else { 0 };
                Ok(resp)
            }
            GPIOA_CMD_CONT => {
                // Continuous output — set pin value
                if self.pins[idx].direction != PinDirection::Output {
                    return Err(GpioError::WrongDirection);
                }
                let val = msg.outval != 0;
                self.pins[idx].output_val = val;
                write_pin(offset, val)?;
                let mut resp = *msg;
                resp.answer = 0;
                Ok(resp)
            }
            GPIOA_CMD_PULSE | GPIOA_CMD_PWM => {
                // Pulse/PWM — simplified: just set the output
                if self.pins[idx].direction != PinDirection::Output {
                    return Err(GpioError::WrongDirection);
                }
                let val = msg.outval != 0;
                self.pins[idx].output_val = val;
                write_pin(offset, val)?;
                let mut resp = *msg;
                resp.answer = 0;
                Ok(resp)
            }
            GPIOA_CMD_SETINT => {
                // Interrupt configuration — acknowledge but
                // actual IRQ delivery would need an interrupt endpoint
                let mut resp = *msg;
                resp.answer = 0;
                Ok(resp)
            }
            _ => Err(GpioError::ProtocolError),
        }
    }

    /// Get the current direction of a pin.
    pub fn direction(&self, offset: u8) -> Option<PinDirection> {
        self.pins.get(offset as usize).map(|p| p.direction)
    }

    /// Get the current output latch value.
    pub fn output_val(&self, offset: u8) -> Option<bool> {
        self.pins.get(offset as usize).map(|p| p.output_val)
    }
}

/// State of GPIO bank B (16 pins, port-wide operations).
pub struct GpioBState {
    /// Bitmask: 1 = output, 0 = input.
    direction_mask: u16,
    /// Current output latch values.
    output_val: u16,
}

impl Default for GpioBState {
    fn default() -> Self {
        Self::new()
    }
}

impl GpioBState {
    pub const NUM_PINS: u8 = 16;

    pub fn new() -> Self {
        Self {
            direction_mask: 0,
            output_val: 0,
        }
    }

    /// Process a GPIO-B message from the host.
    ///
    /// - `read_port() -> u16`: read all 16 input pin levels
    /// - `write_port(val, mask)`: set output pins; mask selects which bits
    /// - `set_dir(mask)`: set direction mask (1=output, 0=input)
    pub fn process_msg<R, W, D>(
        &mut self,
        msg: &GpioBMsg,
        read_port: &mut R,
        write_port: &mut W,
        set_dir: &mut D,
    ) -> Result<GpioBMsg, GpioError>
    where
        R: FnMut() -> Result<u16, GpioError>,
        W: FnMut(u16, u16) -> Result<(), GpioError>,
        D: FnMut(u16) -> Result<(), GpioError>,
    {
        match msg.cmd {
            GPIOB_CMD_SETDIR => {
                let mask = msg.val_u16();
                self.direction_mask = mask;
                set_dir(mask)?;
                Ok(GpioBMsg::new_readback(mask))
            }
            GPIOB_CMD_SETVAL => {
                let val = msg.val_u16();
                let mask = msg.mask_u16();
                // Update output latch: clear masked bits, set new ones
                self.output_val = (self.output_val & !mask) | (val & mask);
                write_port(self.output_val, mask)?;
                // Read back actual pin states
                let readback = read_port()?;
                Ok(GpioBMsg::new_readback(readback))
            }
            _ => Err(GpioError::ProtocolError),
        }
    }

    /// Get current direction mask.
    pub fn direction_mask(&self) -> u16 {
        self.direction_mask
    }

    /// Is pin at offset an output?
    pub fn is_output(&self, offset: u8) -> bool {
        if offset >= Self::NUM_PINS {
            return false;
        }
        (self.direction_mask & (1 << offset)) != 0
    }
}

// ──────────────────────────────────────────────────────────────────
// embedded-hal adapter: wraps a set of HAL pins for use with GpioAState
// ──────────────────────────────────────────────────────────────────

/// Adapter that bridges an array of `embedded-hal` pins to the
/// GPIO-A state machine's callback interface.
///
/// This allows any MCU's GPIO pins (implementing `InputPin + OutputPin`)
/// to back the viperboard USB GPIO interface.
///
/// # Type Parameters
/// - `P`: Pin type that implements both `InputPin` and `OutputPin`
///   (most HAL "flex" or "configurable" pin types do this)
/// - `N`: Number of pins to expose (up to 16)
pub struct HalPinAdapter<P, const N: usize> {
    pins: [P; N],
}

impl<P, const N: usize> HalPinAdapter<P, N>
where
    P: InputPin + OutputPin + ErrorType,
{
    pub fn new(pins: [P; N]) -> Self {
        Self { pins }
    }

    /// Read a pin's input level.
    pub fn read_pin(&mut self, offset: u8) -> Result<bool, GpioError> {
        let pin = self
            .pins
            .get_mut(offset as usize)
            .ok_or(GpioError::InvalidPin)?;
        pin.is_high().map_err(|_| GpioError::HalError)
    }

    /// Write a pin's output level.
    pub fn write_pin(&mut self, offset: u8, high: bool) -> Result<(), GpioError> {
        let pin = self
            .pins
            .get_mut(offset as usize)
            .ok_or(GpioError::InvalidPin)?;
        pin.set_state(if high { PinState::High } else { PinState::Low })
            .map_err(|_| GpioError::HalError)
    }
}
