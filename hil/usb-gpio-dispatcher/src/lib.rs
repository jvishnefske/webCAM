//! # vprbrd-usb-gpio
//!
//! A `no_std` Rust implementation of the device side of the
//! [viperboard](https://www.nanorivertech.com/viperboard.html) GPIO USB
//! protocol, built on [`embassy-usb`](https://docs.embassy.dev/embassy-usb/).
//!
//! When a microcontroller running this library is connected to a Linux host,
//! the kernel's `mfd-viperboard` + `gpio-viperboard` drivers automatically
//! recognize it (by VID:PID `0x2058:0x1005`) and create two GPIO chips:
//!
//! - **gpioa** — 16 individually-addressable pins with direction control
//! - **gpiob** — 16 pins operated as a port with bitmask access
//!
//! These appear under `/sys/class/gpio/` and can be controlled with
//! standard sysfs GPIO, libgpiod, or any Linux GPIO consumer.
//!
//! ## Why viperboard over MPSSE?
//!
//! | Driver | Protocol | New HW? | Complexity |
//! |---|---|---|---|
//! | **gpio-viperboard** | EP0 control xfers only | **Yes** — VID:PID match | **Low** — no bulk EPs |
//! | gpio-mpsse (FTDI) | FTDI MPSSE command processor | No — requires FTDI chip | **High** — stateful command engine |
//!
//! The MPSSE approach (`gpio-mpsse.c`) emulates FTDI's proprietary MPSSE
//! command processor protocol, which requires implementing a complex
//! stateful byte-stream command engine and is designed around FTDI's
//! specific bulk endpoint layout. You cannot implement MPSSE device-side
//! on a generic MCU without essentially becoming an FTDI chip emulator.
//!
//! The viperboard protocol is straightforward: all GPIO operations are
//! USB vendor control transfers on EP0. No bulk endpoints, no command
//! parsing state machines. The Linux driver sends a packed struct,
//! the device processes it and returns a response.
//!
//! ## Architecture
//!
//! ```text
//!  ┌───────────────────────────────────┐     ┌──────────────────────────┐
//!  │  Linux Host                       │     │  MCU (Embassy)           │
//!  │                                   │ USB │                          │
//!  │  /sys/class/gpio/gpiochipN/       │◄───►│  vprbrd-usb-gpio         │
//!  │    export / unexport              │     │   ├─ class.rs (Handler)  │
//!  │    gpioN/direction                │ EP0 │   ├─ gpio.rs  (state)   │
//!  │    gpioN/value                    │ctrl │   └─ protocol.rs (wire) │
//!  │                                   │xfer │          │              │
//!  │  mfd-viperboard (VID:PID match)   │     │   embedded-hal pins     │
//!  │  gpio-viperboard (platform drv)   │     │   (InputPin + OutputPin)│
//!  └───────────────────────────────────┘     └──────────────────────────┘
//! ```
//!
//! ## Protocol Summary
//!
//! All operations are USB vendor control transfers (`bmRequestType = 0x40/0xC0`):
//!
//! **GPIO-A** (`bRequest = 0xED`):
//! - `SETOUT(offset)` / `SETIN(offset)` — configure direction
//! - `CONT(offset, val)` — set output level
//! - `GETIN(offset)` — read input level (returns in `answer` field)
//!
//! **GPIO-B** (`bRequest = 0xEE`):
//! - `SETDIR(mask)` — set direction bitmask for all 16 pins
//! - `SETVAL(val, mask)` — write output values with bit mask
//! - IN transfer reads back current port state
//!
//! ## Accepting `embedded-hal` Pins
//!
//! The handler accepts closures for pin I/O, but the `HalPinAdapter`
//! in `gpio.rs` provides a ready-made bridge for any array of pins
//! implementing `embedded_hal::digital::{InputPin, OutputPin}`:
//!
//! ```ignore
//! use vprbrd_usb_gpio::gpio::HalPinAdapter;
//!
//! // Your MCU's flex pins (e.g., embassy-rp FlexPin)
//! let pins = [pin0, pin1, pin2, /* ... */];
//! let mut adapter = HalPinAdapter::new(pins);
//!
//! let handler = VprbrdGpioHandler::new(
//!     &mut state,
//!     |offset| adapter.read_pin(offset),
//!     |offset, val| adapter.write_pin(offset, val),
//!     |_offset, _dir| Ok(()), // direction change handled by flex pin
//!     // ... GPIO-B callbacks
//! );
//! ```
//!
//! ## Async Wait Support
//!
//! The `embedded_hal_async::digital::Wait` trait is consumed at the
//! integration level — if your MCU pins implement `Wait`, you can
//! use them to back interrupt-capable GPIO-A pins. The viperboard
//! protocol's `SETINT` command configures edge detection; delivering
//! the interrupt to the host would require an interrupt USB endpoint
//! (not implemented in the basic viperboard protocol, but extensible).
//!
//! ## Host Setup
//!
//! ```bash
//! # Ensure kernel modules are loaded
//! modprobe mfd-viperboard
//! modprobe gpio-viperboard
//!
//! # After plugging in, find the gpiochip:
//! ls /sys/class/gpio/gpiochip*
//!
//! # Export and use a pin:
//! echo 0 > /sys/class/gpio/export
//! echo out > /sys/class/gpio/gpio0/direction
//! echo 1 > /sys/class/gpio/gpio0/value
//!
//! # Or use libgpiod:
//! gpioset gpiochip0 0=1
//! gpioget gpiochip0 1
//! ```

#![no_std]
#![forbid(unsafe_code)]

pub mod class;
pub mod gpio;
pub mod protocol;

// Re-export primary API types.
pub use class::{add_vprbrd_interface, VprbrdGpioConfig, VprbrdGpioHandler, VprbrdGpioState};
pub use gpio::{GpioAState, GpioBState, GpioError, HalPinAdapter, PinDirection};
pub use protocol::{VPRBRD_PID, VPRBRD_VID};
