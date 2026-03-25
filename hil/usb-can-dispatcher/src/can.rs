//! Bridge between gs_usb host frames and the `embedded-can` traits.
//!
//! This module provides adapter types that implement the `embedded_can`
//! traits, allowing gs_usb frames to interoperate with any CAN driver
//! that uses the standard embedded-can API.

use embedded_can::{ExtendedId, Frame, Id, StandardId};

/// CAN ID flag bits as used in Linux SocketCAN / gs_usb `can_id` field.
pub mod can_id_flags {
    /// Extended frame format (29-bit ID).
    pub const EFF_FLAG: u32 = 0x8000_0000;
    /// Remote transmission request.
    pub const RTR_FLAG: u32 = 0x4000_0000;
    /// Error frame.
    pub const ERR_FLAG: u32 = 0x2000_0000;

    /// Mask for standard 11-bit ID.
    pub const SFF_MASK: u32 = 0x0000_07FF;
    /// Mask for extended 29-bit ID.
    pub const EFF_MASK: u32 = 0x1FFF_FFFF;
}

/// A CAN frame compatible with `embedded_can::Frame`.
///
/// This can be constructed from a gs_usb `HostFrame` or from any
/// `embedded_can::Frame` implementation, providing bidirectional
/// conversion.
#[derive(Debug, Clone)]
pub struct GsCanFrame {
    id: Id,
    rtr: bool,
    dlc: usize,
    data: [u8; 8],
}

#[cfg(feature = "defmt")]
impl defmt::Format for GsCanFrame {
    fn format(&self, fmt: defmt::Formatter) {
        let raw_id = self.to_raw_can_id();
        defmt::write!(
            fmt,
            "GsCanFrame {{ id: {=u32:#x}, rtr: {=bool}, dlc: {=usize}, data: {=[u8]} }}",
            raw_id,
            self.rtr,
            self.dlc,
            &self.data[..self.dlc],
        );
    }
}

impl GsCanFrame {
    /// Create from a gs_usb host frame.
    pub fn from_host_frame(hf: &crate::protocol::HostFrame) -> Self {
        let raw_id = hf.can_id;
        let rtr = (raw_id & can_id_flags::RTR_FLAG) != 0;

        let id = if (raw_id & can_id_flags::EFF_FLAG) != 0 {
            let raw = raw_id & can_id_flags::EFF_MASK;
            // SAFETY: ExtendedId::new() returns None only if > 29 bits,
            // which can't happen after masking with EFF_MASK.
            Id::Extended(ExtendedId::new(raw).unwrap_or(ExtendedId::ZERO))
        } else {
            let raw = (raw_id & can_id_flags::SFF_MASK) as u16;
            Id::Standard(StandardId::new(raw).unwrap_or(StandardId::ZERO))
        };

        let dlc = (hf.can_dlc as usize).min(8);
        let mut data = [0u8; 8];
        data[..dlc].copy_from_slice(&hf.data[..dlc]);

        Self { id, rtr, dlc, data }
    }

    /// Convert to a gs_usb host frame `can_id` field.
    pub fn to_raw_can_id(&self) -> u32 {
        let mut raw = match self.id {
            Id::Standard(sid) => sid.as_raw() as u32,
            Id::Extended(eid) => eid.as_raw() | can_id_flags::EFF_FLAG,
        };
        if self.rtr {
            raw |= can_id_flags::RTR_FLAG;
        }
        raw
    }
}

impl Frame for GsCanFrame {
    fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self> {
        if data.len() > 8 {
            return None;
        }
        let mut buf = [0u8; 8];
        buf[..data.len()].copy_from_slice(data);
        Some(Self {
            id: id.into(),
            rtr: false,
            dlc: data.len(),
            data: buf,
        })
    }

    fn new_remote(id: impl Into<Id>, dlc: usize) -> Option<Self> {
        if dlc > 8 {
            return None;
        }
        Some(Self {
            id: id.into(),
            rtr: true,
            dlc,
            data: [0u8; 8],
        })
    }

    fn is_extended(&self) -> bool {
        matches!(self.id, Id::Extended(_))
    }

    fn is_remote_frame(&self) -> bool {
        self.rtr
    }

    fn id(&self) -> Id {
        self.id
    }

    fn dlc(&self) -> usize {
        self.dlc
    }

    fn data(&self) -> &[u8] {
        &self.data[..self.dlc]
    }
}

/// Convert any `embedded_can::Frame` into a gs_usb `HostFrame` for sending to the host.
pub fn frame_to_host_frame(
    frame: &impl Frame,
    channel: u8,
    timestamp_us: Option<u32>,
) -> crate::protocol::HostFrame {
    let can_id = match frame.id() {
        Id::Standard(sid) => {
            let mut id = sid.as_raw() as u32;
            if frame.is_remote_frame() {
                id |= can_id_flags::RTR_FLAG;
            }
            id
        }
        Id::Extended(eid) => {
            let mut id = eid.as_raw() | can_id_flags::EFF_FLAG;
            if frame.is_remote_frame() {
                id |= can_id_flags::RTR_FLAG;
            }
            id
        }
    };

    let mut hf = crate::protocol::HostFrame::new_rx(channel, can_id, frame.data());
    hf.can_dlc = frame.dlc() as u8;
    if let Some(ts) = timestamp_us {
        hf = hf.with_timestamp(ts);
    }
    hf
}
