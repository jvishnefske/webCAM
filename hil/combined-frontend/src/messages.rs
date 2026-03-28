//! CBOR message types for the HIL WebSocket protocol.
//!
//! All messages are CBOR maps with integer keys. Key 0 is always the message
//! tag (u8) that discriminates the variant. Request types flow from the browser
//! to the Pico, and response types flow from the Pico to the browser.
//!
//! # Wire Format
//!
//! Each message is a CBOR map where:
//! - Key 0: tag byte identifying the message type
//! - Keys 1..N: payload fields specific to each message type

/// Request messages sent from the browser to the Pico.
#[derive(Debug, Clone)]
pub enum Request {
    /// Read bytes from an I2C device register.
    ///
    /// Fields: bus index, device address, register address, byte count.
    I2cRead {
        /// I2C bus index (0-9).
        bus: u8,
        /// 7-bit I2C device address.
        addr: u8,
        /// Register address to read from.
        reg: u8,
        /// Number of bytes to read.
        len: u8,
    },
    /// Write bytes to an I2C device.
    ///
    /// Fields: bus index, device address, data payload.
    I2cWrite {
        /// I2C bus index (0-9).
        bus: u8,
        /// 7-bit I2C device address.
        addr: u8,
        /// Data bytes to write (first byte is typically register address).
        data: Vec<u8>,
    },
    /// Request a list of all buses and their devices.
    ListBuses,
    /// Request telemetry readings from all sensors.
    ReadAllTelemetry,
    /// Reboot the Pico into BOOTSEL mode for firmware update.
    RebootBootsel,
    /// Begin a firmware update session.
    ///
    /// Fields: total firmware size, expected CRC32.
    FwBegin {
        /// Total firmware image size in bytes.
        total_size: u32,
        /// CRC32 of the complete firmware image.
        crc32: u32,
    },
    /// Send a firmware data chunk.
    ///
    /// Fields: byte offset in the firmware image, data bytes.
    FwChunk {
        /// Byte offset of this chunk in the firmware image.
        offset: u32,
        /// Firmware data bytes for this chunk.
        data: Vec<u8>,
    },
    /// Finish a firmware update and verify CRC32.
    FwFinish {
        /// CRC32 of the complete firmware image for verification.
        crc32: u32,
    },
    /// Mark the current firmware as successfully booted.
    FwMarkBooted,
}

/// Response messages received from the Pico.
#[derive(Debug, Clone)]
pub enum Response {
    /// Data read from an I2C device.
    I2cData {
        /// The bytes read from the device.
        data: Vec<u8>,
    },
    /// Acknowledgement that an I2C write succeeded.
    WriteOk,
    /// List of all I2C buses and their devices.
    BusList {
        /// Each entry describes one bus and its attached devices.
        buses: Vec<BusEntry>,
    },
    /// Aggregated telemetry data from all sensors.
    Telemetry {
        /// Temperature readings in raw sensor units.
        temps: Vec<i32>,
        /// Power readings in raw sensor units.
        power: Vec<i32>,
        /// Fan RPM readings.
        fans: Vec<i32>,
    },
    /// Error message from the Pico.
    Error {
        /// Human-readable error description.
        message: String,
    },
    /// Firmware update ready to receive chunks.
    FwReady {
        /// Maximum chunk size in bytes.
        max_chunk: u16,
    },
    /// Acknowledgement of a firmware chunk write.
    FwChunkAck {
        /// Next expected byte offset.
        next_offset: u32,
    },
    /// Acknowledgement that firmware update is complete.
    FwFinishAck,
    /// Acknowledgement that firmware was marked as booted.
    FwMarkBootedAck,
}

/// Description of a single I2C bus and its attached devices.
#[derive(Debug, Clone)]
pub struct BusEntry {
    /// Bus index (0-9).
    pub bus_idx: u8,
    /// Devices discovered on this bus.
    pub devices: Vec<DeviceEntry>,
}

/// Description of a single I2C device on a bus.
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    /// 7-bit I2C address of the device.
    pub addr: u8,
    /// Human-readable device name.
    pub name: String,
}

