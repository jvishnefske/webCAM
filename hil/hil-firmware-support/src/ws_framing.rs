//! Minimal WebSocket framing for embassy-net TCP sockets.
//!
//! Implements the subset of RFC 6455 needed for a single-connection
//! WebSocket server: HTTP upgrade handshake, binary frame read/write,
//! and close/ping handling. All operations use fixed-size buffers.
//!
//! Pure functions (base64, HTTP header parsing) are always available.
//! TCP socket operations require the `tcp` feature.

#[cfg(feature = "tcp")]
use embassy_net::tcp::TcpSocket;

/// A decoded WebSocket frame.
#[cfg(feature = "tcp")]
pub struct Frame<'a> {
    /// Frame opcode (1=text, 2=binary, 8=close, 9=ping, 10=pong).
    pub opcode: u8,
    /// Unmasked payload data, borrowed from the read buffer.
    pub payload: &'a [u8],
}

/// WebSocket protocol errors.
#[cfg(feature = "tcp")]
pub enum WsError {
    /// TCP read/write failed.
    Io,
    /// Frame too large for buffer.
    FrameTooLarge,
    /// Invalid frame format.
    Protocol,
    /// Connection closed by peer.
    Closed,
}

/// Standard base64 alphabet used for encoding SHA-1 digests.
const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes a byte slice as base64 into the output buffer.
///
/// Only needs to handle the 20-byte SHA-1 output (producing 28 base64
/// characters plus one `=` pad character). Returns the number of bytes
/// written to `output`.
///
/// # Panics
///
/// Panics if `output` is too small to hold the encoded result. The
/// required size is `4 * ((input.len() + 2) / 3)`.
pub fn base64_encode(input: &[u8], output: &mut [u8]) -> usize {
    let mut i = 0;
    let mut o = 0;
    while i + 2 < input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        output[o] = B64_CHARS[((b0 >> 2) & 0x3F) as usize];
        output[o + 1] = B64_CHARS[(((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F)) as usize];
        output[o + 2] = B64_CHARS[(((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03)) as usize];
        output[o + 3] = B64_CHARS[(b2 & 0x3F) as usize];
        i += 3;
        o += 4;
    }
    let remaining = input.len() - i;
    if remaining == 1 {
        let b0 = input[i];
        output[o] = B64_CHARS[((b0 >> 2) & 0x3F) as usize];
        output[o + 1] = B64_CHARS[((b0 & 0x03) << 4) as usize];
        output[o + 2] = b'=';
        output[o + 3] = b'=';
        o += 4;
    } else if remaining == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        output[o] = B64_CHARS[((b0 >> 2) & 0x3F) as usize];
        output[o + 1] = B64_CHARS[(((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F)) as usize];
        output[o + 2] = B64_CHARS[((b1 & 0x0F) << 2) as usize];
        output[o + 3] = b'=';
        o += 4;
    }
    o
}

/// Writes all bytes in `buf` to the TCP socket.
///
/// Loops until every byte has been sent. Returns [`WsError::Io`] on
/// any TCP write error. This is a standalone helper to avoid importing
/// the `embedded_io_async::Write` trait.
#[cfg(feature = "tcp")]
pub async fn write_all_to_socket(socket: &mut TcpSocket<'_>, buf: &[u8]) -> Result<(), WsError> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = socket
            .write(&buf[offset..])
            .await
            .map_err(|_| WsError::Io)?;
        if n == 0 {
            return Err(WsError::Io);
        }
        offset += n;
    }
    Ok(())
}

/// Reads exactly `buf.len()` bytes from the TCP socket.
///
/// Loops until the buffer is completely filled. Returns [`WsError::Closed`]
/// if the peer closes the connection before all bytes arrive, or
/// [`WsError::Io`] on any TCP error.
#[cfg(feature = "tcp")]
async fn read_exact(socket: &mut TcpSocket<'_>, buf: &mut [u8]) -> Result<(), WsError> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = socket
            .read(&mut buf[offset..])
            .await
            .map_err(|_| WsError::Io)?;
        if n == 0 {
            return Err(WsError::Closed);
        }
        offset += n;
    }
    Ok(())
}

