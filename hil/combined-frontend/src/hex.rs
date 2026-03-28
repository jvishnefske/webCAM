//! Hexadecimal string parsing utilities.
//!
//! Provides functions to parse hex strings (with optional `0x` prefix) into
//! byte values, used by the I2C console for address and data input.

/// Parse a hexadecimal string (with optional "0x" prefix) into a `u8`.
///
/// Returns `None` if the string is not a valid hex byte.
pub fn parse_hex_u8(s: &str) -> Option<u8> {
    let s = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    u8::from_str_radix(s, 16).ok()
}

/// Parse a hex data string like "AB CD 01" or "ABCD01" into bytes.
///
/// Accepts space-separated hex bytes or a continuous hex string.
/// Returns `None` if any byte is invalid or the input is empty.
pub fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try space-separated first
    if s.contains(' ') {
        let result: Option<Vec<u8>> = s.split_whitespace().map(parse_hex_u8).collect();
        return result;
    }

    // Continuous hex string (must be even length)
    let s = s.strip_prefix("0x").unwrap_or(s);
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let result: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_u8_valid_byte() {
        assert_eq!(parse_hex_u8("48"), Some(0x48));
    }

    #[test]
    fn parse_hex_u8_max_value() {
        assert_eq!(parse_hex_u8("FF"), Some(0xFF));
    }

    #[test]
    fn parse_hex_u8_zero() {
        assert_eq!(parse_hex_u8("00"), Some(0x00));
    }

    #[test]
    fn parse_hex_u8_with_prefix() {
        assert_eq!(parse_hex_u8("0x48"), Some(0x48));
    }

    #[test]
    fn parse_hex_u8_empty_string() {
        assert_eq!(parse_hex_u8(""), None);
    }

    #[test]
    fn parse_hex_u8_invalid_chars() {
        assert_eq!(parse_hex_u8("GG"), None);
    }

    #[test]
    fn parse_hex_u8_prefix_only() {
        assert_eq!(parse_hex_u8("0x"), None);
    }

    #[test]
    fn parse_hex_u8_lowercase() {
        assert_eq!(parse_hex_u8("ab"), Some(0xAB));
    }

    #[test]
    fn parse_hex_bytes_space_separated() {
        assert_eq!(parse_hex_bytes("AB CD 01"), Some(vec![0xAB, 0xCD, 0x01]));
    }

    #[test]
    fn parse_hex_bytes_continuous() {
        assert_eq!(parse_hex_bytes("ABCD01"), Some(vec![0xAB, 0xCD, 0x01]));
    }

    #[test]
    fn parse_hex_bytes_odd_length_returns_none() {
        assert_eq!(parse_hex_bytes("ABC"), None);
    }

    #[test]
    fn parse_hex_bytes_empty_returns_none() {
        assert_eq!(parse_hex_bytes(""), None);
    }

    #[test]
    fn parse_hex_bytes_with_0x_prefix() {
        assert_eq!(parse_hex_bytes("0xABCD"), Some(vec![0xAB, 0xCD]));
    }

    #[test]
    fn parse_hex_bytes_single_byte() {
        assert_eq!(parse_hex_bytes("FF"), Some(vec![0xFF]));
    }

    #[test]
    fn parse_hex_bytes_whitespace_only() {
        assert_eq!(parse_hex_bytes("   "), None);
    }
}