/// Errors that can occur when decoding a CBOR response.
#[derive(Debug, Clone)]
pub enum DecodeError {
    /// The CBOR data is malformed or does not match the expected schema.
    Cbor(String),
    /// The message tag is not recognized.
    UnknownTag(u32),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::Cbor(msg) => write!(f, "CBOR decode error: {msg}"),
            DecodeError::UnknownTag(tag) => write!(f, "Unknown message tag: {tag}"),
        }
    }
}

impl From<minicbor::decode::Error> for DecodeError {
    fn from(e: minicbor::decode::Error) -> Self {
        DecodeError::Cbor(format!("{e}"))
    }
}

/// Encode a [`Request`] into a CBOR byte vector for transmission over WebSocket.
///
/// The encoded format is a CBOR map where key 0 holds the message tag.
pub fn encode_request(req: &Request) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut enc = minicbor::Encoder::new(&mut buf);
    match req {
        Request::I2cRead {
            bus,
            addr,
            reg,
            len,
        } => {
            enc.map(5)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(1)
                .expect("tag")
                .u32(1)
                .expect("k1")
                .u32(u32::from(*bus))
                .expect("bus")
                .u32(2)
                .expect("k2")
                .u32(u32::from(*addr))
                .expect("addr")
                .u32(3)
                .expect("k3")
                .u32(u32::from(*reg))
                .expect("reg")
                .u32(4)
                .expect("k4")
                .u32(u32::from(*len))
                .expect("len");
        }
        Request::I2cWrite { bus, addr, data } => {
            enc.map(3)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(2)
                .expect("tag")
                .u32(1)
                .expect("k1")
                .u32(u32::from(*bus))
                .expect("bus")
                .u32(2)
                .expect("k2")
                .u32(u32::from(*addr))
                .expect("addr")
                .u32(3)
                .expect("k3")
                .bytes(data)
                .expect("data");
        }
        Request::ListBuses => {
            enc.map(1)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(3)
                .expect("tag");
        }
        Request::ReadAllTelemetry => {
            enc.map(1)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(4)
                .expect("tag");
        }
        Request::RebootBootsel => {
            enc.map(1)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(12)
                .expect("tag");
        }
        Request::FwBegin { total_size, crc32 } => {
            enc.map(3)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(20)
                .expect("tag")
                .u32(1)
                .expect("k1")
                .u32(*total_size)
                .expect("total_size")
                .u32(2)
                .expect("k2")
                .u32(*crc32)
                .expect("crc32");
        }
        Request::FwChunk { offset, data } => {
            enc.map(3)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(21)
                .expect("tag")
                .u32(1)
                .expect("k1")
                .u32(*offset)
                .expect("offset")
                .u32(2)
                .expect("k2")
                .bytes(data)
                .expect("data");
        }
        Request::FwFinish { crc32 } => {
            enc.map(2)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(22)
                .expect("tag")
                .u32(1)
                .expect("k1")
                .u32(*crc32)
                .expect("crc32");
        }
        Request::FwMarkBooted => {
            enc.map(1)
                .expect("map")
                .u32(0)
                .expect("k0")
                .u32(23)
                .expect("tag");
        }
    }
    buf
}