/// Scans HTTP upgrade request headers for the `Sec-WebSocket-Key` value.
///
/// Performs a simple byte scan looking for the header name (case-insensitive
/// match on the canonical casing). Returns the 24-byte base64 key value
/// if found, or `None` if the header is absent or malformed.
pub fn parse_upgrade_key(buf: &[u8]) -> Option<[u8; 24]> {
    // Search for "Sec-WebSocket-Key: " (or "sec-websocket-key: ")
    // We scan line by line looking for the header.
    let needle_lower = b"sec-websocket-key: ";
    let needle_len = needle_lower.len();

    let mut pos = 0;
    while pos + needle_len < buf.len() {
        // Check if this position starts a matching header name
        let candidate = &buf[pos..];
        if candidate.len() >= needle_len {
            let mut matches = true;
            let mut j = 0;
            while j < needle_len {
                let c = candidate[j];
                let lo = if c.is_ascii_uppercase() { c + 32 } else { c };
                if lo != needle_lower[j] {
                    matches = false;
                    break;
                }
                j += 1;
            }
            if matches {
                // Found the header, extract the 24-byte key value
                let value_start = pos + needle_len;
                // Skip any leading whitespace after colon+space
                let mut vs = value_start;
                while vs < buf.len() && buf[vs] == b' ' {
                    vs += 1;
                }
                // Read up to \r or end, expecting exactly 24 bytes
                let mut end = vs;
                while end < buf.len() && buf[end] != b'\r' && buf[end] != b'\n' {
                    end += 1;
                }
                // Trim trailing whitespace
                while end > vs && buf[end - 1] == b' ' {
                    end -= 1;
                }
                let key_len = end - vs;
                if key_len == 24 {
                    let mut key = [0u8; 24];
                    key.copy_from_slice(&buf[vs..end]);
                    return Some(key);
                }
                return None;
            }
        }
        // Advance to next line
        while pos < buf.len() && buf[pos] != b'\n' {
            pos += 1;
        }
        if pos < buf.len() {
            pos += 1; // skip \n
        }
    }
    None
}

/// Builds the HTTP 101 Switching Protocols response for a WebSocket upgrade.
///
/// Concatenates the client key with the RFC 6455 magic GUID, computes the
/// SHA-1 hash, base64-encodes it, and writes the complete HTTP response
/// into `buf`. Returns the number of bytes written.
///
/// # Panics
///
/// Panics if `buf` is shorter than 256 bytes, which is always sufficient
/// for the fixed-format upgrade response.
pub fn build_upgrade_response(key: &[u8; 24], buf: &mut [u8]) -> usize {
    // Concatenate key + magic GUID and compute SHA-1
    let mut sha1 = sha1_smol::Sha1::new();
    sha1.update(key);
    sha1.update(b"258EAFA5-E914-47DA-95CA-5AB5D3F3ABDA");
    let hash = sha1.digest().bytes();

    // Base64-encode the 20-byte hash -> 28 characters
    let mut accept_b64 = [0u8; 28];
    let b64_len = base64_encode(&hash, &mut accept_b64);

    // Write the HTTP 101 response
    let header = b"HTTP/1.1 101 Switching Protocols\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Accept: ";
    let trailer = b"\r\n\r\n";

    let total = header.len() + b64_len + trailer.len();
    buf[..header.len()].copy_from_slice(header);
    buf[header.len()..header.len() + b64_len].copy_from_slice(&accept_b64[..b64_len]);
    buf[header.len() + b64_len..total].copy_from_slice(trailer);
    total
}

/// Reads a single WebSocket frame from the TCP socket.
///
/// Handles FIN bit, opcode extraction, 7-bit and 16-bit extended payload
/// lengths (64-bit extended length is rejected as too large), and the
/// 4-byte masking key required for client-to-server frames. The payload
/// is unmasked in-place within `buf`.
///
/// # Errors
///
/// - [`WsError::Closed`] if the TCP connection is closed.
/// - [`WsError::FrameTooLarge`] if the payload exceeds `buf` capacity.
/// - [`WsError::Protocol`] if the frame uses 64-bit extended length.
/// - [`WsError::Io`] on TCP read failure.
#[cfg(feature = "tcp")]
pub async fn read_frame<'a>(
    socket: &mut TcpSocket<'_>,
    buf: &'a mut [u8],
) -> Result<Frame<'a>, WsError> {
    // Read the 2-byte frame header
    let mut header = [0u8; 2];
    read_exact(socket, &mut header).await?;

    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let len_byte = header[1] & 0x7F;

    // Determine payload length
    let payload_len: usize = if len_byte <= 125 {
        len_byte as usize
    } else if len_byte == 126 {
        // 16-bit extended length
        let mut ext = [0u8; 2];
        read_exact(socket, &mut ext).await?;
        u16::from_be_bytes(ext) as usize
    } else {
        // 64-bit extended length is not supported on embedded
        return Err(WsError::Protocol);
    };

    if payload_len > buf.len() {
        return Err(WsError::FrameTooLarge);
    }

    // Read masking key if present
    let mut mask_key = [0u8; 4];
    if masked {
        read_exact(socket, &mut mask_key).await?;
    }

    // Read payload
    if payload_len > 0 {
        read_exact(socket, &mut buf[..payload_len]).await?;
    }

    // Unmask payload in-place
    if masked {
        let mut i = 0;
        while i < payload_len {
            buf[i] ^= mask_key[i & 3];
            i += 1;
        }
    }

    Ok(Frame {
        opcode,
        payload: &buf[..payload_len],
    })
}

