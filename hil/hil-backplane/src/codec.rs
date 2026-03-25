//! Fixed-buffer encode/decode helpers for backplane datagrams.
//!
//! These functions replace `Vec`-based allocation with caller-provided
//! `&mut [u8]` buffers, making them suitable for `no_std` environments.

use crate::envelope::{Envelope, ENVELOPE_SIZE};
use crate::error::BackplaneError;
use crate::message::BackplaneMessage;

/// Encodes an envelope and CBOR message payload into `buf`.
///
/// Writes the 17-byte envelope header followed by the CBOR-encoded
/// payload. Returns the total number of bytes written.
///
/// # Errors
///
/// Returns [`BackplaneError::BufferTooSmall`] if `buf` is shorter than
/// [`ENVELOPE_SIZE`] or cannot hold the encoded payload.
/// Returns [`BackplaneError::EncodeFailed`] if CBOR encoding fails.
pub fn encode_to_slice<M: BackplaneMessage>(
    envelope: &Envelope,
    msg: &M,
    buf: &mut [u8],
) -> Result<usize, BackplaneError> {
    if buf.len() < ENVELOPE_SIZE {
        return Err(BackplaneError::BufferTooSmall);
    }

    let header = envelope.to_bytes();
    buf[..ENVELOPE_SIZE].copy_from_slice(&header);

    let payload_buf = &mut buf[ENVELOPE_SIZE..];
    let before_len = payload_buf.len();
    // minicbor implements Write for &mut [u8], shrinking the slice as bytes are written.
    let mut writer: &mut [u8] = payload_buf;
    minicbor::encode(msg, &mut writer).map_err(|_| BackplaneError::EncodeFailed)?;
    let after_len = writer.len();
    let payload_len = before_len - after_len;

    Ok(ENVELOPE_SIZE + payload_len)
}

/// Decodes an envelope from a datagram buffer.
///
/// Returns the parsed [`Envelope`] and a slice containing the CBOR
/// payload bytes (everything after the 17-byte header).
///
/// # Errors
///
/// Returns [`BackplaneError::InvalidEnvelope`] if `buf` is too short
/// or contains an invalid kind byte.
pub fn decode_envelope(buf: &[u8]) -> Result<(Envelope, &[u8]), BackplaneError> {
    let envelope = Envelope::from_bytes(buf)?;
    let payload = &buf[ENVELOPE_SIZE..];
    Ok((envelope, payload))
}
