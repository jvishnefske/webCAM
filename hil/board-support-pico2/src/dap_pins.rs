//! RP2350 SWD pin configuration and DAP processor (stub).

use dap_dispatch::protocol::DapProcessor;

const DAP_CMD_INFO: u8 = 0x00;
const DAP_INFO_VENDOR: u8 = 0x01;
const DAP_INFO_PRODUCT: u8 = 0x02;
const DAP_INFO_PACKET_COUNT: u8 = 0xFE;
const DAP_INFO_PACKET_SIZE: u8 = 0xFF;

pub struct PicoDapProcessor {
    _private: (),
}

impl PicoDapProcessor {
    pub fn new() -> Self {
        Self { _private: () }
    }

    fn write_string_info(s: &[u8], response: &mut [u8]) -> usize {
        let len = s.len();
        if response.len() < 2 + len {
            return 0;
        }
        response[0] = DAP_CMD_INFO;
        response[1] = len as u8;
        response[2..2 + len].copy_from_slice(s);
        2 + len
    }
}

impl DapProcessor for PicoDapProcessor {
    fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
        if request.is_empty() || response.len() < 2 {
            return 0;
        }
        let cmd = request[0];
        if cmd == DAP_CMD_INFO {
            if request.len() < 2 {
                response[0] = DAP_CMD_INFO;
                response[1] = 0;
                return 2;
            }
            match request[1] {
                DAP_INFO_VENDOR => Self::write_string_info(b"i2c-hil", response),
                DAP_INFO_PRODUCT => Self::write_string_info(b"Pico2 DAP", response),
                DAP_INFO_PACKET_SIZE => {
                    if response.len() < 4 {
                        return 0;
                    }
                    response[0] = DAP_CMD_INFO;
                    response[1] = 2;
                    response[2] = 0x00;
                    response[3] = 0x02;
                    4
                }
                DAP_INFO_PACKET_COUNT => {
                    if response.len() < 3 {
                        return 0;
                    }
                    response[0] = DAP_CMD_INFO;
                    response[1] = 1;
                    response[2] = 1;
                    3
                }
                _ => {
                    response[0] = DAP_CMD_INFO;
                    response[1] = 0;
                    2
                }
            }
        } else {
            response[0] = cmd;
            response[1] = 0xFF;
            2
        }
    }
}