/// Parses the HTTP method from a request line.
///
/// Extracts "GET", "POST", etc. from `GET /path HTTP/1.1\r\n...`.
pub fn parse_request_method(buf: &[u8]) -> Option<&str> {
    let end = buf.iter().position(|&b| b == b' ')?;
    core::str::from_utf8(&buf[..end]).ok()
}

/// Parses the URL path from an HTTP request line.
///
/// Extracts the path from `GET /path HTTP/1.1\r\n...` format.
/// Returns `None` if the request line is not valid UTF-8 or does not
/// contain the expected space-delimited structure.
pub fn parse_request_path(buf: &[u8]) -> Option<&str> {
    // Skip method (find first space after GET/POST/etc.)
    let start = buf.iter().position(|&b| b == b' ')? + 1;
    // Find end of path (second space before HTTP/1.1)
    let remaining = &buf[start..];
    let path_len = remaining.iter().position(|&b| b == b' ')?;
    core::str::from_utf8(&buf[start..start + path_len]).ok()
}

/// Writes a single WebSocket frame to the TCP socket.
///
/// Server-to-client frames are never masked per RFC 6455. Writes the
/// FIN bit set, the given opcode, the payload length (7-bit or 16-bit
/// extended), and the payload bytes.
///
/// # Errors
///
/// Returns [`WsError::Io`] if any TCP write fails.
#[cfg(feature = "tcp")]
pub async fn write_frame(
    socket: &mut TcpSocket<'_>,
    opcode: u8,
    payload: &[u8],
) -> Result<(), WsError> {
    // First byte: FIN=1 + opcode
    let b0 = 0x80 | (opcode & 0x0F);

    if payload.len() <= 125 {
        let header = [b0, payload.len() as u8];
        write_all_to_socket(socket, &header).await?;
    } else {
        // 16-bit extended length (payload.len() fits in u16 for our buffers)
        let len_bytes = (payload.len() as u16).to_be_bytes();
        let header = [b0, 126, len_bytes[0], len_bytes[1]];
        write_all_to_socket(socket, &header).await?;
    }

    if !payload.is_empty() {
        write_all_to_socket(socket, payload).await?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_sha1_digest() {
        // RFC 6455 test vector: SHA-1 of "dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-5AB5D3F3ABDA"
        let hash: [u8; 20] = [
            0xb3, 0x7a, 0x4f, 0x2c, 0xc0, 0x62, 0x4f, 0x16, 0x90, 0xf6, 0x46, 0x06, 0xcf, 0x38,
            0x59, 0x45, 0xb2, 0xbe, 0xc4, 0xea,
        ];
        let mut output = [0u8; 28];
        let len = base64_encode(&hash, &mut output);
        assert_eq!(len, 28);
        assert_eq!(&output[..len], b"s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_base64_encode_empty() {
        let mut output = [0u8; 4];
        let len = base64_encode(&[], &mut output);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_parse_upgrade_key_found() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";
        let key = parse_upgrade_key(request);
        assert!(key.is_some());
        assert_eq!(&key.unwrap(), b"dGhlIHNhbXBsZSBub25jZQ==");
    }

    #[test]
    fn test_parse_upgrade_key_missing() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(parse_upgrade_key(request).is_none());
    }

    #[test]
    fn test_parse_upgrade_key_case_insensitive() {
        let request = b"GET / HTTP/1.1\r\nsec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";
        let key = parse_upgrade_key(request);
        assert!(key.is_some());
        assert_eq!(&key.unwrap(), b"dGhlIHNhbXBsZSBub25jZQ==");
    }

    #[test]
    fn test_parse_request_path_get() {
        let request = b"GET /foo HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_request_path(request), Some("/foo"));
    }

    #[test]
    fn test_parse_request_path_root() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_request_path(request), Some("/"));
    }

    #[test]
    fn test_build_upgrade_response_format() {
        let key = *b"dGhlIHNhbXBsZSBub25jZQ==";
        let mut buf = [0u8; 256];
        let len = build_upgrade_response(&key, &mut buf);
        let response = core::str::from_utf8(&buf[..len]).expect("valid utf-8");
        assert!(response.starts_with("HTTP/1.1 101"));
        assert!(response.contains("Sec-WebSocket-Accept:"));
        assert!(response.ends_with("\r\n\r\n"));
    }
}
