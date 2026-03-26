//! CAN 2.0B transport with transparent fragmentation.
//!
//! CAN 2.0 data frames carry at most 8 bytes. A pubsub [`Frame`] serialises
//! to up to 75 bytes (11-byte header + 64-byte payload), so this transport
//! fragments outgoing frames across multiple CAN frames and reassembles
//! incoming ones.
//!
//! # CAN ID encoding (extended 29-bit)
//!
//! ```text
//! [priority:3][topic_hash:18][source_device:8]
//!  28..26       25..8          7..0
//! ```
//!
//! # Fragmentation protocol
//!
//! Byte 0 of each CAN data field is a sequence header:
//!
//! ```text
//! [more_fragments:1][seq_num:7]
//! ```
//!
//! The remaining 7 bytes carry the serialised pubsub frame data.
//! A complete pubsub frame (max 75 bytes) requires at most
//! `ceil(75 / 7) = 11` CAN frames.

use embedded_can::nb::Can;
use embedded_can::{ExtendedId, Frame as CanFrame, Id};
use heapless::Vec;

use super::{Transport, TransportError};
use crate::frame::{Frame, FRAME_HEADER_SIZE, MAX_FRAME_PAYLOAD};

/// Maximum serialised pubsub frame size: header + max payload.
const MAX_WIRE_SIZE: usize = FRAME_HEADER_SIZE + MAX_FRAME_PAYLOAD;

/// Usable data bytes per CAN frame (8 total minus 1 sequence byte).
const CAN_DATA_PER_FRAME: usize = 7;

/// Default priority for outgoing CAN frames (mid-range).
const DEFAULT_PRIORITY: u32 = 0b011;

/// CAN 2.0B transport that wraps an `embedded_can::nb::Can` peripheral.
pub struct CanTransport<C: Can> {
    can: C,
    /// Reassembly buffer for incoming multi-frame messages.
    reassembly_buf: Vec<u8, 128>,
    /// Next expected sequence number during reassembly.
    expected_seq: u8,
}

impl<C: Can> CanTransport<C> {
    /// Create a new CAN transport wrapping the given peripheral.
    pub fn new(can: C) -> Self {
        Self {
            can,
            reassembly_buf: Vec::new(),
            expected_seq: 0,
        }
    }

    /// Build a 29-bit extended CAN ID from priority, topic hash, and source device.
    fn build_can_id(priority: u32, topic: u32, source_device: u8) -> ExtendedId {
        let prio_bits = (priority & 0x07) << 26;
        let topic_bits = (topic & 0x3_FFFF) << 8;
        let src_bits = source_device as u32;
        // SAFETY: The value is at most 0x1FFF_FFFF (29 bits), since
        // prio(3) + topic(18) + src(8) = 29 bits and each field is masked.
        ExtendedId::new(prio_bits | topic_bits | src_bits).expect("29-bit CAN ID must be valid")
    }

    /// Reset the reassembly state, discarding any partial message.
    fn reset_reassembly(&mut self) {
        self.reassembly_buf.clear();
        self.expected_seq = 0;
    }
}

