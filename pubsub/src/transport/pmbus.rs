//! PMBus/SMBus transport over I2C.
//!
//! PMBus is built on SMBus, which is built on I2C. This transport uses
//! the `embedded_hal::i2c::I2c` trait from embedded-hal 1.0.
//!
//! # SMBus Block Transfer
//!
//! - **Block Write**: `[command_code, byte_count, data...]`
//! - **Block Read**: controller sends `[command_code]`, device responds
//!   with `[byte_count, data...]`
//!
//! Maximum block payload per SMBus spec: 32 bytes.
//!
//! # Fragmentation
//!
//! First byte of each block payload is a fragmentation header:
//! `[more:1][seq:7]`. The remaining 31 bytes carry frame data.
//!
//! The topic hash is mapped to a PMBus command code (lower 8 bits).

use embedded_hal::i2c::I2c;

use crate::frame::{Frame, FRAME_HEADER_SIZE, MAX_FRAME_PAYLOAD};
use crate::transport::{Transport, TransportError};
use heapless::Vec;

/// Maximum bytes in a single SMBus block transfer.
const SMBUS_BLOCK_MAX: usize = 32;

/// Payload bytes per fragment (32 minus 1 fragmentation header).
const PMBUS_FRAGMENT_PAYLOAD: usize = SMBUS_BLOCK_MAX - 1;

/// PMBus/SMBus transport over I2C.
///
/// Fragments pub/sub [`Frame`]s into SMBus block transfers and
/// reassembles them on the receive side.
pub struct PmbusTransport<I: I2c> {
    i2c: I,
    /// 7-bit I2C address of the target device.
    device_addr: u8,
    reassembly_buf: Vec<u8, 128>,
    expected_seq: u8,
}

impl<I: I2c> PmbusTransport<I> {
    /// Create a new PMBus transport.
    ///
    /// `device_addr` is the 7-bit I2C address of the remote device.
    pub fn new(i2c: I, device_addr: u8) -> Self {
        Self {
            i2c,
            device_addr,
            reassembly_buf: Vec::new(),
            expected_seq: 0,
        }
    }

    /// Consume the transport and return the inner I2C bus.
    pub fn into_inner(self) -> I {
        self.i2c
    }

    /// Return the configured 7-bit device address.
    pub fn device_addr(&self) -> u8 {
        self.device_addr
    }

    /// Reset the reassembly state.
    fn reset_reassembly(&mut self) {
        self.reassembly_buf.clear();
        self.expected_seq = 0;
    }

    /// Perform an SMBus Block Write: `[command_code, byte_count, data...]`.
    fn block_write(&mut self, command: u8, data: &[u8]) -> Result<(), TransportError> {
        // Wire format: command byte, then byte count, then data bytes.
        // We write it as a single I2C write transaction.
        let dlen = data.len();
        if dlen > SMBUS_BLOCK_MAX {
            return Err(TransportError::FrameTooLarge);
        }

        // Build the write buffer: [command, byte_count, data...]
        let mut buf = [0u8; 2 + SMBUS_BLOCK_MAX];
        buf[0] = command;
        buf[1] = dlen as u8;
        buf[2..2 + dlen].copy_from_slice(data);

        self.i2c
            .write(self.device_addr, &buf[..2 + dlen])
            .map_err(|_| TransportError::SendFailed)
    }

    /// Perform an SMBus Block Read.
    ///
    /// Sends `[command]` then reads `[byte_count, data...]`.
    /// Returns the number of data bytes read into `out`.
    fn block_read(
        &mut self,
        command: u8,
        out: &mut [u8; SMBUS_BLOCK_MAX],
    ) -> Result<usize, TransportError> {
        // Write the command byte, then read the response.
        // Response format: [byte_count, data_0, data_1, ...]
        let mut resp = [0u8; 1 + SMBUS_BLOCK_MAX];

        self.i2c
            .write_read(self.device_addr, &[command], &mut resp)
            .map_err(|_| TransportError::RecvFailed)?;

        let byte_count = resp[0] as usize;
        if byte_count == 0 {
            return Ok(0);
        }
        let byte_count = byte_count.min(SMBUS_BLOCK_MAX);
        out[..byte_count].copy_from_slice(&resp[1..1 + byte_count]);

        Ok(byte_count)
    }
}

impl<I: I2c> Transport for PmbusTransport<I> {
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
        // Serialize the full pub/sub frame to bytes
        let mut frame_bytes = [0u8; FRAME_HEADER_SIZE + MAX_FRAME_PAYLOAD];
        let total = frame
            .to_bytes(&mut frame_bytes)
            .map_err(|_| TransportError::FrameTooLarge)?;

        // Command code: lower 8 bits of topic hash
        let command = (frame.topic.as_u32() & 0xFF) as u8;

        let mut offset = 0;
        let mut seq: u8 = 0;

