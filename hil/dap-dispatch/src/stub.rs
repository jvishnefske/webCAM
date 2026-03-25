//! Stub CMSIS-DAP processor for boards without SWD hardware.
//!
//! [`StubDapProcessor`] handles `DAP_Info` (command `0x00`) with basic
//! identification data and returns `DAP_ERROR` (`0xFF`) for all other
//! command IDs. The product name is configurable via the constructor.

use crate::protocol::DapProcessor;

/// Stub CMSIS-DAP processor for development and testing.
///
/// Handles `DAP_Info` (command `0x00`) with basic identification data
/// and returns `DAP_ERROR` (`0xFF`) for all other command IDs. The
/// product name is configurable at construction time.
pub struct StubDapProcessor<'a> {
    product_name: &'a str,
}

impl<'a> StubDapProcessor<'a> {
    /// Creates a new stub DAP processor with the given product name.
    pub fn new(product_name: &'a str) -> Self {
        Self { product_name }
    }
}

/// Writes a length-prefixed string into the response buffer.
///
/// Format: `[command_id, length, string_bytes...]`
/// Returns the total number of bytes written (2 + string length).
fn write_info_string(response: &mut [u8], s: &[u8]) -> usize {
    let len = s.len();
    response[0] = 0x00; // DAP_Info response ID
    response[1] = len as u8;
    response[2..2 + len].copy_from_slice(s);
    2 + len
}

impl DapProcessor for StubDapProcessor<'_> {
    /// Processes a single CMSIS-DAP command.
    ///
    /// Supported commands:
    /// - `0x00` (`DAP_Info`): returns identification based on subcommand:
    ///   - `0x01` (Vendor): `"i2c-hil"`
    ///   - `0x02` (Product): the configured product name
    ///   - `0xFE` (Packet Count): `1`
    ///   - `0xFF` (Packet Size): `512` (u16 LE)
    ///   - Other info IDs: length `0`
    /// - All other commands: `[command_id, 0xFF]` (`DAP_ERROR`)
    fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
        if request.is_empty() {
            return 0;
        }

        let command_id = request[0];

        match command_id {
            // DAP_Info
            0x00 => {
                if request.len() < 2 {
                    response[0] = 0x00;
                    response[1] = 0;
                    return 2;
                }

                let info_id = request[1];
                match info_id {
                    // Vendor Name
                    0x01 => write_info_string(response, b"i2c-hil"),
                    // Product Name
                    0x02 => write_info_string(response, self.product_name.as_bytes()),
                    // Packet Count
                    0xFE => {
                        response[0] = 0x00;
                        response[1] = 1; // 1 byte follows
                        response[2] = 1; // 1 packet
                        3
                    }
                    // Packet Size
                    0xFF => {
                        response[0] = 0x00;
                        response[1] = 2; // 2 bytes follow (u16 LE)
                        response[2] = 0x00; // 512 & 0xFF
                        response[3] = 0x02; // 512 >> 8
                        4
                    }
                    // Unknown info ID — respond with length 0
                    _ => {
                        response[0] = 0x00;
                        response[1] = 0;
                        2
                    }
                }
            }
            // Unknown command — DAP_ERROR
            _ => {
                response[0] = command_id;
                response[1] = 0xFF;
                2
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_info() {
        let mut dap = StubDapProcessor::new("Test DAP");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0x01], &mut resp);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 7); // "i2c-hil".len()
        assert_eq!(&resp[2..2 + 7], b"i2c-hil");
        assert_eq!(n, 9);
    }

    #[test]
    fn product_info() {
        let mut dap = StubDapProcessor::new("Pi Zero DAP");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0x02], &mut resp);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 11); // "Pi Zero DAP".len()
        assert_eq!(&resp[2..2 + 11], b"Pi Zero DAP");
        assert_eq!(n, 13);
    }

    #[test]
    fn custom_product_info() {
        let mut dap = StubDapProcessor::new("Custom Board");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0x02], &mut resp);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 12); // "Custom Board".len()
        assert_eq!(&resp[2..2 + 12], b"Custom Board");
        assert_eq!(n, 14);
    }

    #[test]
    fn packet_count_info() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0xFE], &mut resp);
        assert_eq!(n, 3);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 1);
        assert_eq!(resp[2], 1);
    }

    #[test]
    fn packet_size_info() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0xFF], &mut resp);
        assert_eq!(n, 4);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 2);
        let size = u16::from_le_bytes([resp[2], resp[3]]);
        assert_eq!(size, 512);
    }

    #[test]
    fn unknown_info_id_returns_zero_length() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00, 0x50], &mut resp);
        assert_eq!(n, 2);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 0);
    }

    #[test]
    fn dap_info_missing_subcommand() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x00], &mut resp);
        assert_eq!(n, 2);
        assert_eq!(resp[0], 0x00);
        assert_eq!(resp[1], 0);
    }

    #[test]
    fn unknown_command_returns_error() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[0x10], &mut resp);
        assert_eq!(n, 2);
        assert_eq!(resp[0], 0x10);
        assert_eq!(resp[1], 0xFF);
    }

    #[test]
    fn empty_request_returns_zero() {
        let mut dap = StubDapProcessor::new("Test");
        let mut resp = [0u8; 64];
        let n = dap.process_command(&[], &mut resp);
        assert_eq!(n, 0);
    }
}
