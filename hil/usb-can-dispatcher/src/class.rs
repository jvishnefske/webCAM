//! gs_usb USB class implementation for embassy-usb.
//!
//! This module implements the device side of the gs_usb protocol as an
//! `embassy_usb::Handler` + bulk endpoint pair. When plugged into a Linux
//! host, the device is recognized by the `gs_usb` kernel module and
//! appears as a SocketCAN `canX` network interface.
//!
//! # USB Descriptor Layout
//!
//! The gs_usb protocol uses a vendor-specific USB class:
//! - `bInterfaceClass = 0xFF` (vendor-specific)
//! - `bInterfaceSubClass = 0xFF`
//! - `bInterfaceProtocol = 0xFF`
//! - One Bulk IN endpoint (device → host: RX frames + TX echoes)
//! - One Bulk OUT endpoint (host → device: TX frames)
//! - Vendor control transfers on EP0 for configuration

use embassy_usb::control::{InResponse, OutResponse, Recipient, Request, RequestType};
use embassy_usb::driver::{Driver, EndpointIn, EndpointOut};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::{Builder, Handler};

use crate::protocol::*;

/// The USB endpoint size for bulk transfers (full-speed).
/// gs_usb frames fit within 64 bytes.
pub const GS_USB_EP_SIZE: u16 = 64;

/// VID/PID for candleLight-compatible devices (open hardware).
/// VID 0x1D50 is OpenMoko, PID 0x606F is gs_usb / candleLight.
pub const GS_USB_VID: u16 = 0x1D50;
pub const GS_USB_PID: u16 = 0x606F;

/// Alternative: candleLight "bytewerk" VID/PID.
pub const CANDLELIGHT_VID: u16 = 0x1209;
pub const CANDLELIGHT_PID: u16 = 0x2323;

/// Configuration for the gs_usb device.
#[derive(Debug, Clone)]
pub struct GsUsbConfig {
    /// Number of CAN channels (1–based).
    pub num_channels: u8,
    /// Software version reported to host.
    pub sw_version: u32,
    /// Hardware version reported to host.
    pub hw_version: u32,
    /// Feature flags (see `features::*`).
    pub features: u32,
    /// CAN clock frequency in Hz (e.g., 48_000_000 for 48 MHz).
    pub fclk_can: u32,
    /// Bit timing constants.
    pub tseg1_min: u32,
    pub tseg1_max: u32,
    pub tseg2_min: u32,
    pub tseg2_max: u32,
    pub sjw_max: u32,
    pub brp_min: u32,
    pub brp_max: u32,
    pub brp_inc: u32,
}

impl Default for GsUsbConfig {
    fn default() -> Self {
        // Reasonable defaults for a typical MCU CAN peripheral (e.g. STM32/RP2040 with MCP2515)
        Self {
            num_channels: 1,
            sw_version: 2,
            hw_version: 1,
            features: features::LISTEN_ONLY
                | features::LOOP_BACK
                | features::HW_TIMESTAMP
                | features::GET_STATE,
            fclk_can: 48_000_000,
            tseg1_min: 1,
            tseg1_max: 16,
            tseg2_min: 1,
            tseg2_max: 8,
            sjw_max: 4,
            brp_min: 1,
            brp_max: 1024,
            brp_inc: 1,
        }
    }
}

/// State shared between the control handler and the bulk endpoint tasks.
///
/// The handler processes vendor control requests on EP0 while bulk
/// endpoints run in separate async tasks.
pub struct GsUsbState {
    config: GsUsbConfig,
    /// Current channel state.
    channel_started: [bool; 4],
    /// Current mode flags per channel.
    channel_flags: [u32; 4],
    /// Whether HW timestamps are enabled.
    hw_timestamps: bool,
    /// Interface number assigned by the builder.
    interface: InterfaceNumber,
    /// Bit timing received from host (per channel).
    bit_timing: [Option<BitTiming>; 4],
}

impl GsUsbState {
    pub fn new(config: GsUsbConfig) -> Self {
        Self {
            config,
            channel_started: [false; 4],
            channel_flags: [0; 4],
            hw_timestamps: false,
            interface: InterfaceNumber(0),
            bit_timing: [None; 4],
        }
    }

