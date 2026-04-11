//! CBOR tag 40 encode/decode for DAP-over-WebSocket transport.
//!
//! DAP commands are wrapped in a CBOR map with tag 40:
//!
//! ```text
//! Request:  {0: 40, 1: h'<raw DAP bytes>'}
//! Response: {0: 40, 1: h'<raw DAP bytes>'}
//! ```
//!
//! This module provides helpers to encode, decode, and dispatch these
//! messages without heap allocation.

use crate::protocol::DapProcessor;

/// CBOR map key 0 value identifying DAP command messages.
pub const DAP_COMMAND_TAG: u32 = 40;

/// Error type for DAP CBOR dispatch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CborDispatchError {
    /// The CBOR data is malformed or cannot be decoded.
    MalformedCbor,
    /// The CBOR tag is not the expected DAP command tag (40).
    WrongTag,
    /// The output buffer is too small for the encoded response.
    BufferTooSmall,
    /// The DAP processor returned an empty (zero-length) response.
    EmptyResponse,
}

/// Returns `true` if `tag` is the DAP command tag (40).
pub fn is_dap_tag(tag: u32) -> bool {
    tag == DAP_COMMAND_TAG
}

/// Extracts the raw DAP command bytes from a CBOR tag-40 request.
///
/// Expected wire format: `{0: 40, 1: h'...'}`.
/// Returns the byte string payload from key 1.
///
/// # Errors
///
/// Returns [`CborDispatchError::MalformedCbor`] if the CBOR cannot be decoded,
/// or [`CborDispatchError::WrongTag`] if the tag is not 40.
pub fn decode_dap_request(cbor: &[u8]) -> Result<&[u8], CborDispatchError> {
    let mut dec = minicbor::Decoder::new(cbor);

    let _map_len = dec.map().map_err(|_| CborDispatchError::MalformedCbor)?;

    // Key 0: tag
    let _k0 = dec.u32().map_err(|_| CborDispatchError::MalformedCbor)?;
    let tag = dec.u32().map_err(|_| CborDispatchError::MalformedCbor)?;
    if tag != DAP_COMMAND_TAG {
        return Err(CborDispatchError::WrongTag);
    }

    // Key 1: DAP bytes
    let _k1 = dec.u32().map_err(|_| CborDispatchError::MalformedCbor)?;
    let data = dec.bytes().map_err(|_| CborDispatchError::MalformedCbor)?;

    Ok(data)
}

/// Encodes a DAP response as a CBOR tag-40 message.
///
/// Wire format: `{0: 40, 1: h'<dap_data>'}`.
/// Returns the number of bytes written into `buf`.
///
/// # Errors
///
/// Returns [`CborDispatchError::BufferTooSmall`] if `buf` is too small for the encoded response.
pub fn encode_dap_response(buf: &mut [u8], dap_data: &[u8]) -> Result<usize, CborDispatchError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| CborDispatchError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| CborDispatchError::BufferTooSmall)?;
    enc.u32(DAP_COMMAND_TAG)
        .map_err(|_| CborDispatchError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| CborDispatchError::BufferTooSmall)?;
    enc.bytes(dap_data)
        .map_err(|_| CborDispatchError::BufferTooSmall)?;

    // End the encoder's borrow on `writer` so we can measure remaining capacity.
    let _ = enc;
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Full decode → process → encode pipeline for a DAP-over-CBOR request.
///
/// 1. Decodes the CBOR tag-40 request to extract raw DAP bytes
/// 2. Passes them through [`DapProcessor::process_command`]
/// 3. Encodes the response as a CBOR tag-40 message
///
/// Returns the number of bytes written into `resp_buf`.
///
/// # Errors
///
/// Returns [`CborDispatchError::MalformedCbor`] or [`CborDispatchError::WrongTag`]
/// if CBOR decoding fails, [`CborDispatchError::EmptyResponse`] if the processor
/// returns 0 bytes, or [`CborDispatchError::BufferTooSmall`] if the response buffer
/// is too small.
pub fn handle_dap_request<P: DapProcessor + ?Sized>(
    dap: &mut P,
    request: &[u8],
    resp_buf: &mut [u8],
) -> Result<usize, CborDispatchError> {
    let dap_bytes = decode_dap_request(request)?;

    // Use a fixed intermediate buffer for the raw DAP response.
    // CMSIS-DAP v2 max packet size is 512 bytes.
    let mut dap_resp = [0u8; 512];
    let resp_len = dap.process_command(dap_bytes, &mut dap_resp);

    if resp_len == 0 {
        return Err(CborDispatchError::EmptyResponse);
    }

    encode_dap_response(resp_buf, &dap_resp[..resp_len])
}

