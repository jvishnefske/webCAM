//! RP2040 SWD pin configuration and DAP processor.
//!
//! Provides a stub CMSIS-DAP processor that responds to DAP_Info
//! queries over USB bulk endpoints. The actual SWD bitbang will be
//! connected when a compatible CMSIS-DAP backend is integrated.

use dap_dispatch::protocol::DapProcessor;

// Pin assignments are reserved for future SWD bitbang integration.
#[allow(dead_code)]
/// SWCLK pin assignment (compile-time constant).
/// GP2 on Pico - adjust for your board layout.
pub const SWCLK_PIN: u8 = 2;

#[allow(dead_code)]
/// SWDIO pin assignment (compile-time constant).
/// GP3 on Pico - adjust for your board layout.
pub const SWDIO_PIN: u8 = 3;

#[allow(dead_code)]
/// nRESET pin assignment (compile-time constant).
/// GP4 on Pico - adjust for your board layout.
pub const NRESET_PIN: u8 = 4;

/// DAP_Info command identifier.
const DAP_CMD_INFO: u8 = 0x00;

/// DAP_Info subcommand: vendor name.
const DAP_INFO_VENDOR: u8 = 0x01;

/// DAP_Info subcommand: product name.
const DAP_INFO_PRODUCT: u8 = 0x02;

/// DAP_Info subcommand: maximum packet count.
const DAP_INFO_PACKET_COUNT: u8 = 0xFE;

/// DAP_Info subcommand: maximum packet size.
const DAP_INFO_PACKET_SIZE: u8 = 0xFF;

/// Stub CMSIS-DAP processor for RP2040.
///
/// Responds to `DAP_Info` queries with device metadata (vendor name,
/// product name, packet size, packet count). All other commands receive
/// a `DAP_ERROR` (0xFF) response. When a real SWD bitbang backend is
/// integrated, the remaining commands will be forwarded to it.
pub struct PicoDapProcessor {
    _private: (),
}

impl PicoDapProcessor {
    /// Creates a new stub DAP processor.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Writes a length-prefixed string info response.
    ///
    /// Format: `[0x00, len, string_bytes...]`
    /// Returns the total number of bytes written.
    fn write_string_info(s: &[u8], response: &mut [u8]) -> usize {
        let len = s.len();
        if response.len() < 2 + len {
            return 0;
        }
        response[0] = DAP_CMD_INFO;
        // The length byte includes the trailing NUL expected by some hosts,
        // but CMSIS-DAP spec says length of the information data.
        // We send the raw string without NUL; length = string length.
        response[1] = len as u8;
        response[2..2 + len].copy_from_slice(s);
        2 + len
    }
}

impl DapProcessor for PicoDapProcessor {
    /// Processes a single CMSIS-DAP command.
    ///
    /// Handles `DAP_Info` (0x00) for vendor, product, packet size, and
    /// packet count queries. Returns `[cmd, 0xFF]` for unrecognized
    /// commands.
    fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
        if request.is_empty() || response.len() < 2 {
            return 0;
        }

        let cmd = request[0];

        if cmd == DAP_CMD_INFO {
            // DAP_Info requires at least a subcommand byte
            if request.len() < 2 {
                response[0] = DAP_CMD_INFO;
                response[1] = 0;
                return 2;
            }

            let sub = request[1];
            match sub {
                DAP_INFO_VENDOR => Self::write_string_info(b"i2c-hil", response),
                DAP_INFO_PRODUCT => Self::write_string_info(b"Pico DAP", response),
                DAP_INFO_PACKET_SIZE => {
                    // Packet size as u16 LE: 512 = 0x0200
                    if response.len() < 4 {
                        return 0;
                    }
                    response[0] = DAP_CMD_INFO;
                    response[1] = 2; // 2 bytes of data
                    response[2] = 0x00; // low byte of 512
                    response[3] = 0x02; // high byte of 512
                    4
                }
                DAP_INFO_PACKET_COUNT => {
                    if response.len() < 3 {
                        return 0;
                    }
                    response[0] = DAP_CMD_INFO;
                    response[1] = 1; // 1 byte of data
                    response[2] = 1; // 1 packet at a time
                    3
                }
                _ => {
                    // Unknown info subcommand: no data
                    response[0] = DAP_CMD_INFO;
                    response[1] = 0;
                    2
                }
            }
        } else {
            // Unknown command: DAP_ERROR
            response[0] = cmd;
            response[1] = 0xFF;
            2
        }
    }
}