    /// Returns true if the given channel is currently started.
    pub fn is_channel_started(&self, ch: u8) -> bool {
        self.channel_started
            .get(ch as usize)
            .copied()
            .unwrap_or(false)
    }

    /// Returns the bit timing for a channel, if configured.
    pub fn channel_bit_timing(&self, ch: u8) -> Option<&BitTiming> {
        self.bit_timing.get(ch as usize).and_then(|bt| bt.as_ref())
    }

    /// Returns true if HW timestamps are enabled.
    pub fn hw_timestamps_enabled(&self) -> bool {
        self.hw_timestamps
    }
}

/// Control request handler for gs_usb vendor requests.
///
/// This implements `embassy_usb::Handler` to process the vendor-specific
/// control transfers that the Linux gs_usb driver sends during probe,
/// open, and close.
pub struct GsUsbHandler<'a> {
    state: &'a mut GsUsbState,
}

impl<'a> GsUsbHandler<'a> {
    pub fn new(state: &'a mut GsUsbState) -> Self {
        Self { state }
    }

    /// Set the interface number (called by `add_gs_usb_interface`).
    pub fn set_interface(&mut self, iface: InterfaceNumber) {
        self.state.interface = iface;
    }

    fn build_device_config(&self) -> DeviceConfig {
        DeviceConfig::new(
            self.state.config.num_channels,
            self.state.config.sw_version,
            self.state.config.hw_version,
        )
    }

    fn build_bt_const(&self) -> BtConst {
        let cfg = &self.state.config;
        BtConst {
            feature: cfg.features.to_le_bytes(),
            fclk_can: cfg.fclk_can.to_le_bytes(),
            tseg1_min: cfg.tseg1_min.to_le_bytes(),
            tseg1_max: cfg.tseg1_max.to_le_bytes(),
            tseg2_min: cfg.tseg2_min.to_le_bytes(),
            tseg2_max: cfg.tseg2_max.to_le_bytes(),
            sjw_max: cfg.sjw_max.to_le_bytes(),
            brp_min: cfg.brp_min.to_le_bytes(),
            brp_max: cfg.brp_max.to_le_bytes(),
            brp_inc: cfg.brp_inc.to_le_bytes(),
        }
    }
}

impl<'a> Handler for GsUsbHandler<'a> {
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        // Only handle vendor requests to our interface
        if req.request_type != RequestType::Vendor {
            return None;
        }
        if req.recipient != Recipient::Interface {
            return None;
        }

        let breq = match BreqCode::from_u8(req.request) {
            Some(b) => b,
            None => return Some(OutResponse::Rejected),
        };

