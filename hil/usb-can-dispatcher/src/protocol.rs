//! gs_usb wire protocol types.
//!
//! These types match the Linux kernel `gs_usb.c` driver protocol exactly.
//! All multi-byte fields are little-endian on the wire.

/// USB vendor-specific control request codes.
///
/// These are sent as `bRequest` in USB control transfers with
/// `bmRequestType = USB_DIR_OUT | USB_TYPE_VENDOR | USB_RECIP_INTERFACE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum BreqCode {
    /// Host sends byte order indicator (always 0x0000beef LE).
    HostFormat = 0,
    /// Host sends bit timing configuration.
    BitTiming = 1,
    /// Host sends mode (start/reset).
    Mode = 2,
    /// Bus error request.
    Berr = 3,
    /// Device returns bit timing constants.
    BtConst = 4,
    /// Device returns device config (interface count, versions).
    DeviceConfig = 5,
    /// Timestamp request.
    Timestamp = 6,
    /// Identify (blink LED).
    Identify = 7,
    /// Get user-defined ID.
    GetUserId = 8,
    /// Set user-defined ID.
    SetUserId = 9,
    /// Data bit timing (CAN FD).
    DataBitTiming = 10,
    /// Extended bit timing constants (CAN FD).
    BtConstExt = 11,
    /// Set termination resistor.
    SetTermination = 12,
    /// Get termination state.
    GetTermination = 13,
    /// Get CAN channel state.
    GetState = 14,
}

impl BreqCode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::HostFormat),
            1 => Some(Self::BitTiming),
            2 => Some(Self::Mode),
            3 => Some(Self::Berr),
            4 => Some(Self::BtConst),
            5 => Some(Self::DeviceConfig),
            6 => Some(Self::Timestamp),
            7 => Some(Self::Identify),
            8 => Some(Self::GetUserId),
            9 => Some(Self::SetUserId),
            10 => Some(Self::DataBitTiming),
            11 => Some(Self::BtConstExt),
            12 => Some(Self::SetTermination),
            13 => Some(Self::GetTermination),
            14 => Some(Self::GetState),
            _ => None,
        }
    }
}

/// Feature flags advertised by the device in `gs_device_bt_const::feature`.
pub mod features {
    pub const LISTEN_ONLY: u32 = 1 << 0;
    pub const LOOP_BACK: u32 = 1 << 1;
    pub const TRIPLE_SAMPLE: u32 = 1 << 2;
    pub const ONE_SHOT: u32 = 1 << 3;
    pub const HW_TIMESTAMP: u32 = 1 << 4;
    pub const IDENTIFY: u32 = 1 << 5;
    pub const USER_ID: u32 = 1 << 6;
    pub const PAD_PKTS_TO_MAX_PKT_SIZE: u32 = 1 << 7;
    pub const FD: u32 = 1 << 8;
    pub const REQ_USB_QUIRK_LPC546XX: u32 = 1 << 9;
    pub const BT_CONST_EXT: u32 = 1 << 10;
    pub const TERMINATION: u32 = 1 << 11;
    pub const BERR_REPORTING: u32 = 1 << 12;
    pub const GET_STATE: u32 = 1 << 13;
}

/// Mode flags sent by host in `gs_device_mode::flags`.
pub mod mode_flags {
    pub const NORMAL: u32 = 0;
    pub const LISTEN_ONLY: u32 = 1 << 0;
    pub const LOOP_BACK: u32 = 1 << 1;
    pub const TRIPLE_SAMPLE: u32 = 1 << 2;
    pub const ONE_SHOT: u32 = 1 << 3;
    pub const HW_TIMESTAMP: u32 = 1 << 4;
    pub const PAD_PKTS_TO_MAX_PKT_SIZE: u32 = 1 << 7;
    pub const FD: u32 = 1 << 8;
    pub const BERR_REPORTING: u32 = 1 << 12;
}