/// Peeks at the tag value (key 0) in a CBOR map without consuming the input.
///
/// Returns `Some(tag)` if the first key-value pair is `0: <u32>`,
/// or `None` if the data is not a valid CBOR map or the first key
/// is not 0.
pub fn peek_cbor_tag(data: &[u8]) -> Option<u32> {
    let mut dec = minicbor::Decoder::new(data);
    let _map_len = dec.map().ok()?;
    let k0 = dec.u32().ok()?;
    if k0 != 0 {
        return None;
    }
    dec.u32().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock DAP processor that echoes the first byte + 1.
    struct EchoDap;

    impl DapProcessor for EchoDap {
        fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
            if request.is_empty() {
                return 0;
            }
            response[0] = request[0].wrapping_add(1);
            1
        }
    }

    /// Mock DAP processor that returns 0 bytes (no response).
    struct EmptyDap;

    impl DapProcessor for EmptyDap {
        fn process_command(&mut self, _request: &[u8], _response: &mut [u8]) -> usize {
            0
        }
    }

    /// Helper: encode a tag-40 request with the given DAP bytes.
    fn encode_request(buf: &mut [u8], dap_bytes: &[u8]) -> usize {
        let buf_len = buf.len();
        let mut writer: &mut [u8] = buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(2).unwrap();
        enc.u32(0).unwrap();
        enc.u32(DAP_COMMAND_TAG).unwrap();
        enc.u32(1).unwrap();
        enc.bytes(dap_bytes).unwrap();
        drop(enc);
        buf_len - writer.len()
    }

    #[test]
    fn is_dap_tag_true() {
        assert!(is_dap_tag(40));
    }

    #[test]
    fn is_dap_tag_false() {
        assert!(!is_dap_tag(1));
        assert!(!is_dap_tag(0));
        assert!(!is_dap_tag(255));
    }

    #[test]
    fn round_trip_encode_decode() {
        let dap_bytes = [0x01, 0x02, 0x03];
        let mut buf = [0u8; 64];
        let n = encode_dap_response(&mut buf, &dap_bytes).unwrap();

        let decoded = decode_dap_request(&buf[..n]).unwrap();
        assert_eq!(decoded, &dap_bytes);
    }

    #[test]
    fn decode_wrong_tag_fails() {
        // Encode with tag 1 instead of 40
        let mut buf = [0u8; 64];
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(2).unwrap();
        enc.u32(0).unwrap();
        enc.u32(1).unwrap(); // wrong tag
        enc.u32(1).unwrap();
        enc.bytes(&[0xAA]).unwrap();
        drop(enc);
        let n = buf_len - writer.len();

        assert!(decode_dap_request(&buf[..n]).is_err());
    }

    #[test]
    fn decode_malformed_cbor_fails() {
        assert!(decode_dap_request(&[0xFF]).is_err());
        assert!(decode_dap_request(&[]).is_err());
    }

    #[test]
    fn encode_response_buffer_too_small() {
        let mut buf = [0u8; 3];
        assert!(encode_dap_response(&mut buf, &[0x01, 0x02]).is_err());
    }

    #[test]
    fn handle_dap_request_echo() {
        let mut dap = EchoDap;
        let mut req = [0u8; 64];
        let req_len = encode_request(&mut req, &[0x42]);

        let mut resp = [0u8; 64];
        let n = handle_dap_request(&mut dap, &req[..req_len], &mut resp).unwrap();

        let decoded = decode_dap_request(&resp[..n]).unwrap();
        assert_eq!(decoded, &[0x43]); // 0x42 + 1
    }

    #[test]
    fn handle_dap_request_empty_response_fails() {
        let mut dap = EmptyDap;
        let mut req = [0u8; 64];
        let req_len = encode_request(&mut req, &[0x01]);

        let mut resp = [0u8; 64];
        assert!(handle_dap_request(&mut dap, &req[..req_len], &mut resp).is_err());
    }

    #[test]
    fn handle_dap_request_bad_cbor_fails() {
        let mut dap = EchoDap;
        let mut resp = [0u8; 64];
        assert!(handle_dap_request(&mut dap, &[0xFF], &mut resp).is_err());
    }

    #[test]
    fn peek_cbor_tag_dap() {
        let mut buf = [0u8; 64];
        let n = encode_request(&mut buf, &[0x01]);
        assert_eq!(peek_cbor_tag(&buf[..n]), Some(40));
    }

    #[test]
    fn peek_cbor_tag_i2c_read() {
        // Encode {0: 1, 1: 0}
        let mut buf = [0u8; 64];
        let buf_len = buf.len();
        let mut writer: &mut [u8] = &mut buf;
        let mut enc = minicbor::Encoder::new(&mut writer);
        enc.map(2).unwrap();
        enc.u32(0).unwrap();
        enc.u32(1).unwrap();
        enc.u32(1).unwrap();
        enc.u8(0).unwrap();
        drop(enc);
        let n = buf_len - writer.len();

        assert_eq!(peek_cbor_tag(&buf[..n]), Some(1));
    }

    #[test]
    fn peek_cbor_tag_empty() {
        assert_eq!(peek_cbor_tag(&[]), None);
    }

    #[test]
    fn peek_cbor_tag_malformed() {
        assert_eq!(peek_cbor_tag(&[0xFF]), None);
    }

    #[test]
    fn handle_dap_request_multi_byte() {
        struct MultiDap;
        impl DapProcessor for MultiDap {
            fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
                let len = request.len().min(response.len());
                let mut i = 0;
                while i < len {
                    response[i] = request[i];
                    i += 1;
                }
                len
            }
        }

        let mut dap = MultiDap;
        let payload = [0x01, 0x02, 0x03, 0x04, 0x05];
        let mut req = [0u8; 64];
        let req_len = encode_request(&mut req, &payload);

        let mut resp = [0u8; 64];
        let n = handle_dap_request(&mut dap, &req[..req_len], &mut resp).unwrap();

        let decoded = decode_dap_request(&resp[..n]).unwrap();
        assert_eq!(decoded, &payload);
    }

    #[test]
    fn handle_dap_request_resp_buf_too_small() {
        let mut dap = EchoDap;
        let mut req = [0u8; 64];
        let req_len = encode_request(&mut req, &[0x42]);

        let mut resp = [0u8; 3]; // too small
        assert!(handle_dap_request(&mut dap, &req[..req_len], &mut resp).is_err());
    }
}
