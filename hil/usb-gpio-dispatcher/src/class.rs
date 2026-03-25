//! Viperboard USB class implementation for embassy-usb.
//!
//! Implements the device side of the viperboard GPIO protocol.
//! When plugged into a Linux host with `CONFIG_MFD_VIPERBOARD` and
//! `CONFIG_GPIO_VIPERBOARD` enabled, the device appears as two
//! `gpiochip` devices under `/sys/class/gpio/`.
//!
//! # USB Descriptor Layout
//!
//! The viperboard uses a single vendor-specific interface:
//! - `bInterfaceClass = 0xFF` (vendor-specific)
//! - `bInterfaceSubClass = 0x00`
//! - `bInterfaceProtocol = 0x00`
//! - All GPIO operations use vendor control transfers on EP0
//! - No bulk/interrupt endpoints needed for basic GPIO
//!
//! The Linux MFD driver probes by VID:PID match (`0x2058:0x1005`),
//! then the gpio-viperboard platform driver sends vendor control
//! messages for each GPIO operation.

use embassy_usb::control::{InResponse, OutResponse, Request, RequestType};
use embassy_usb::Handler;

use crate::gpio::{GpioAState, GpioBState, GpioError, PinDirection};
use crate::protocol::*;

/// Configuration for the viperboard GPIO device.
#[derive(Debug, Clone)]
pub struct VprbrdGpioConfig {
    /// Number of GPIO-A pins to expose (max 16).
    pub gpioa_pins: u8,
    /// Number of GPIO-B pins to expose (max 16).
    pub gpiob_pins: u8,
}

impl Default for VprbrdGpioConfig {
    fn default() -> Self {
        Self {
            gpioa_pins: 16,
            gpiob_pins: 16,
        }
    }
}

/// Shared state for the viperboard GPIO USB device.
///
/// This holds the GPIO state machines and configuration.
/// The handler borrows this mutably to process control requests.
pub struct VprbrdGpioState {
    pub config: VprbrdGpioConfig,
    pub gpioa: GpioAState,
    pub gpiob: GpioBState,
}

impl VprbrdGpioState {
    pub fn new(config: VprbrdGpioConfig) -> Self {
        Self {
            config,
            gpioa: GpioAState::new(),
            gpiob: GpioBState::new(),
        }
    }
}

/// Control request handler for the viperboard GPIO protocol.
///
/// # Lifetime of pin callbacks
///
/// The handler stores closures for reading/writing physical pins.
/// These closures typically capture a reference to your MCU's GPIO
/// peripheral or an array of `embedded-hal` pin objects.
///
/// # Example
///
/// ```ignore
/// let handler = VprbrdGpioHandler::new(
///     &mut state,
///     |offset| { /* read pin */ Ok(pins.read_pin(offset)?) },
///     |offset, val| { /* write pin */ pins.write_pin(offset, val)?; Ok(()) },
///     |offset, dir| { /* set direction */ Ok(()) },
///     || { /* read port B */ Ok(port_b.read_all()) },
///     |val, mask| { /* write port B */ port_b.write_masked(val, mask); Ok(()) },
///     |mask| { /* set port B direction */ port_b.set_direction(mask); Ok(()) },
/// );
/// ```
pub struct VprbrdGpioHandler<'a, RA, WA, DA, RB, WB, DB>
where
    RA: FnMut(u8) -> Result<bool, GpioError>,
    WA: FnMut(u8, bool) -> Result<(), GpioError>,
    DA: FnMut(u8, PinDirection) -> Result<(), GpioError>,
    RB: FnMut() -> Result<u16, GpioError>,
    WB: FnMut(u16, u16) -> Result<(), GpioError>,
    DB: FnMut(u16) -> Result<(), GpioError>,
{
    state: &'a mut VprbrdGpioState,
    read_a: RA,
    write_a: WA,
    dir_a: DA,
    read_b: RB,
    write_b: WB,
    dir_b: DB,
}

impl<'a, RA, WA, DA, RB, WB, DB> VprbrdGpioHandler<'a, RA, WA, DA, RB, WB, DB>
where
    RA: FnMut(u8) -> Result<bool, GpioError>,
    WA: FnMut(u8, bool) -> Result<(), GpioError>,
    DA: FnMut(u8, PinDirection) -> Result<(), GpioError>,
    RB: FnMut() -> Result<u16, GpioError>,
    WB: FnMut(u16, u16) -> Result<(), GpioError>,
    DB: FnMut(u16) -> Result<(), GpioError>,
{
    pub fn new(
        state: &'a mut VprbrdGpioState,
        read_a: RA,
        write_a: WA,
        dir_a: DA,
        read_b: RB,
        write_b: WB,
        dir_b: DB,
    ) -> Self {
        Self {
            state,
            read_a,
            write_a,
            dir_a,
            read_b,
            write_b,
            dir_b,
        }
    }
}