/// Host frame flags in `gs_host_frame::flags`.
pub mod frame_flags {
    pub const OVERFLOW: u8 = 1 << 0;
    pub const FD: u8 = 1 << 1;
    pub const BRS: u8 = 1 << 2;
    pub const ESI: u8 = 1 << 3;
}

/// CAN channel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u32)]
pub enum CanState {
    ErrorActive = 0,
    ErrorWarning = 1,
    ErrorPassive = 2,
    BusOff = 3,
    Stopped = 4,
    Sleeping = 5,
}

// ──────────────────────────────────────────────────────────────────
// Wire-format structs — all little-endian, packed
// ──────────────────────────────────────────────────────────────────

/// Sent by host during probe: `BREQ_HOST_FORMAT`.
/// Always contains `0x0000beef` in LE.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct HostConfig {
    pub byte_order: [u8; 4],
}

impl HostConfig {
    pub const EXPECTED: [u8; 4] = [0xef, 0xbe, 0x00, 0x00]; // 0x0000beef LE

    pub fn is_valid(&self) -> bool {
        self.byte_order == Self::EXPECTED
    }
}

/// Device config returned on `BREQ_DEVICE_CONFIG`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DeviceConfig {
    pub reserved1: u8,
    pub reserved2: u8,
    pub reserved3: u8,
    /// Number of CAN interfaces minus 1.
    pub icount: u8,
    pub sw_version: [u8; 4],
    pub hw_version: [u8; 4],
}

impl DeviceConfig {
    pub fn new(num_channels: u8, sw_version: u32, hw_version: u32) -> Self {
        Self {
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            icount: num_channels.saturating_sub(1),
            sw_version: sw_version.to_le_bytes(),
            hw_version: hw_version.to_le_bytes(),
        }
    }

    /// Serialize to a byte array.
    pub fn as_bytes(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0] = self.reserved1;
        buf[1] = self.reserved2;
        buf[2] = self.reserved3;
        buf[3] = self.icount;
        buf[4..8].copy_from_slice(&self.sw_version);
        buf[8..12].copy_from_slice(&self.hw_version);
        buf
    }
}

/// Bit timing constants, returned on `BREQ_BT_CONST`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BtConst {
    pub feature: [u8; 4],
    pub fclk_can: [u8; 4],
    pub tseg1_min: [u8; 4],
    pub tseg1_max: [u8; 4],
    pub tseg2_min: [u8; 4],
    pub tseg2_max: [u8; 4],
    pub sjw_max: [u8; 4],
    pub brp_min: [u8; 4],
    pub brp_max: [u8; 4],
    pub brp_inc: [u8; 4],
}

impl BtConst {
    /// Serialize to a byte array.
    pub fn as_bytes(&self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..4].copy_from_slice(&self.feature);
        buf[4..8].copy_from_slice(&self.fclk_can);
        buf[8..12].copy_from_slice(&self.tseg1_min);
        buf[12..16].copy_from_slice(&self.tseg1_max);
        buf[16..20].copy_from_slice(&self.tseg2_min);
        buf[20..24].copy_from_slice(&self.tseg2_max);
        buf[24..28].copy_from_slice(&self.sjw_max);
        buf[28..32].copy_from_slice(&self.brp_min);
        buf[32..36].copy_from_slice(&self.brp_max);
        buf[36..40].copy_from_slice(&self.brp_inc);
        buf
    }
}

/// Bit timing sent by host on `BREQ_BITTIMING`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BitTiming {
    pub prop_seg: [u8; 4],
    pub phase_seg1: [u8; 4],
    pub phase_seg2: [u8; 4],
    pub sjw: [u8; 4],
    pub brp: [u8; 4],
}