        while offset < total {
            let remaining = total - offset;
            let chunk = remaining.min(PMBUS_FRAGMENT_PAYLOAD);
            let more = if offset + chunk < total {
                0x80u8
            } else {
                0x00u8
            };

            // Build block data: [frag_header, payload_chunk...]
            let mut block = [0u8; SMBUS_BLOCK_MAX];
            block[0] = more | (seq & 0x7F);
            block[1..1 + chunk].copy_from_slice(&frame_bytes[offset..offset + chunk]);

            self.block_write(command, &block[..1 + chunk])?;

            offset += chunk;
            seq += 1;
        }

        Ok(())
    }

    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
        // Use command 0x00 as the default read command.
        // In a real system the command code would be negotiated or polled.
        let mut block = [0u8; SMBUS_BLOCK_MAX];
        let n = self.block_read(0x00, &mut block)?;
        if n == 0 {
            return Ok(false);
        }

        let header = block[0];
        let more = (header & 0x80) != 0;
        let seq = header & 0x7F;

        // Check sequence continuity
        if seq != self.expected_seq {
            self.reset_reassembly();
            if seq != 0 {
                return Err(TransportError::BusError);
            }
        }

        // Append payload bytes (skip the fragmentation header byte)
        let payload_len = (n - 1).min(PMBUS_FRAGMENT_PAYLOAD);
        for i in 0..payload_len {
            self.reassembly_buf
                .push(block[1 + i])
                .map_err(|_| TransportError::FrameTooLarge)?;
        }
        self.expected_seq = seq + 1;

        if more {
            return Ok(false);
        }

        // Final fragment -- reassemble the full pub/sub frame
        let result = Frame::from_bytes(&self.reassembly_buf);
        self.reset_reassembly();

        match result {
            Ok(frame) => {
                *buf = frame;
                Ok(true)
            }
            Err(_) => Err(TransportError::RecvFailed),
        }
    }

    fn mtu(&self) -> usize {
        PMBUS_FRAGMENT_PAYLOAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::NodeAddr;
    use crate::topic::TopicId;
    use core::cell::RefCell;
    use heapless::Deque;

    /// Record of an I2C operation for the mock.
    #[derive(Debug, Clone)]
    enum I2cOp {
        Write {
            addr: u8,
            data: heapless::Vec<u8, 64>,
        },
        WriteRead {
            addr: u8,
            command: u8,
        },
    }

    /// Mock I2C bus backed by heapless queues.
    struct MockI2c {
        /// Log of operations performed.
        ops: RefCell<heapless::Vec<I2cOp, 64>>,
        /// Canned responses for write_read (block read) calls.
        /// Each entry is the full response buffer: [byte_count, data...].
        read_responses: RefCell<Deque<heapless::Vec<u8, 64>, 16>>,
    }

    #[derive(Debug)]
    struct MockI2cError;

    impl embedded_hal::i2c::Error for MockI2cError {
        fn kind(&self) -> embedded_hal::i2c::ErrorKind {
            embedded_hal::i2c::ErrorKind::Other
        }
    }

    impl embedded_hal::i2c::ErrorType for MockI2c {
        type Error = MockI2cError;
    }

    impl embedded_hal::i2c::ErrorType for &MockI2c {
        type Error = MockI2cError;
    }

    impl MockI2c {
        fn new() -> Self {
            Self {
                ops: RefCell::new(heapless::Vec::new()),
                read_responses: RefCell::new(Deque::new()),
            }
        }

        /// Enqueue a canned response for the next `write_read` call.
        /// The buffer should be `[byte_count, data_0, data_1, ...]`.
        fn enqueue_read_response(&self, resp: &[u8]) {
            let mut v = heapless::Vec::<u8, 64>::new();
            for &b in resp {
                let _ = v.push(b);
            }
            let _ = self.read_responses.borrow_mut().push_back(v);
        }

        /// Return the number of write operations logged.
        fn write_count(&self) -> usize {
            self.ops
                .borrow()
                .iter()
                .filter(|op| matches!(op, I2cOp::Write { .. }))
                .count()
        }
    }

    impl I2c for &MockI2c {
        fn transaction(
            &mut self,
            _address: u8,
            _operations: &mut [embedded_hal::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            // Not used directly; write/read/write_read are provided via default impls
            // but we override them below.
            Err(MockI2cError)
        }

        fn write(&mut self, address: u8, data: &[u8]) -> Result<(), Self::Error> {
            let mut v = heapless::Vec::<u8, 64>::new();
            for &b in data {
                let _ = v.push(b);
            }
            let _ = self.ops.borrow_mut().push(I2cOp::Write {
                addr: address,
                data: v,
            });
            Ok(())
        }

        fn read(&mut self, _address: u8, _data: &mut [u8]) -> Result<(), Self::Error> {
            Ok(())
        }

        fn write_read(
            &mut self,
            address: u8,
            write: &[u8],
            read: &mut [u8],
        ) -> Result<(), Self::Error> {
            let command = if write.is_empty() { 0 } else { write[0] };
            let _ = self.ops.borrow_mut().push(I2cOp::WriteRead {
                addr: address,
                command,
            });

            // Fill read buffer from canned response
            let mut responses = self.read_responses.borrow_mut();
            if let Some(resp) = responses.pop_front() {
                let copy_len = resp.len().min(read.len());
                read[..copy_len].copy_from_slice(&resp[..copy_len]);
                // Zero the rest
                for b in &mut read[copy_len..] {
                    *b = 0;
                }
            } else {
                // No response queued: return all zeros (byte_count=0 means no data)
                for b in read.iter_mut() {
                    *b = 0;
                }
            }
            Ok(())
        }
    }

    // ---- Basic tests ----

    #[test]
    fn new_transport() {
        let mock = MockI2c::new();
        let i2c_ref: &MockI2c = &mock;
        let transport = PmbusTransport::new(i2c_ref, 0x48);
        assert_eq!(transport.device_addr(), 0x48);
        assert_eq!(transport.mtu(), PMBUS_FRAGMENT_PAYLOAD);
        assert_eq!(transport.mtu(), 31);
    }

    #[test]
    fn send_small_frame() {
        let mock = MockI2c::new();
        let i2c_ref: &MockI2c = &mock;
        let mut transport = PmbusTransport::new(i2c_ref, 0x48);

        let src = NodeAddr::new(1, 0, 0);
        let dst = NodeAddr::new(2, 0, 0);
        let topic = TopicId::from_name("voltage");
        let mut frame = Frame::new(src, dst, topic);
        frame.set_payload(&[0x12, 0x34]).unwrap();

        let result = transport.send(&frame);
        assert!(result.is_ok());

        // Should have performed at least one I2C write
        assert!(mock.write_count() > 0);
    }

    #[test]
    fn send_recv_round_trip() {
        // Send a frame, capture the block write data, then feed it back as read responses.
        let tx_mock = MockI2c::new();
        let tx_ref: &MockI2c = &tx_mock;
        let mut sender = PmbusTransport::new(tx_ref, 0x48);

        let src = NodeAddr::new(1, 2, 3);
        let dst = NodeAddr::new(4, 5, 6);
        let topic = TopicId::from_name("pmtest");
        let mut frame = Frame::new(src, dst, topic);
        frame.set_payload(&[0xAA, 0xBB, 0xCC]).unwrap();

        sender.send(&frame).unwrap();

        // Extract the block write payloads (skip command byte and byte_count prefix)
        // Each I2C write is: [command, byte_count, frag_header, data...]
        let ops = tx_mock.ops.borrow();
        let rx_mock = MockI2c::new();

        for op in ops.iter() {
            if let I2cOp::Write { data, .. } = op {
                // data = [command, byte_count, block_data...]
                if data.len() >= 2 {
                    let byte_count = data[1] as usize;
                    let block_data = &data[2..2 + byte_count.min(data.len() - 2)];
                    // Enqueue as read response: [byte_count, block_data...]
                    let mut resp = heapless::Vec::<u8, 64>::new();
                    let _ = resp.push(block_data.len() as u8);
                    for &b in block_data {
                        let _ = resp.push(b);
                    }
                    rx_mock.enqueue_read_response(&resp);
                }
            }
        }

        let rx_ref: &MockI2c = &rx_mock;
        let mut receiver = PmbusTransport::new(rx_ref, 0x48);

        let mut out = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        let mut got_frame = false;
        for _ in 0..20 {
            match receiver.recv(&mut out) {
                Ok(true) => {
                    got_frame = true;
                    break;
                }
                Ok(false) => continue,
                Err(e) => panic!("recv error: {:?}", e),
            }
        }

        assert!(got_frame, "never reassembled a complete frame");
        assert_eq!(out.source, src);
        assert_eq!(out.destination, dst);
        assert_eq!(out.topic, topic);
        assert_eq!(out.payload_slice(), &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn recv_returns_false_when_no_data() {
        let mock = MockI2c::new();
        let i2c_ref: &MockI2c = &mock;
        let mut transport = PmbusTransport::new(i2c_ref, 0x10);

        let mut out = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );
        assert_eq!(transport.recv(&mut out), Ok(false));
    }

    #[test]
    fn mtu_is_31() {
        let mock = MockI2c::new();
        let i2c_ref: &MockI2c = &mock;
        let transport = PmbusTransport::new(i2c_ref, 0x20);
        assert_eq!(transport.mtu(), 31);
    }

    #[test]
    fn into_inner_returns_bus() {
        let mock = MockI2c::new();
        let i2c_ref: &MockI2c = &mock;
        let transport = PmbusTransport::new(i2c_ref, 0x30);
        let _i2c: &MockI2c = transport.into_inner();
    }
}