/// Decode a CBOR response from the Pico into a [`Response`] variant.
///
/// Returns a [`DecodeError`] if the data is malformed or the tag is unknown.
pub fn decode_response(data: &[u8]) -> Result<Response, DecodeError> {
    let mut dec = minicbor::Decoder::new(data);
    let _map_len = dec.map()?;

    // Read tag from key 0
    let _key0 = dec.u32()?;
    let tag = dec.u32()?;

    match tag {
        // I2cData
        1 => {
            let _key1 = dec.u32()?;
            let bytes = dec.bytes()?;
            Ok(Response::I2cData {
                data: bytes.to_vec(),
            })
        }
        // WriteOk
        2 => Ok(Response::WriteOk),
        // BusList
        3 => {
            let _key1 = dec.u32()?;
            let bus_count = dec.array()?;
            let n = bus_count.unwrap_or(0) as usize;
            let mut buses = Vec::with_capacity(n);
            for _ in 0..n {
                let _bus_map = dec.map()?;
                // key 0: bus_idx
                let _k0 = dec.u32()?;
                let bus_idx = dec.u8()?;
                // key 1: devices array
                let _k1 = dec.u32()?;
                let dev_count = dec.array()?;
                let dn = dev_count.unwrap_or(0) as usize;
                let mut devices = Vec::with_capacity(dn);
                for _ in 0..dn {
                    let _dev_map = dec.map()?;
                    let _dk0 = dec.u32()?;
                    let addr = dec.u8()?;
                    let _dk1 = dec.u32()?;
                    let name = dec.str()?.to_string();
                    devices.push(DeviceEntry { addr, name });
                }
                buses.push(BusEntry { bus_idx, devices });
            }
            Ok(Response::BusList { buses })
        }
        // Telemetry
        4 => {
            let _key1 = dec.u32()?;
            let _inner_map = dec.map()?;

            // key 0: temps
            let _k0 = dec.u32()?;
            let temps = decode_i32_array(&mut dec)?;
            // key 1: power
            let _k1 = dec.u32()?;
            let power = decode_i32_array(&mut dec)?;
            // key 2: fans
            let _k2 = dec.u32()?;
            let fans = decode_i32_array(&mut dec)?;

            Ok(Response::Telemetry { temps, power, fans })
        }
        // FwReady: {0:20, 1:max_chunk(u16)}
        20 => {
            let _key1 = dec.u32()?;
            let max_chunk = dec.u16()?;
            Ok(Response::FwReady { max_chunk })
        }
        // FwChunkAck: {0:21, 1:next_offset(u32)}
        21 => {
            let _key1 = dec.u32()?;
            let next_offset = dec.u32()?;
            Ok(Response::FwChunkAck { next_offset })
        }
        // FwFinishAck: {0:22}
        22 => Ok(Response::FwFinishAck),
        // FwMarkBootedAck: {0:23}
        23 => Ok(Response::FwMarkBootedAck),
        // Error
        255 => {
            let _key1 = dec.u32()?;
            let message = dec.str()?.to_string();
            Ok(Response::Error { message })
        }
        other => Err(DecodeError::UnknownTag(other)),
    }
}