impl BitTiming {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < core::mem::size_of::<Self>() {
            return None;
        }
        let mut bt = Self {
            prop_seg: [0; 4],
            phase_seg1: [0; 4],
            phase_seg2: [0; 4],
            sjw: [0; 4],
            brp: [0; 4],
        };
        bt.prop_seg.copy_from_slice(&data[0..4]);
        bt.phase_seg1.copy_from_slice(&data[4..8]);
        bt.phase_seg2.copy_from_slice(&data[8..12]);
        bt.sjw.copy_from_slice(&data[12..16]);
        bt.brp.copy_from_slice(&data[16..20]);
        Some(bt)
    }

    pub fn prop_seg(&self) -> u32 {
        u32::from_le_bytes(self.prop_seg)
    }
    pub fn phase_seg1(&self) -> u32 {
        u32::from_le_bytes(self.phase_seg1)
    }
    pub fn phase_seg2(&self) -> u32 {
        u32::from_le_bytes(self.phase_seg2)
    }
    pub fn sjw(&self) -> u32 {
        u32::from_le_bytes(self.sjw)
    }
    pub fn brp(&self) -> u32 {
        u32::from_le_bytes(self.brp)
    }
}

/// Device mode sent by host on `BREQ_MODE`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DeviceMode {
    pub mode: [u8; 4],
    pub flags: [u8; 4],
}

impl DeviceMode {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let mut dm = Self {
            mode: [0; 4],
            flags: [0; 4],
        };
        dm.mode.copy_from_slice(&data[0..4]);
        dm.flags.copy_from_slice(&data[4..8]);
        Some(dm)
    }

    pub fn mode(&self) -> u32 {
        u32::from_le_bytes(self.mode)
    }

    pub fn flags(&self) -> u32 {
        u32::from_le_bytes(self.flags)
    }

    pub fn is_start(&self) -> bool {
        self.mode() == 1
    }

    pub fn is_reset(&self) -> bool {
        self.mode() == 0
    }
}

/// Device state returned on `BREQ_GET_STATE`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DeviceState {
    pub state: [u8; 4],
    pub rxerr: [u8; 4],
    pub txerr: [u8; 4],
}

impl DeviceState {
    pub fn new(state: CanState, rxerr: u32, txerr: u32) -> Self {
        Self {
            state: (state as u32).to_le_bytes(),
            rxerr: rxerr.to_le_bytes(),
            txerr: txerr.to_le_bytes(),
        }
    }

    /// Serialize to a byte array.
    pub fn as_bytes(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&self.state);
        buf[4..8].copy_from_slice(&self.rxerr);
        buf[8..12].copy_from_slice(&self.txerr);
        buf
    }
}

/// Identify mode sent/received on `BREQ_IDENTIFY`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdentifyMode {
    pub mode: [u8; 4],
}

impl IdentifyMode {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let mut im = Self { mode: [0; 4] };
        im.mode.copy_from_slice(&data[0..4]);
        Some(im)
    }

    pub fn is_on(&self) -> bool {
        u32::from_le_bytes(self.mode) == 1
    }
}

// ──────────────────────────────────────────────────────────────────
// Host frame — the bulk transfer payload
// ──────────────────────────────────────────────────────────────────

/// Echo ID indicating a received frame (not a TX echo).
pub const ECHO_ID_RX: u32 = 0xFFFF_FFFF;

/// Maximum classic CAN data length.
pub const CAN_DATA_MAX: usize = 8;

/// Maximum CAN FD data length.
#[cfg(feature = "canfd")]
pub const CANFD_DATA_MAX: usize = 64;

/// Size of the host frame header (before data).
pub const HOST_FRAME_HEADER_SIZE: usize = 4 + 4 + 1 + 1 + 1 + 1; // 12 bytes

/// Classic CAN host frame: 12-byte header + 8 bytes data = 20 bytes.
pub const CLASSIC_CAN_FRAME_SIZE: usize = HOST_FRAME_HEADER_SIZE + CAN_DATA_MAX;

/// Classic CAN host frame with timestamp: 20 + 4 = 24 bytes.
pub const CLASSIC_CAN_TS_FRAME_SIZE: usize = CLASSIC_CAN_FRAME_SIZE + 4;

