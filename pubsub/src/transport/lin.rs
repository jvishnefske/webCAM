//! LIN (Local Interconnect Network) bus transport.
//!
//! LIN is a single-wire UART-based protocol with 8-byte data frames.
//! This transport fragments pub/sub [`Frame`]s into LIN-sized chunks and
//! reassembles them on receive.
//!
//! # LIN frame format (on the wire)
//!
//! ```text
//! SYNC (0x55) | PID (1 byte) | DATA (1-8 bytes) | CHECKSUM (1 byte)
//! ```
//!
//! The PID (Protected Identifier) encodes the topic hash in its lower 6 bits
//! with two parity bits (P0, P1) in bits 6 and 7.
//!
//! # Fragmentation
//!
//! The first data byte of each LIN frame carries a fragmentation header:
//! `[more:1][seq:7]`. The remaining 7 data bytes carry payload.

use crate::frame::{Frame, FRAME_HEADER_SIZE, MAX_FRAME_PAYLOAD};
use crate::transport::{Transport, TransportError};
use heapless::Vec;

/// Maximum data bytes in a single LIN frame.
const LIN_DATA_MAX: usize = 8;

/// Sync byte that begins every LIN frame on the wire.
const LIN_SYNC: u8 = 0x55;

/// Payload bytes per LIN fragment (8 data bytes minus 1 fragmentation header).
const LIN_FRAGMENT_PAYLOAD: usize = LIN_DATA_MAX - 1;

/// Total wire bytes for a single LIN frame: SYNC + PID + DATA(8) + CHECKSUM.
const LIN_WIRE_SIZE: usize = 1 + 1 + LIN_DATA_MAX + 1;

/// User-provided UART for LIN bus communication.
///
/// LIN uses UART at specific baud rates (typically 9600-19200 baud).
/// The implementor handles the physical-layer timing; this trait provides
/// the byte-level read/write interface.
pub trait LinUart {
    /// Error type for UART operations.
    type Error: core::fmt::Debug;

    /// Write all bytes in `buf` to the UART.
    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error>;

    /// Read bytes from the UART into `buf`.
    ///
    /// Returns the number of bytes actually read (may be less than `buf.len()`).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Compute the LIN Protected Identifier from a raw 6-bit frame ID.
///
/// PID layout: `[P1:1][P0:1][ID5:1]...[ID0:1]`
/// - P0 = ID0 ^ ID1 ^ ID2 ^ ID4 (even parity of selected bits)
/// - P1 = !(ID1 ^ ID3 ^ ID4 ^ ID5) (odd parity of selected bits)
pub fn compute_pid(id: u8) -> u8 {
    let id = id & 0x3F; // mask to 6 bits
    let p0 = ((id >> 0) ^ (id >> 1) ^ (id >> 2) ^ (id >> 4)) & 1;
    let p1 = (((id >> 1) ^ (id >> 3) ^ (id >> 4) ^ (id >> 5)) & 1) ^ 1;
    id | (p0 << 6) | (p1 << 7)
}

/// Extract the raw 6-bit frame ID from a PID byte.
pub fn pid_to_id(pid: u8) -> u8 {
    pid & 0x3F
}

/// Verify the parity bits of a PID byte.
pub fn verify_pid(pid: u8) -> bool {
    compute_pid(pid & 0x3F) == pid
}

/// Compute the LIN enhanced checksum.
///
/// Enhanced checksum (LIN 2.x): sum of all data bytes **and** the PID,
/// with carry-add, then bitwise inverted.
pub fn enhanced_checksum(pid: u8, data: &[u8]) -> u8 {
    let mut sum: u16 = pid as u16;
    for &b in data {
        sum += b as u16;
        if sum > 0xFF {
            sum = (sum & 0xFF) + 1; // carry-add
        }
    }
    !(sum as u8)
}

/// LIN bus transport with fragmentation and reassembly.
pub struct LinTransport<U: LinUart> {
    uart: U,
    reassembly_buf: Vec<u8, 128>,
    expected_seq: u8,
}

impl<U: LinUart> LinTransport<U> {
    /// Create a new LIN transport wrapping the given UART.
    pub fn new(uart: U) -> Self {
        Self {
            uart,
            reassembly_buf: Vec::new(),
            expected_seq: 0,
        }
    }

    /// Consume the transport and return the inner UART.
    pub fn into_inner(self) -> U {
        self.uart
    }

    /// Send a single LIN wire frame: SYNC + PID + data + CHECKSUM.
    fn send_lin_frame(&mut self, pid: u8, data: &[u8]) -> Result<(), TransportError> {
        let mut wire = [0u8; LIN_WIRE_SIZE];
        wire[0] = LIN_SYNC;
        wire[1] = pid;
        let dlen = data.len().min(LIN_DATA_MAX);
        wire[2..2 + dlen].copy_from_slice(&data[..dlen]);
        // Pad remaining data bytes with 0xFF (LIN convention)
        for b in &mut wire[2 + dlen..2 + LIN_DATA_MAX] {
            *b = 0xFF;
        }
        wire[2 + LIN_DATA_MAX] = enhanced_checksum(pid, &wire[2..2 + LIN_DATA_MAX]);

        self.uart
            .write(&wire[..3 + LIN_DATA_MAX])
            .map_err(|_| TransportError::SendFailed)
    }