impl<C> Transport for CanTransport<C>
where
    C: Can,
{
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
        // Serialise the pubsub frame into a stack buffer.
        let mut wire = [0u8; MAX_WIRE_SIZE];
        let wire_len = frame
            .to_bytes(&mut wire)
            .map_err(|_| TransportError::FrameTooLarge)?;

        let can_id: Id = Self::build_can_id(
            DEFAULT_PRIORITY,
            frame.topic.as_u32(),
            frame.source.device(),
        )
        .into();

        // Fragment into CAN frames with 7 data bytes each.
        let chunks = wire[..wire_len].chunks(CAN_DATA_PER_FRAME);
        let total_chunks = chunks.len();

        for (seq, chunk) in wire[..wire_len].chunks(CAN_DATA_PER_FRAME).enumerate() {
            let more = if seq < total_chunks - 1 {
                0x80u8
            } else {
                0x00u8
            };
            let seq_byte = more | (seq as u8 & 0x7F);

            let mut can_data = [0u8; 8];
            can_data[0] = seq_byte;
            can_data[1..1 + chunk.len()].copy_from_slice(chunk);
            let data_len = 1 + chunk.len();

            let can_frame = C::Frame::new(can_id, &can_data[..data_len])
                .ok_or(TransportError::FrameTooLarge)?;

            // Blocking send: retry on WouldBlock.
            loop {
                match self.can.transmit(&can_frame) {
                    Ok(_) => break,
                    Err(nb::Error::WouldBlock) => continue,
                    Err(nb::Error::Other(_)) => return Err(TransportError::BusError),
                }
            }
        }

        Ok(())
    }

    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
        // Try to receive a CAN frame (non-blocking).
        let can_frame = match self.can.receive() {
            Ok(f) => f,
            Err(nb::Error::WouldBlock) => return Ok(false),
            Err(nb::Error::Other(_)) => return Err(TransportError::BusError),
        };

        let data = can_frame.data();
        if data.is_empty() {
            // Ignore remote frames or empty data frames.
            return Ok(false);
        }

        let seq_byte = data[0];
        let seq_num = seq_byte & 0x7F;
        let more_fragments = seq_byte & 0x80 != 0;

        // If the sequence number doesn't match, reset and start over.
        // Sequence 0 always starts a new reassembly.
        if seq_num == 0 {
            self.reset_reassembly();
        } else if seq_num != self.expected_seq {
            self.reset_reassembly();
            return Ok(false);
        }

        // Append payload bytes (everything after the sequence byte).
        if self.reassembly_buf.extend_from_slice(&data[1..]).is_err() {
            // Buffer overflow — discard.
            self.reset_reassembly();
            return Err(TransportError::RecvFailed);
        }

        self.expected_seq = seq_num + 1;

        if more_fragments {
            // Not yet complete.
            return Ok(false);
        }

        // All fragments received — deserialise the pubsub frame.
        let result =
            Frame::from_bytes(&self.reassembly_buf).map_err(|_| TransportError::RecvFailed)?;

        *buf = result;
        self.reset_reassembly();
        Ok(true)
    }

    fn mtu(&self) -> usize {
        // Fragmentation is transparent; the logical limit is the full payload.
        MAX_FRAME_PAYLOAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::NodeAddr;
    use crate::topic::TopicId;
    use embedded_can::{self, ErrorKind, Id};

    // ---- Mock CAN frame ------------------------------------------------

    /// A simple CAN frame for testing.
    #[derive(Clone, Debug)]
    struct MockCanFrame {
        id: Id,
        data: heapless::Vec<u8, 8>,
    }

    impl embedded_can::Frame for MockCanFrame {
        fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self> {
            if data.len() > 8 {
                return None;
            }
            let mut v = heapless::Vec::new();
            v.extend_from_slice(data).ok()?;
            Some(Self {
                id: id.into(),
                data: v,
            })
        }

        fn new_remote(_id: impl Into<Id>, _dlc: usize) -> Option<Self> {
            None // not needed for tests
        }

        fn is_extended(&self) -> bool {
            matches!(self.id, Id::Extended(_))
        }

        fn is_remote_frame(&self) -> bool {
            false
        }

        fn id(&self) -> Id {
            self.id
        }

        fn dlc(&self) -> usize {
            self.data.len()
        }

        fn data(&self) -> &[u8] {
            &self.data
        }
    }

    // ---- Mock CAN error ------------------------------------------------

    #[derive(Debug)]
    struct MockCanError;

    impl embedded_can::Error for MockCanError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    // ---- Mock CAN peripheral -------------------------------------------

    /// Mock CAN bus: transmitted frames go into `tx`, received frames are
    /// consumed from `rx`.
    struct MockCan {
        tx: heapless::Vec<MockCanFrame, 32>,
        rx: heapless::Vec<MockCanFrame, 32>,
    }

    impl MockCan {
        fn new() -> Self {
            Self {
                tx: heapless::Vec::new(),
                rx: heapless::Vec::new(),
            }
        }
    }

    impl Can for MockCan {
        type Frame = MockCanFrame;
        type Error = MockCanError;

        fn transmit(
            &mut self,
            frame: &Self::Frame,
        ) -> nb::Result<Option<Self::Frame>, Self::Error> {
            self.tx
                .push(frame.clone())
                .map_err(|_| nb::Error::Other(MockCanError))?;
            Ok(None)
        }

        fn receive(&mut self) -> nb::Result<Self::Frame, Self::Error> {
            if self.rx.is_empty() {
                Err(nb::Error::WouldBlock)
            } else {
                // Pop from front (FIFO).
                Ok(self.rx.remove(0))
            }
        }
    }

    // ---- Helper --------------------------------------------------------

    fn make_frame(payload: &[u8]) -> Frame {
        let src = NodeAddr::new(0x01, 0x42, 0x03);
        let dst = NodeAddr::new(0x02, 0x10, 0x05);
        let topic = TopicId::from_name("test/can");
        let mut f = Frame::new(src, dst, topic);
        f.set_payload(payload).unwrap();
        f
    }

    // ---- Tests ---------------------------------------------------------

    #[test]
    fn can_id_encoding() {
        let id = CanTransport::<MockCan>::build_can_id(0b011, 0x1_2345, 0xAB);
        // priority=3 → bits 28..26 = 0b011 = 0x0C00_0000
        // topic=0x1_2345 → bits 25..8 = 0x0001_2345_00 shifted → 0x0123_4500
        // source=0xAB → bits 7..0
        // 0x0C00_0000 | 0x0123_4500 | 0xAB = 0x0D23_45AB
        let raw = id.as_raw();
        assert_eq!((raw >> 26) & 0x07, 0b011); // priority
        assert_eq!((raw >> 8) & 0x3_FFFF, 0x1_2345); // topic hash (18 bits)
        assert_eq!(raw & 0xFF, 0xAB); // source device
    }

    #[test]
    fn send_small_frame_produces_correct_fragment_count() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        // Empty payload: header only = 11 bytes → ceil(11/7) = 2 CAN frames.
        let f = make_frame(&[]);
        transport.send(&f).unwrap();
        assert_eq!(transport.can.tx.len(), 2);
    }

    #[test]
    fn send_max_frame_produces_11_fragments() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let f = make_frame(&[0xAA; MAX_FRAME_PAYLOAD]);
        transport.send(&f).unwrap();

        // 11 + 64 = 75 bytes → ceil(75/7) = 11 CAN frames.
        assert_eq!(transport.can.tx.len(), 11);
    }

    #[test]
    fn fragment_sequence_bytes_are_correct() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let f = make_frame(&[1, 2, 3]);
        transport.send(&f).unwrap();

        // 11 + 3 = 14 bytes → ceil(14/7) = 2 CAN frames.
        assert_eq!(transport.can.tx.len(), 2);

        // First fragment: more=1, seq=0 → 0x80
        assert_eq!(transport.can.tx[0].data()[0], 0x80);
        // Last fragment: more=0, seq=1 → 0x01
        assert_eq!(transport.can.tx[1].data()[0], 0x01);
    }

    #[test]
    fn round_trip_empty_payload() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let original = make_frame(&[]);
        transport.send(&original).unwrap();

        // Move transmitted frames into the rx buffer for reassembly.
        let sent: heapless::Vec<MockCanFrame, 32> = transport.can.tx.clone();
        transport.can.rx = sent;
        transport.can.tx.clear();

        let mut received = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        // Pump recv until a frame is assembled.
        let mut got_frame = false;
        for _ in 0..20 {
            match transport.recv(&mut received) {
                Ok(true) => {
                    got_frame = true;
                    break;
                }
                Ok(false) => continue,
                Err(e) => panic!("recv error: {:?}", e),
            }
        }

        assert!(got_frame, "should have reassembled the frame");
        assert_eq!(received.source, original.source);
        assert_eq!(received.destination, original.destination);
        assert_eq!(received.topic, original.topic);
        assert_eq!(received.payload_slice(), original.payload_slice());
    }

    #[test]
    fn round_trip_with_payload() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let payload = [10, 20, 30, 40, 50, 60, 70, 80];
        let original = make_frame(&payload);
        transport.send(&original).unwrap();

        let sent: heapless::Vec<MockCanFrame, 32> = transport.can.tx.clone();
        transport.can.rx = sent;
        transport.can.tx.clear();

        let mut received = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        let mut got_frame = false;
        for _ in 0..20 {
            if transport.recv(&mut received).unwrap() {
                got_frame = true;
                break;
            }
        }

        assert!(got_frame);
        assert_eq!(received.payload_slice(), &payload);
        assert_eq!(received.source, original.source);
        assert_eq!(received.destination, original.destination);
    }

    #[test]
    fn round_trip_max_payload() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let payload = [0xBE; MAX_FRAME_PAYLOAD];
        let original = make_frame(&payload);
        transport.send(&original).unwrap();

        let sent: heapless::Vec<MockCanFrame, 32> = transport.can.tx.clone();
        transport.can.rx = sent;
        transport.can.tx.clear();

        let mut received = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        let mut got_frame = false;
        for _ in 0..20 {
            if transport.recv(&mut received).unwrap() {
                got_frame = true;
                break;
            }
        }

        assert!(got_frame);
        assert_eq!(received.payload_slice(), &payload[..]);
    }

    #[test]
    fn recv_returns_false_when_no_data() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        let mut received = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );
        assert_eq!(transport.recv(&mut received).unwrap(), false);
    }

    #[test]
    fn mtu_reports_max_payload() {
        let mock = MockCan::new();
        let transport = CanTransport::new(mock);
        assert_eq!(transport.mtu(), MAX_FRAME_PAYLOAD);
    }

    #[test]
    fn out_of_order_sequence_resets_reassembly() {
        let mock = MockCan::new();
        let mut transport = CanTransport::new(mock);

        // Send a valid frame to get CAN fragments.
        let original = make_frame(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        transport.send(&original).unwrap();

        let mut sent: heapless::Vec<MockCanFrame, 32> = transport.can.tx.clone();
        transport.can.tx.clear();

        // Corrupt: skip fragment 1 (remove it), so seq jumps from 0 to 2.
        if sent.len() > 2 {
            sent.remove(1);
        }

        transport.can.rx = sent;

        let mut received = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        // None of the recv calls should succeed since the sequence is broken.
        let mut got_frame = false;
        for _ in 0..20 {
            match transport.recv(&mut received) {
                Ok(true) => {
                    got_frame = true;
                    break;
                }
                Ok(false) => continue,
                Err(_) => break,
            }
        }

        assert!(!got_frame, "should not reassemble with missing fragment");
    }
}