/// A gs_usb host frame, parsed from bulk transfers.
///
/// This is the primary data exchange unit between host and device.
/// - For TX: host sends frame, device echoes back with same `echo_id` on completion.
/// - For RX: device sends frame with `echo_id == 0xFFFFFFFF`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HostFrame {
    /// Echo ID: 0xFFFFFFFF for RX frames, sender-assigned for TX frames.
    pub echo_id: u32,
    /// CAN ID (includes EFF/RTR/ERR flags in bits 31:29).
    pub can_id: u32,
    /// Data length code.
    pub can_dlc: u8,
    /// Channel number (0-based).
    pub channel: u8,
    /// Frame flags (see `frame_flags`).
    pub flags: u8,
    /// Data payload.
    pub data: [u8; 8],
    /// Optional hardware timestamp in microseconds.
    pub timestamp_us: Option<u32>,
}

impl HostFrame {
    /// Create a new RX frame to send to the host.
    pub fn new_rx(channel: u8, can_id: u32, data: &[u8]) -> Self {
        let dlc = data.len().min(8) as u8;
        let mut frame_data = [0u8; 8];
        frame_data[..dlc as usize].copy_from_slice(&data[..dlc as usize]);
        Self {
            echo_id: ECHO_ID_RX,
            can_id,
            can_dlc: dlc,
            channel,
            flags: 0,
            data: frame_data,
            timestamp_us: None,
        }
    }

    /// Create a TX-complete echo frame.
    pub fn new_tx_echo(echo_id: u32, channel: u8, can_id: u32, dlc: u8, data: &[u8]) -> Self {
        let len = (dlc as usize).min(8);
        let mut frame_data = [0u8; 8];
        frame_data[..len].copy_from_slice(&data[..len]);
        Self {
            echo_id,
            can_id,
            can_dlc: dlc,
            channel,
            flags: 0,
            data: frame_data,
            timestamp_us: None,
        }
    }

    /// Parse a host frame from a bulk OUT transfer (host → device TX request).
    pub fn from_bulk_out(data: &[u8]) -> Option<Self> {
        if data.len() < HOST_FRAME_HEADER_SIZE + 1 {
            return None;
        }
        let echo_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let can_id = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let can_dlc = data[8];
        let channel = data[9];
        let flags = data[10];
        // data[11] is reserved

        let payload_len = (can_dlc as usize).min(8);
        if data.len() < HOST_FRAME_HEADER_SIZE + payload_len {
            return None;
        }

        let mut frame_data = [0u8; 8];
        frame_data[..payload_len]
            .copy_from_slice(&data[HOST_FRAME_HEADER_SIZE..HOST_FRAME_HEADER_SIZE + payload_len]);

        Some(Self {
            echo_id,
            can_id,
            can_dlc,
            channel,
            flags,
            data: frame_data,
            timestamp_us: None,
        })
    }

    /// Serialize to bytes for bulk IN transfer (device → host).
    /// Returns number of bytes written.
    pub fn to_bulk_in(&self, buf: &mut [u8], include_timestamp: bool) -> usize {
        let payload_len = (self.can_dlc as usize).min(8);
        let total = if include_timestamp {
            HOST_FRAME_HEADER_SIZE + payload_len + 4
        } else {
            HOST_FRAME_HEADER_SIZE + payload_len
        };

        if buf.len() < total {
            return 0;
        }

        buf[0..4].copy_from_slice(&self.echo_id.to_le_bytes());
        buf[4..8].copy_from_slice(&self.can_id.to_le_bytes());
        buf[8] = self.can_dlc;
        buf[9] = self.channel;
        buf[10] = self.flags;
        buf[11] = 0; // reserved

        buf[HOST_FRAME_HEADER_SIZE..HOST_FRAME_HEADER_SIZE + payload_len]
            .copy_from_slice(&self.data[..payload_len]);

        if include_timestamp {
            let ts = self.timestamp_us.unwrap_or(0);
            let off = HOST_FRAME_HEADER_SIZE + payload_len;
            buf[off..off + 4].copy_from_slice(&ts.to_le_bytes());
        }

        total
    }

    /// Set the hardware timestamp.
    pub fn with_timestamp(mut self, timestamp_us: u32) -> Self {
        self.timestamp_us = Some(timestamp_us);
        self
    }
}
