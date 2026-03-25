//! Wire format envelope for backplane datagrams.
//!
//! The envelope is a 17-byte little-endian header prepended to every
//! datagram. It carries routing metadata so receivers can dispatch
//! without decoding the CBOR payload.

use crate::error::BackplaneError;
use crate::node_id::NodeId;

/// Size of the envelope header in bytes.
pub const ENVELOPE_SIZE: usize = 17;

/// Classifies the intent of a backplane datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// Fire-and-forget broadcast to all subscribers.
    Publish,
    /// Directed request expecting a [`MessageKind::Response`].
    Request,
    /// Reply correlated to a prior request by `request_seq`.
    Response {
        /// Sequence number of the original request.
        request_seq: u32,
    },
}

/// Fixed-size header prepended to every backplane datagram.
///
/// ```text
/// Offset  Size  Field
/// 0       4     type_id      u32 LE — FNV-1a hash of message type
/// 4       4     seq          u32 LE — monotonic sequence number
/// 8       4     source       u32 LE — NodeId of sender
/// 12      1     kind         u8 — 0=Publish, 1=Request, 2=Response
/// 13      4     request_seq  u32 LE — for Response: seq of request; else 0
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// FNV-1a hash identifying the payload message type.
    pub type_id: u32,
    /// Monotonically increasing sequence number from the sender.
    pub seq: u32,
    /// Identity of the sending node.
    pub source: NodeId,
    /// Whether this is a publish, request, or response.
    pub kind: MessageKind,
}

impl Envelope {
    /// Serializes the envelope into a 17-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; ENVELOPE_SIZE] {
        let mut buf = [0u8; ENVELOPE_SIZE];
        buf[0..4].copy_from_slice(&self.type_id.to_le_bytes());
        buf[4..8].copy_from_slice(&self.seq.to_le_bytes());
        buf[8..12].copy_from_slice(&self.source.raw().to_le_bytes());
        let (kind_byte, request_seq) = match self.kind {
            MessageKind::Publish => (0u8, 0u32),
            MessageKind::Request => (1u8, 0u32),
            MessageKind::Response { request_seq } => (2u8, request_seq),
        };
        buf[12] = kind_byte;
        buf[13..17].copy_from_slice(&request_seq.to_le_bytes());
        buf
    }

    /// Deserializes an envelope from a byte buffer.
    ///
    /// Returns [`BackplaneError::InvalidEnvelope`] if the buffer is too
    /// short or contains an unrecognized kind byte.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, BackplaneError> {
        if buf.len() < ENVELOPE_SIZE {
            return Err(BackplaneError::InvalidEnvelope);
        }
        let type_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let seq = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let source = NodeId::new(u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]));
        let kind_byte = buf[12];
        let request_seq = u32::from_le_bytes([buf[13], buf[14], buf[15], buf[16]]);
        let kind = match kind_byte {
            0 => MessageKind::Publish,
            1 => MessageKind::Request,
            2 => MessageKind::Response { request_seq },
            _ => return Err(BackplaneError::InvalidEnvelope),
        };
        Ok(Self {
            type_id,
            seq,
            source,
            kind,
        })
    }
}