impl<'a, RA, WA, DA, RB, WB, DB> Handler for VprbrdGpioHandler<'a, RA, WA, DA, RB, WB, DB>
where
    RA: FnMut(u8) -> Result<bool, GpioError>,
    WA: FnMut(u8, bool) -> Result<(), GpioError>,
    DA: FnMut(u8, PinDirection) -> Result<(), GpioError>,
    RB: FnMut() -> Result<u16, GpioError>,
    WB: FnMut(u16, u16) -> Result<(), GpioError>,
    DB: FnMut(u16) -> Result<(), GpioError>,
{
    /// Handle vendor OUT control transfers (host → device).
    ///
    /// The Linux viperboard driver sends GPIO commands as vendor
    /// control transfers. GPIO-A uses a two-phase pattern:
    /// 1. OUT transfer sends the command
    /// 2. IN transfer reads back the response (with answer field)
    ///
    /// GPIO-B sends the command and reads back in a single IN transfer
    /// for reads, or OUT for writes.
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request_type != RequestType::Vendor {
            return None;
        }

        match req.request {
            VPRBRD_USB_REQUEST_GPIOA => {
                // GPIO-A OUT: host sends a command
                if let Some(msg) = GpioAMsg::from_bytes(data) {
                    match self.state.gpioa.process_msg(
                        &msg,
                        &mut self.read_a,
                        &mut self.write_a,
                        &mut self.dir_a,
                    ) {
                        Ok(_) => Some(OutResponse::Accepted),
                        Err(_) => Some(OutResponse::Rejected),
                    }
                } else {
                    Some(OutResponse::Rejected)
                }
            }
            VPRBRD_USB_REQUEST_GPIOB => {
                // GPIO-B OUT: host sends SETDIR or SETVAL
                if let Some(msg) = GpioBMsg::from_bytes(data) {
                    match self.state.gpiob.process_msg(
                        &msg,
                        &mut self.read_b,
                        &mut self.write_b,
                        &mut self.dir_b,
                    ) {
                        Ok(_) => Some(OutResponse::Accepted),
                        Err(_) => Some(OutResponse::Rejected),
                    }
                } else {
                    Some(OutResponse::Rejected)
                }
            }
            _ => None,
        }
    }

    /// Handle vendor IN control transfers (device → host).
    ///
    /// GPIO-A: The host reads back the GpioAMsg with the `answer` field
    /// populated (e.g., for GETIN the pin state is in `answer`).
    ///
    /// GPIO-B: The host reads back the GpioBMsg with port values.
    fn control_in<'b>(&'b mut self, req: Request, buf: &'b mut [u8]) -> Option<InResponse<'b>> {
        if req.request_type != RequestType::Vendor {
            return None;
        }

        match req.request {
            VPRBRD_USB_REQUEST_GPIOA => {
                // GPIO-A IN: The Linux driver does an OUT then an IN.
                // For GETIN, we need to read the pin now.
                // The wValue field typically carries the pin offset.
                let offset = req.value as u8;

                // Build a GETIN message and process it
                let msg = GpioAMsg {
                    cmd: GPIOA_CMD_GETIN,
                    clk: 0,
                    offset,
                    t1: 0,
                    t2: 0,
                    invert: 0,
                    pwmlevel: 0,
                    outval: 0,
                    risefall: 0,
                    answer: 0,
                    _fill: 0,
                };

                match self.state.gpioa.process_msg(
                    &msg,
                    &mut self.read_a,
                    &mut self.write_a,
                    &mut self.dir_a,
                ) {
                    Ok(resp) => {
                        let n = resp.to_bytes(buf);
                        if n > 0 {
                            Some(InResponse::Accepted(&buf[..n]))
                        } else {
                            Some(InResponse::Rejected)
                        }
                    }
                    Err(_) => Some(InResponse::Rejected),
                }
            }
            VPRBRD_USB_REQUEST_GPIOB => {
                // GPIO-B IN: read back port values
                match (self.read_b)() {
                    Ok(val) => {
                        let resp = GpioBMsg::new_readback(val);
                        let n = resp.to_bytes(buf);
                        if n > 0 {
                            Some(InResponse::Accepted(&buf[..n]))
                        } else {
                            Some(InResponse::Rejected)
                        }
                    }
                    Err(_) => Some(InResponse::Rejected),
                }
            }
            _ => None,
        }
    }
}

/// Builder helper to configure USB descriptors for viperboard detection.
///
/// The Linux `mfd/viperboard.c` driver matches on VID:PID only
/// (`USB_DEVICE(0x2058, 0x1005)`), so the interface class doesn't
/// matter for binding. However, we set vendor-specific for correctness.
///
/// Call this during your embassy-usb builder setup:
///
/// ```ignore
/// use embassy_usb::Builder;
///
/// let mut config = embassy_usb::Config::new(VPRBRD_VID, VPRBRD_PID);
/// config.product = Some("Viperboard GPIO");
/// config.manufacturer = Some("Nano River Technologies");
///
/// let mut builder = Builder::new(driver, config, ...);
/// add_vprbrd_interface(&mut builder, &mut handler);
/// ```
pub fn add_vprbrd_interface<'d, D, RA, WA, DA, RB, WB, DB>(
    builder: &mut embassy_usb::Builder<'d, D>,
    handler: &'d mut VprbrdGpioHandler<'d, RA, WA, DA, RB, WB, DB>,
) where
    D: embassy_usb::driver::Driver<'d>,
    RA: FnMut(u8) -> Result<bool, GpioError>,
    WA: FnMut(u8, bool) -> Result<(), GpioError>,
    DA: FnMut(u8, PinDirection) -> Result<(), GpioError>,
    RB: FnMut() -> Result<u16, GpioError>,
    WB: FnMut(u16, u16) -> Result<(), GpioError>,
    DB: FnMut(u16) -> Result<(), GpioError>,
{
    {
        let mut func = builder.function(0xFF, 0x00, 0x00);
        let mut iface = func.interface();
        let _alt = iface.alt_setting(0xFF, 0x00, 0x00, None);
        // No bulk/interrupt endpoints — all GPIO is done via EP0 control transfers
    }

    builder.handler(handler);
}