    /// Try to receive a single LIN wire frame.
    ///
    /// Returns `Ok(Some((pid, data_buf, data_len)))` or `Ok(None)` if nothing
    /// available.
    fn recv_lin_frame(
        &mut self,
    ) -> Result<Option<(u8, [u8; LIN_DATA_MAX], usize)>, TransportError> {
        let mut wire = [0u8; LIN_WIRE_SIZE];
        let n = self
            .uart
            .read(&mut wire)
            .map_err(|_| TransportError::RecvFailed)?;
        if n == 0 {
            return Ok(None);
        }
        // Need at least SYNC + PID + 1 data byte + CHECKSUM
        if n < 4 {
            return Err(TransportError::BusError);
        }
        if wire[0] != LIN_SYNC {
            return Err(TransportError::BusError);
        }
        let pid = wire[1];
        if !verify_pid(pid) {
            return Err(TransportError::BusError);
        }

        // Data length: everything between PID and checksum
        let data_len = n - 3; // minus SYNC, PID, CHECKSUM
        let data_len = data_len.min(LIN_DATA_MAX);
        let checksum_idx = 2 + data_len;

        // Verify checksum over full 8-byte data field (padded with 0xFF)
        let mut data_buf = [0xFFu8; LIN_DATA_MAX];
        data_buf[..data_len].copy_from_slice(&wire[2..2 + data_len]);

        let expected_cs = enhanced_checksum(pid, &data_buf);
        if n > checksum_idx && wire[checksum_idx] != expected_cs {
            return Err(TransportError::BusError);
        }

        Ok(Some((pid, data_buf, data_len)))
    }

    /// Reset the reassembly state.
    fn reset_reassembly(&mut self) {
        self.reassembly_buf.clear();
        self.expected_seq = 0;
    }
}

impl<U: LinUart> Transport for LinTransport<U> {
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
        // Serialize the full pub/sub frame to bytes
        let mut frame_bytes = [0u8; FRAME_HEADER_SIZE + MAX_FRAME_PAYLOAD];
        let total = frame
            .to_bytes(&mut frame_bytes)
            .map_err(|_| TransportError::FrameTooLarge)?;

        // PID: lower 6 bits of topic hash
        let pid = compute_pid((frame.topic.as_u32() & 0x3F) as u8);

        // Fragment into LIN-sized pieces
        let mut offset = 0;
        let mut seq: u8 = 0;

        while offset < total {
            let remaining = total - offset;
            let chunk = remaining.min(LIN_FRAGMENT_PAYLOAD);
            let more = if offset + chunk < total {
                0x80u8
            } else {
                0x00u8
            };

            let mut data = [0xFFu8; LIN_DATA_MAX];
            data[0] = more | (seq & 0x7F);
            data[1..1 + chunk].copy_from_slice(&frame_bytes[offset..offset + chunk]);

            self.send_lin_frame(pid, &data)?;

            offset += chunk;
            seq += 1;
        }

        Ok(())
    }

    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
        let (_pid, data, data_len) = match self.recv_lin_frame()? {
            Some(v) => v,
            None => return Ok(false),
        };

        if data_len < 1 {
            return Err(TransportError::BusError);
        }

        let header = data[0];
        let more = (header & 0x80) != 0;
        let seq = header & 0x7F;

        // Check sequence continuity
        if seq != self.expected_seq {
            self.reset_reassembly();
            if seq != 0 {
                return Err(TransportError::BusError);
            }
        }

        // Append payload bytes (skip the fragmentation header)
        let payload_len = (data_len - 1).min(LIN_FRAGMENT_PAYLOAD);
        for i in 0..payload_len {
            self.reassembly_buf
                .push(data[1 + i])
                .map_err(|_| TransportError::FrameTooLarge)?;
        }
        self.expected_seq = seq + 1;

        if more {
            // More fragments to come
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
        LIN_FRAGMENT_PAYLOAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::NodeAddr;
    use crate::topic::TopicId;
    use core::cell::RefCell;
    use heapless::Deque;

    /// Mock UART backed by heapless queues for deterministic testing.
    struct MockUart {
        /// Bytes written by the transport (outgoing wire data).
        tx: RefCell<Vec<u8, 512>>,
        /// Bytes to be read by the transport (incoming wire data).
        rx: RefCell<Deque<u8, 512>>,
    }

    #[derive(Debug)]
    struct MockError;

    impl MockUart {
        fn new() -> Self {
            Self {
                tx: RefCell::new(Vec::new()),
                rx: RefCell::new(Deque::new()),
            }
        }

        /// Push wire bytes that will be returned by the next `read` call.
        fn push_rx(&self, data: &[u8]) {
            let mut rx = self.rx.borrow_mut();
            for &b in data {
                let _ = rx.push_back(b);
            }
        }

        /// Drain all transmitted bytes.
        fn drain_tx(&self) -> Vec<u8, 512> {
            let mut tx = self.tx.borrow_mut();
            let out = tx.clone();
            tx.clear();
            out
        }
    }

    impl LinUart for &MockUart {
        type Error = MockError;

        fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            let mut tx = self.tx.borrow_mut();
            for &b in buf {
                tx.push(b).map_err(|_| MockError)?;
            }
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let mut rx = self.rx.borrow_mut();
            let mut count = 0;
            for slot in buf.iter_mut() {
                match rx.pop_front() {
                    Some(b) => {
                        *slot = b;
                        count += 1;
                    }
                    None => break,
                }
            }
            Ok(count)
        }
    }

