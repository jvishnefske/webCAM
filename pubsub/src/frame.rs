//! Wire frame for the pubsub protocol.
//!
//! A frame carries a payload between addressed nodes on a specific topic.
//! The serialised layout is:
//!
//! ```text
//! [source:3][destination:3][topic:4 BE][len:1][payload:0..64]
//! ```

use crate::addr::NodeAddr;
use crate::topic::TopicId;

/// Maximum payload bytes in a single frame.
pub const MAX_FRAME_PAYLOAD: usize = 64;

/// Header size in bytes: source(3) + destination(3) + topic(4) + len(1).
pub const FRAME_HEADER_SIZE: usize = 11;

/// Errors that can occur when building or parsing a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Payload exceeds [`MAX_FRAME_PAYLOAD`] bytes.
    PayloadTooLarge,
    /// Supplied buffer is too small for the serialised frame.
    BufferTooSmall,
    /// The byte slice does not represent a valid frame.
    InvalidFrame,
}

/// A pubsub frame carrying an addressed, topic-tagged payload.
#[derive(Clone, PartialEq)]
pub struct Frame {
    pub source: NodeAddr,
    pub destination: NodeAddr,
    pub topic: TopicId,
    pub payload: [u8; MAX_FRAME_PAYLOAD],
    pub len: u8,
}

impl Frame {
    /// Create a new empty frame addressed from `source` to `destination` on `topic`.
    pub fn new(source: NodeAddr, destination: NodeAddr, topic: TopicId) -> Self {
        Self {
            source,
            destination,
            topic,
            payload: [0u8; MAX_FRAME_PAYLOAD],
            len: 0,
        }
    }

    /// Return the active payload as a byte slice.
    pub fn payload_slice(&self) -> &[u8] {
        &self.payload[..self.len as usize]
    }

    /// Set the payload from a byte slice. Returns an error if `data` is too large.
    pub fn set_payload(&mut self, data: &[u8]) -> Result<(), FrameError> {
        if data.len() > MAX_FRAME_PAYLOAD {
            return Err(FrameError::PayloadTooLarge);
        }
        self.payload[..data.len()].copy_from_slice(data);
        self.len = data.len() as u8;
        Ok(())
    }

    /// Serialise the frame into `buf`. Returns the number of bytes written.
    pub fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, FrameError> {
        let total = FRAME_HEADER_SIZE + self.len as usize;
        if buf.len() < total {
            return Err(FrameError::BufferTooSmall);
        }
        buf[0] = self.source.bus();
        buf[1] = self.source.device();
        buf[2] = self.source.endpoint();
        buf[3] = self.destination.bus();
        buf[4] = self.destination.device();
        buf[5] = self.destination.endpoint();
        let t = self.topic.as_u32().to_be_bytes();
        buf[6..10].copy_from_slice(&t);
        buf[10] = self.len;
        buf[FRAME_HEADER_SIZE..total].copy_from_slice(self.payload_slice());
        Ok(total)
    }

    /// Deserialise a frame from a byte slice.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, FrameError> {
        if buf.len() < FRAME_HEADER_SIZE {
            return Err(FrameError::InvalidFrame);
        }
        let len = buf[10];
        if len as usize > MAX_FRAME_PAYLOAD {
            return Err(FrameError::InvalidFrame);
        }
        let total = FRAME_HEADER_SIZE + len as usize;
        if buf.len() < total {
            return Err(FrameError::InvalidFrame);
        }
        let source = NodeAddr::new(buf[0], buf[1], buf[2]);
        let destination = NodeAddr::new(buf[3], buf[4], buf[5]);
        let topic = TopicId::from_raw(u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]));
        let mut frame = Self::new(source, destination, topic);
        frame
            .set_payload(&buf[FRAME_HEADER_SIZE..total])
            .map_err(|_| FrameError::InvalidFrame)?;
        Ok(frame)
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Frame")
            .field("source", &self.source)
            .field("destination", &self.destination)
            .field("topic", &self.topic)
            .field("len", &self.len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_empty_payload() {
        let src = NodeAddr::new(1, 2, 3);
        let dst = NodeAddr::new(4, 5, 6);
        let topic = TopicId::from_raw(0xAABBCCDD);
        let f = Frame::new(src, dst, topic);

        let mut buf = [0u8; 128];
        let n = f.to_bytes(&mut buf).unwrap();
        assert_eq!(n, FRAME_HEADER_SIZE);

        let f2 = Frame::from_bytes(&buf[..n]).unwrap();
        assert_eq!(f2.source, src);
        assert_eq!(f2.destination, dst);
        assert_eq!(f2.topic, topic);
        assert_eq!(f2.len, 0);
    }

    #[test]
    fn round_trip_with_payload() {
        let src = NodeAddr::new(0x10, 0x20, 0x30);
        let dst = NodeAddr::BROADCAST;
        let topic = TopicId::from_name("test");
        let mut f = Frame::new(src, dst, topic);
        f.set_payload(&[1, 2, 3, 4, 5]).unwrap();

        let mut buf = [0u8; 128];
        let n = f.to_bytes(&mut buf).unwrap();
        assert_eq!(n, FRAME_HEADER_SIZE + 5);

        let f2 = Frame::from_bytes(&buf[..n]).unwrap();
        assert_eq!(f2.payload_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn payload_too_large() {
        let mut f = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        let big = [0u8; MAX_FRAME_PAYLOAD + 1];
        assert_eq!(f.set_payload(&big), Err(FrameError::PayloadTooLarge));
    }

    #[test]
    fn buffer_too_small() {
        let mut f = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        f.set_payload(&[1, 2, 3]).unwrap();
        let mut buf = [0u8; 5];
        assert_eq!(f.to_bytes(&mut buf), Err(FrameError::BufferTooSmall));
    }

    #[test]
    fn from_bytes_invalid() {
        assert_eq!(Frame::from_bytes(&[0u8; 5]), Err(FrameError::InvalidFrame));
    }

    #[test]
    fn max_payload_round_trip() {
        let mut f = Frame::new(
            NodeAddr::new(1, 1, 1),
            NodeAddr::new(2, 2, 2),
            TopicId::from_raw(1),
        );
        let data = [0xAB; MAX_FRAME_PAYLOAD];
        f.set_payload(&data).unwrap();

        let mut buf = [0u8; FRAME_HEADER_SIZE + MAX_FRAME_PAYLOAD];
        let n = f.to_bytes(&mut buf).unwrap();
        let f2 = Frame::from_bytes(&buf[..n]).unwrap();
        assert_eq!(f2.payload_slice(), &data[..]);
    }
}