/// Decode a CBOR array of signed 32-bit integers.
fn decode_i32_array(dec: &mut minicbor::Decoder<'_>) -> Result<Vec<i32>, DecodeError> {
    let arr_len = dec.array()?;
    let n = arr_len.unwrap_or(0) as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(dec.i32()?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_list_buses() {
        let encoded = encode_request(&Request::ListBuses);
        // Verify it's valid CBOR with tag 3
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map_len = dec.map().expect("map");
        let key = dec.u32().expect("key");
        assert_eq!(key, 0);
        let tag = dec.u32().expect("tag");
        assert_eq!(tag, 3);
    }

    #[test]
    fn round_trip_i2c_read() {
        let req = Request::I2cRead {
            bus: 2,
            addr: 0x48,
            reg: 0x00,
            len: 2,
        };
        let encoded = encode_request(&req);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map_len = dec.map().expect("map");
        // key 0 -> tag 1
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 1);
        // key 1 -> bus 2
        assert_eq!(dec.u32().expect("k"), 1);
        assert_eq!(dec.u32().expect("v"), 2);
        // key 2 -> addr 0x48
        assert_eq!(dec.u32().expect("k"), 2);
        assert_eq!(dec.u32().expect("v"), 0x48);
        // key 3 -> reg 0
        assert_eq!(dec.u32().expect("k"), 3);
        assert_eq!(dec.u32().expect("v"), 0);
        // key 4 -> len 2
        assert_eq!(dec.u32().expect("k"), 4);
        assert_eq!(dec.u32().expect("v"), 2);
    }

    #[test]
    fn decode_write_ok_response() {
        // Manually encode a WriteOk response: {0: 2}
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).expect("map");
        enc.u32(0).expect("k").u32(2).expect("v");
        let resp = decode_response(&buf).expect("decode");
        assert!(matches!(resp, Response::WriteOk));
    }

    #[test]
    fn decode_error_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(255).expect("v");
        enc.u32(1).expect("k").str("bus fault").expect("v");
        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::Error { message } => assert_eq!(message, "bus fault"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_i2c_data_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(1).expect("v");
        enc.u32(1).expect("k").bytes(&[0xAB, 0xCD]).expect("v");
        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::I2cData { data } => assert_eq!(data, vec![0xAB, 0xCD]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_unknown_tag_returns_error() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).expect("map");
        enc.u32(0).expect("k").u32(99).expect("v");
        let result = decode_response(&buf);
        assert!(matches!(result, Err(DecodeError::UnknownTag(99))));
    }

    #[test]
    fn encode_reboot_bootsel() {
        let encoded = encode_request(&Request::RebootBootsel);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map_len = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 12);
    }

    #[test]
    fn round_trip_i2c_write() {
        let req = Request::I2cWrite {
            bus: 1,
            addr: 0x50,
            data: vec![0x00, 0xAB, 0xCD],
        };
        let encoded = encode_request(&req);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map_len = dec.map().expect("map");
        // key 0 -> tag 2
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 2);
        // key 1 -> bus 1
        assert_eq!(dec.u32().expect("k"), 1);
        assert_eq!(dec.u32().expect("v"), 1);
        // key 2 -> addr 0x50
        assert_eq!(dec.u32().expect("k"), 2);
        assert_eq!(dec.u32().expect("v"), 0x50);
        // key 3 -> data bytes
        assert_eq!(dec.u32().expect("k"), 3);
        assert_eq!(dec.bytes().expect("v"), &[0x00, 0xAB, 0xCD]);
    }

    #[test]
    fn round_trip_read_all_telemetry() {
        let encoded = encode_request(&Request::ReadAllTelemetry);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map_len = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 4);
    }

    #[test]
    fn decode_bus_list_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(3).expect("tag");
        enc.u32(1).expect("k");
        enc.array(2).expect("arr");
        // Bus 0 with 2 devices
        enc.map(2).expect("bus_map");
        enc.u32(0).expect("k").u8(0).expect("v");
        enc.u32(1).expect("k");
        enc.array(2).expect("dev_arr");
        enc.map(2).expect("dev_map");
        enc.u32(0).expect("k").u8(0x48).expect("v");
        enc.u32(1).expect("k").str("TMP117").expect("v");
        enc.map(2).expect("dev_map");
        enc.u32(0).expect("k").u8(0x40).expect("v");
        enc.u32(1).expect("k").str("INA230").expect("v");
        // Bus 1 with 1 device
        enc.map(2).expect("bus_map");
        enc.u32(0).expect("k").u8(1).expect("v");
        enc.u32(1).expect("k");
        enc.array(1).expect("dev_arr");
        enc.map(2).expect("dev_map");
        enc.u32(0).expect("k").u8(0x50).expect("v");
        enc.u32(1).expect("k").str("EEPROM").expect("v");

        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::BusList { buses } => {
                assert_eq!(buses.len(), 2);
                assert_eq!(buses[0].bus_idx, 0);
                assert_eq!(buses[0].devices.len(), 2);
                assert_eq!(buses[0].devices[0].addr, 0x48);
                assert_eq!(buses[0].devices[0].name, "TMP117");
                assert_eq!(buses[0].devices[1].addr, 0x40);
                assert_eq!(buses[0].devices[1].name, "INA230");
                assert_eq!(buses[1].bus_idx, 1);
                assert_eq!(buses[1].devices.len(), 1);
                assert_eq!(buses[1].devices[0].addr, 0x50);
                assert_eq!(buses[1].devices[0].name, "EEPROM");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_telemetry_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(4).expect("tag");
        enc.u32(1).expect("k");
        enc.map(3).expect("inner_map");
        enc.u32(0).expect("k");
        enc.array(2)
            .expect("arr")
            .i32(2500)
            .expect("v")
            .i32(2600)
            .expect("v");
        enc.u32(1).expect("k");
        enc.array(1).expect("arr").i32(1200).expect("v");
        enc.u32(2).expect("k");
        enc.array(3)
            .expect("arr")
            .i32(1500)
            .expect("v")
            .i32(1600)
            .expect("v")
            .i32(1700)
            .expect("v");

        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::Telemetry { temps, power, fans } => {
                assert_eq!(temps, vec![2500, 2600]);
                assert_eq!(power, vec![1200]);
                assert_eq!(fans, vec![1500, 1600, 1700]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_empty_bus_list() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(3).expect("tag");
        enc.u32(1).expect("k");
        enc.array(0).expect("arr");

        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::BusList { buses } => assert!(buses.is_empty()),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_empty_i2c_data() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(1).expect("tag");
        enc.u32(1).expect("k").bytes(&[]).expect("v");

        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::I2cData { data } => assert!(data.is_empty()),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_truncated_input() {
        let buf = vec![0xA2]; // map(2) header with no content
        assert!(decode_response(&buf).is_err());
    }

    #[test]
    fn decode_error_display() {
        let err = DecodeError::Cbor("bad data".to_string());
        assert_eq!(format!("{err}"), "CBOR decode error: bad data");

        let err = DecodeError::UnknownTag(42);
        assert_eq!(format!("{err}"), "Unknown message tag: 42");
    }

    #[test]
    fn encode_fw_begin() {
        let req = Request::FwBegin {
            total_size: 65536,
            crc32: 0xDEADBEEF,
        };
        let encoded = encode_request(&req);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 20);
        assert_eq!(dec.u32().expect("k"), 1);
        assert_eq!(dec.u32().expect("v"), 65536);
        assert_eq!(dec.u32().expect("k"), 2);
        assert_eq!(dec.u32().expect("v"), 0xDEADBEEF);
    }

    #[test]
    fn encode_fw_chunk() {
        let req = Request::FwChunk {
            offset: 1024,
            data: vec![0xAA; 64],
        };
        let encoded = encode_request(&req);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 21);
        assert_eq!(dec.u32().expect("k"), 1);
        assert_eq!(dec.u32().expect("v"), 1024);
        assert_eq!(dec.u32().expect("k"), 2);
        let bytes = dec.bytes().expect("v");
        assert_eq!(bytes.len(), 64);
        assert!(bytes.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn encode_fw_finish() {
        let req = Request::FwFinish { crc32: 0xCAFEBABE };
        let encoded = encode_request(&req);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 22);
        assert_eq!(dec.u32().expect("k"), 1);
        assert_eq!(dec.u32().expect("v"), 0xCAFEBABE);
    }

    #[test]
    fn encode_fw_mark_booted() {
        let encoded = encode_request(&Request::FwMarkBooted);
        let mut dec = minicbor::Decoder::new(&encoded);
        let _map = dec.map().expect("map");
        assert_eq!(dec.u32().expect("k"), 0);
        assert_eq!(dec.u32().expect("v"), 23);
    }

    #[test]
    fn decode_fw_ready_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(20).expect("v");
        enc.u32(1).expect("k").u16(1024).expect("v");
        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::FwReady { max_chunk } => assert_eq!(max_chunk, 1024),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_fw_chunk_ack_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).expect("map");
        enc.u32(0).expect("k").u32(21).expect("v");
        enc.u32(1).expect("k").u32(4096).expect("v");
        let resp = decode_response(&buf).expect("decode");
        match resp {
            Response::FwChunkAck { next_offset } => assert_eq!(next_offset, 4096),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_fw_finish_ack_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).expect("map");
        enc.u32(0).expect("k").u32(22).expect("v");
        let resp = decode_response(&buf).expect("decode");
        assert!(matches!(resp, Response::FwFinishAck));
    }

    #[test]
    fn decode_fw_mark_booted_ack_response() {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).expect("map");
        enc.u32(0).expect("k").u32(23).expect("v");
        let resp = decode_response(&buf).expect("decode");
        assert!(matches!(resp, Response::FwMarkBootedAck));
    }
}