    // ---- PID tests ----

    #[test]
    fn pid_parity_round_trip() {
        for id in 0..64u8 {
            let pid = compute_pid(id);
            assert_eq!(pid_to_id(pid), id);
            assert!(verify_pid(pid), "PID parity failed for id={}", id);
        }
    }

    #[test]
    fn pid_known_values() {
        // ID=0x00 -> P0=0, P1=1 -> PID=0x80
        assert_eq!(compute_pid(0x00), 0x80);
        // ID=0x3F -> all ID bits set
        let pid = compute_pid(0x3F);
        assert!(verify_pid(pid));
    }

    #[test]
    fn pid_bad_parity_rejected() {
        let pid = compute_pid(0x0A);
        // Flip parity bit P0
        let bad = pid ^ 0x40;
        assert!(!verify_pid(bad));
    }

    // ---- Checksum tests ----

    #[test]
    fn enhanced_checksum_known() {
        // Simple case: PID=0x80 (id=0), data=[0x01]
        // sum = 0x80 + 0x01 = 0x81, no carry, inverted = 0x7E
        let cs = enhanced_checksum(0x80, &[0x01]);
        assert_eq!(cs, 0x7E);
    }

    #[test]
    fn enhanced_checksum_with_carry() {
        // PID=0x00, data=[0xFF, 0x02]
        // sum = 0x00 + 0xFF = 0xFF, then + 0x02 = 0x101 -> carry -> 0x02
        // inverted = 0xFD
        let cs = enhanced_checksum(0x00, &[0xFF, 0x02]);
        assert_eq!(cs, 0xFD);
    }

    #[test]
    fn enhanced_checksum_empty_data() {
        // PID=0x10, no data: sum = 0x10, inverted = 0xEF
        let cs = enhanced_checksum(0x10, &[]);
        assert_eq!(cs, 0xEF);
    }

    // ---- Transport send/recv tests ----

    #[test]
    fn send_small_frame() {
        let mock = MockUart::new();
        let uart_ref: &MockUart = &mock;
        let mut transport = LinTransport::new(uart_ref);

        let src = NodeAddr::new(1, 0, 0);
        let dst = NodeAddr::new(2, 0, 0);
        let topic = TopicId::from_name("test");
        let mut frame = Frame::new(src, dst, topic);
        frame.set_payload(&[0xAA, 0xBB]).unwrap();

        let result = transport.send(&frame);
        assert!(result.is_ok());

        // Should have sent at least one LIN wire frame
        let tx_data = mock.drain_tx();
        assert!(!tx_data.is_empty());

        // First byte of every wire frame is SYNC
        assert_eq!(tx_data[0], LIN_SYNC);
    }

    #[test]
    fn send_recv_round_trip() {
        // Send a frame, capture wire bytes, then feed them back to recv.
        let tx_mock = MockUart::new();
        let tx_ref: &MockUart = &tx_mock;
        let mut sender = LinTransport::new(tx_ref);

        let src = NodeAddr::new(1, 0, 0);
        let dst = NodeAddr::new(2, 0, 0);
        let topic = TopicId::from_name("rt");
        let mut frame = Frame::new(src, dst, topic);
        frame.set_payload(&[10, 20, 30]).unwrap();

        sender.send(&frame).unwrap();
        let wire = tx_mock.drain_tx();

        // Feed wire bytes into receiver
        let rx_mock = MockUart::new();
        rx_mock.push_rx(&wire);
        let rx_ref: &MockUart = &rx_mock;
        let mut receiver = LinTransport::new(rx_ref);

        let mut out = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        // Pump recv until we get a complete frame.
        // Each call reads one LIN wire frame (11 bytes).
        let mut got_frame = false;
        for _ in 0..50 {
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
        assert_eq!(out.payload_slice(), &[10, 20, 30]);
    }

    #[test]
    fn recv_returns_false_when_empty() {
        let mock = MockUart::new();
        let uart_ref: &MockUart = &mock;
        let mut transport = LinTransport::new(uart_ref);

        let mut out = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );
        let result = transport.recv(&mut out);
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn mtu_is_seven() {
        let mock = MockUart::new();
        let uart_ref: &MockUart = &mock;
        let transport = LinTransport::new(uart_ref);
        assert_eq!(transport.mtu(), LIN_FRAGMENT_PAYLOAD);
        assert_eq!(transport.mtu(), 7);
    }
}
