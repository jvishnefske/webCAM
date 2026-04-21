//! Subscription discovery for the mesh router.
//!
//! Two reserved control topics carry routing information between bridges:
//!
//! - `topic!("_ps/sub")` — a node announces that it subscribes to a topic.
//! - `topic!("_ps/unsub")` — a node announces it no longer subscribes.
//!
//! The payload is a fixed 7-byte layout (no CBOR dependency on the control
//! path): `[topic:4 BE][bus:1][device:1][endpoint:1]`.
//!
//! Bridges receiving `_ps/sub` on interface `i` record a [`MeshRouter`] route
//! toward `i` for the announced topic, and re-emit the announcement on every
//! other interface that does not already have a route for this topic. The
//! re-emit suppression is what keeps the advertise graph acyclic, which in
//! turn prevents forwarding loops.
//!
//! [`MeshRouter`]: crate::router::MeshRouter

use crate::addr::NodeAddr;
use crate::topic::TopicId;

/// Control topic: a subscriber announces interest in a topic.
pub const SUBSCRIBE_TOPIC: TopicId = TopicId::from_name("_ps/sub");

/// Control topic: a subscriber withdraws interest in a topic.
pub const UNSUBSCRIBE_TOPIC: TopicId = TopicId::from_name("_ps/unsub");

/// Fixed wire layout for a subscription announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscribeAnnouncement {
    /// Topic the announcer subscribes to (raw `u32`).
    pub topic: u32,
    /// Node address of the subscriber.
    pub remote: NodeAddr,
}

impl SubscribeAnnouncement {
    /// Serialised size in bytes.
    pub const SIZE: usize = 7;

    /// Create an announcement for the given topic and subscriber address.
    pub const fn new(topic: TopicId, remote: NodeAddr) -> Self {
        Self {
            topic: topic.as_u32(),
            remote,
        }
    }

    /// Return the topic as a [`TopicId`].
    pub const fn topic_id(&self) -> TopicId {
        TopicId::from_raw(self.topic)
    }

    /// Encode into a fixed-size buffer.
    pub fn encode(&self, buf: &mut [u8; Self::SIZE]) {
        let t = self.topic.to_be_bytes();
        buf[0] = t[0];
        buf[1] = t[1];
        buf[2] = t[2];
        buf[3] = t[3];
        buf[4] = self.remote.bus();
        buf[5] = self.remote.device();
        buf[6] = self.remote.endpoint();
    }

    /// Decode from a byte slice. Returns `None` if the slice is too short.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        let topic = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let remote = NodeAddr::new(buf[4], buf[5], buf[6]);
        Some(Self { topic, remote })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_are_distinct() {
        assert_ne!(SUBSCRIBE_TOPIC, UNSUBSCRIBE_TOPIC);
    }

    #[test]
    fn announcement_round_trip() {
        let ann = SubscribeAnnouncement::new(
            TopicId::from_name("sensor/temp"),
            NodeAddr::new(0x12, 0x34, 0x56),
        );
        let mut buf = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut buf);
        let decoded = SubscribeAnnouncement::decode(&buf).unwrap();
        assert_eq!(decoded, ann);
        assert_eq!(decoded.topic_id(), TopicId::from_name("sensor/temp"));
    }

    #[test]
    fn decode_rejects_short_buffer() {
        let buf = [0u8; SubscribeAnnouncement::SIZE - 1];
        assert!(SubscribeAnnouncement::decode(&buf).is_none());
    }

    #[test]
    fn decode_accepts_longer_buffer() {
        // Extra trailing bytes should be ignored.
        let mut buf = [0u8; SubscribeAnnouncement::SIZE + 4];
        let ann =
            SubscribeAnnouncement::new(TopicId::from_raw(0xDEAD_BEEF), NodeAddr::new(1, 2, 3));
        let mut head = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut head);
        buf[..SubscribeAnnouncement::SIZE].copy_from_slice(&head);
        let decoded = SubscribeAnnouncement::decode(&buf).unwrap();
        assert_eq!(decoded, ann);
    }

    #[test]
    fn big_endian_topic_encoding() {
        let ann = SubscribeAnnouncement::new(
            TopicId::from_raw(0x11_22_33_44),
            NodeAddr::new(0xAA, 0xBB, 0xCC),
        );
        let mut buf = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut buf);
        assert_eq!(buf, [0x11, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC]);
    }
}