        match breq {
            BreqCode::HostFormat => {
                // Host sends byte order; we just accept it.
                // The candleLight firmware ignores this — we do too,
                // always using little-endian.
                Some(OutResponse::Accepted)
            }
            BreqCode::BitTiming => {
                let channel = req.value as u8;
                if let Some(bt) = BitTiming::from_bytes(data) {
                    if let Some(slot) = self.state.bit_timing.get_mut(channel as usize) {
                        *slot = Some(bt);
                        return Some(OutResponse::Accepted);
                    }
                }
                Some(OutResponse::Rejected)
            }
            BreqCode::Mode => {
                let channel = req.value as u8;
                if let Some(dm) = DeviceMode::from_bytes(data) {
                    if let Some(started) = self.state.channel_started.get_mut(channel as usize) {
                        if dm.is_start() {
                            *started = true;
                            if let Some(flags) = self.state.channel_flags.get_mut(channel as usize)
                            {
                                *flags = dm.flags();
                            }
                            self.state.hw_timestamps = (dm.flags() & mode_flags::HW_TIMESTAMP) != 0;
                        } else {
                            *started = false;
                            if let Some(flags) = self.state.channel_flags.get_mut(channel as usize)
                            {
                                *flags = 0;
                            }
                        }
                        return Some(OutResponse::Accepted);
                    }
                }
                Some(OutResponse::Rejected)
            }
            BreqCode::Identify => {
                // Accept identify request — integrator can poll state to blink LED
                Some(OutResponse::Accepted)
            }
            BreqCode::DataBitTiming => {
                // CAN FD data bit timing — accept but store would need FD support
                Some(OutResponse::Accepted)
            }
            BreqCode::SetTermination => Some(OutResponse::Accepted),
            _ => Some(OutResponse::Rejected),
        }
    }

    fn control_in<'b>(&'b mut self, req: Request, buf: &'b mut [u8]) -> Option<InResponse<'b>> {
        if req.request_type != RequestType::Vendor {
            return None;
        }
        if req.recipient != Recipient::Interface {
            return None;
        }

        let breq = match BreqCode::from_u8(req.request) {
            Some(b) => b,
            None => return Some(InResponse::Rejected),
        };

        match breq {
            BreqCode::DeviceConfig => {
                let dc = self.build_device_config();
                let bytes = dc.as_bytes();
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                Some(InResponse::Accepted(&buf[..len]))
            }
            BreqCode::BtConst => {
                let btc = self.build_bt_const();
                let bytes = btc.as_bytes();
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                Some(InResponse::Accepted(&buf[..len]))
            }
            BreqCode::Timestamp => {
                // Return a 32-bit LE timestamp
                // The integrator should replace this with a real timer
                let ts: u32 = 0;
                let bytes = ts.to_le_bytes();
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                Some(InResponse::Accepted(&buf[..len]))
            }
            BreqCode::GetState => {
                let ds = DeviceState::new(CanState::ErrorActive, 0, 0);
                let bytes = ds.as_bytes();
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                Some(InResponse::Accepted(&buf[..len]))
            }
            BreqCode::GetTermination => {
                let state: u32 = 0; // off
                let bytes = state.to_le_bytes();
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                Some(InResponse::Accepted(&buf[..len]))
            }
            _ => Some(InResponse::Rejected),
        }
    }
}

/// Builder helper to add the gs_usb interface to an embassy-usb device.
///
/// This configures the USB descriptors so the Linux `gs_usb` kernel driver
/// recognizes the device.
///
/// Returns the bulk IN and OUT endpoints for use in async tasks.
pub fn add_gs_usb_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    handler: &'d mut GsUsbHandler<'d>,
    ep_size: u16,
) -> (D::EndpointIn, D::EndpointOut) {
    let (eps, if_num) = {
        let mut func = builder.function(0xFF, 0xFF, 0xFF);
        let mut iface = func.interface();
        let if_num = iface.interface_number();

        let mut alt = iface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let ep_in = alt.endpoint_bulk_in(ep_size);
        let ep_out = alt.endpoint_bulk_out(ep_size);
        ((ep_in, ep_out), if_num)
    };

    handler.set_interface(if_num);
    builder.handler(handler);

    eps
}

/// Async task: reads TX frames from host via bulk OUT endpoint.
///
/// The caller provides a callback/channel to handle each received TX frame.
/// For each frame, the integrator should:
/// 1. Transmit the CAN frame on the physical bus
/// 2. Send back a TX-complete echo via the bulk IN endpoint
///
/// # Example integration pattern
/// ```ignore
/// loop {
///     let frame = gs_usb_read_tx(&mut ep_out, &mut buf).await;
///     if let Some(frame) = frame {
///         // Send to CAN peripheral
///         can.transmit(&make_can_frame(&frame)).await;
///         // Echo back for TX completion
///         tx_echo_sender.send(frame).await;
///     }
/// }
/// ```
pub async fn read_host_tx_frame<E: EndpointOut>(
    ep_out: &mut E,
    buf: &mut [u8],
) -> Option<HostFrame> {
    match ep_out.read(buf).await {
        Ok(n) => HostFrame::from_bulk_out(&buf[..n]),
        Err(_) => None,
    }
}

/// Send a frame (RX or TX echo) to the host via bulk IN endpoint.
pub async fn write_host_frame<E: EndpointIn>(
    ep_in: &mut E,
    frame: &HostFrame,
    buf: &mut [u8],
    include_timestamp: bool,
) -> Result<(), embassy_usb::driver::EndpointError> {
    let n = frame.to_bulk_in(buf, include_timestamp);
    if n > 0 {
        ep_in.write(&buf[..n]).await?;
    }
    Ok(())
}
