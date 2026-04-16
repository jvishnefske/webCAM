//! Node discovery via announce messages.

use crate::message::{type_id_hash, BackplaneMessage};
use crate::node_id::NodeId;

/// Announcement broadcast by a node when it joins the backplane.
///
/// Contains the node's identity, human-readable name, and the sets of
/// message type IDs it publishes and serves (handles requests for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAnnounce {
    /// Identity of the announcing node.
    pub node_id: NodeId,
    /// Human-readable node name (max 64 bytes).
    pub name: heapless::String<64>,
    /// Type IDs of messages this node publishes.
    pub publishes: heapless::Vec<u32, 16>,
    /// Type IDs of messages this node serves (request handlers).
    pub serves: heapless::Vec<u32, 16>,
}

impl BackplaneMessage for NodeAnnounce {
    const TYPE_ID: u32 = type_id_hash("hil_backplane::NodeAnnounce");
}

impl<C> minicbor::Encode<C> for NodeAnnounce {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.map(4)?;
        e.u32(0)?.encode(self.node_id)?;
        e.u32(1)?.str(self.name.as_str())?;
        e.u32(2)?.array(self.publishes.len() as u64)?;
        for id in &self.publishes {
            e.u32(*id)?;
        }
        e.u32(3)?.array(self.serves.len() as u64)?;
        for id in &self.serves {
            e.u32(*id)?;
        }
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for NodeAnnounce {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("expected definite-length map"))?;

        let mut node_id = None;
        let mut name = None;
        let mut publishes = None;
        let mut serves = None;

        for _ in 0..len {
            let key = d.u32()?;
            match key {
                0 => node_id = Some(NodeId::decode(d, ctx)?),
                1 => {
                    let s = d.str()?;
                    name = Some(heapless::String::try_from(s).map_err(|_| {
                        minicbor::decode::Error::message("name too long for heapless::String<64>")
                    })?);
                }
                2 => {
                    let arr_len = d.array()?.ok_or_else(|| {
                        minicbor::decode::Error::message("expected definite-length array")
                    })?;
                    let mut v = heapless::Vec::new();
                    for _ in 0..arr_len {
                        v.push(d.u32()?).map_err(|_| {
                            minicbor::decode::Error::message("publishes list exceeds capacity")
                        })?;
                    }
                    publishes = Some(v);
                }
                3 => {
                    let arr_len = d.array()?.ok_or_else(|| {
                        minicbor::decode::Error::message("expected definite-length array")
                    })?;
                    let mut v = heapless::Vec::new();
                    for _ in 0..arr_len {
                        v.push(d.u32()?).map_err(|_| {
                            minicbor::decode::Error::message("serves list exceeds capacity")
                        })?;
                    }
                    serves = Some(v);
                }
                _ => {
                    d.skip()?;
                }
            }
        }

        Ok(Self {
            node_id: node_id
                .ok_or_else(|| minicbor::decode::Error::message("missing field: node_id"))?,
            name: name.ok_or_else(|| minicbor::decode::Error::message("missing field: name"))?,
            publishes: publishes
                .ok_or_else(|| minicbor::decode::Error::message("missing field: publishes"))?,
            serves: serves
                .ok_or_else(|| minicbor::decode::Error::message("missing field: serves"))?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
mod tests {
    use super::*;

    #[test]
    fn node_announce_roundtrip() {
        let mut publishes = heapless::Vec::new();
        publishes.push(100).unwrap();
        publishes.push(200).unwrap();
        let mut serves = heapless::Vec::new();
        serves.push(300).unwrap();

        let original = NodeAnnounce {
            node_id: NodeId::new(42),
            name: heapless::String::try_from("test-node").unwrap(),
            publishes,
            serves,
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");

        assert_eq!(decoded, original);
    }

    #[test]
    fn node_announce_empty_lists() {
        let original = NodeAnnounce {
            node_id: NodeId::new(0),
            name: heapless::String::try_from("empty").unwrap(),
            publishes: heapless::Vec::new(),
            serves: heapless::Vec::new(),
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");

        assert_eq!(decoded, original);
    }

    #[test]
    fn node_announce_type_id() {
        // Verify the TYPE_ID is deterministic
        let id1 = NodeAnnounce::TYPE_ID;
        let id2 = NodeAnnounce::TYPE_ID;
        assert_eq!(id1, id2);
        assert_ne!(id1, 0);
    }

    #[test]
    fn node_announce_full_lists() {
        let mut publishes = heapless::Vec::new();
        for i in 0..16u32 {
            publishes.push(i * 10).unwrap();
        }
        let mut serves = heapless::Vec::new();
        for i in 0..16u32 {
            serves.push(i * 100).unwrap();
        }

        let original = NodeAnnounce {
            node_id: NodeId::new(255),
            name: heapless::String::try_from("full-node").unwrap(),
            publishes,
            serves,
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");

        assert_eq!(decoded, original);
        assert_eq!(decoded.publishes.len(), 16);
        assert_eq!(decoded.serves.len(), 16);
        assert_eq!(decoded.publishes[0], 0);
        assert_eq!(decoded.publishes[15], 150);
        assert_eq!(decoded.serves[15], 1500);
    }

    #[test]
    fn node_announce_max_node_id() {
        let original = NodeAnnounce {
            node_id: NodeId::new(u32::MAX),
            name: heapless::String::try_from("max-id").unwrap(),
            publishes: heapless::Vec::new(),
            serves: heapless::Vec::new(),
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");
        assert_eq!(decoded, original);
    }

    #[test]
    fn node_announce_long_name() {
        // 64-byte name (max for heapless::String<64>)
        let long_name: String = "a".repeat(64);
        let original = NodeAnnounce {
            node_id: NodeId::new(1),
            name: heapless::String::try_from(long_name.as_str()).unwrap(),
            publishes: heapless::Vec::new(),
            serves: heapless::Vec::new(),
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");
        assert_eq!(decoded, original);
        assert_eq!(decoded.name.len(), 64);
    }

    #[test]
    fn node_announce_only_publishes() {
        let mut publishes = heapless::Vec::new();
        publishes.push(42).unwrap();

        let original = NodeAnnounce {
            node_id: NodeId::new(7),
            name: heapless::String::try_from("pub-only").unwrap(),
            publishes,
            serves: heapless::Vec::new(),
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");
        assert_eq!(decoded, original);
        assert_eq!(decoded.publishes.len(), 1);
        assert!(decoded.serves.is_empty());
    }

    #[test]
    fn node_announce_only_serves() {
        let mut serves = heapless::Vec::new();
        serves.push(99).unwrap();

        let original = NodeAnnounce {
            node_id: NodeId::new(8),
            name: heapless::String::try_from("srv-only").unwrap(),
            publishes: heapless::Vec::new(),
            serves,
        };

        let encoded = minicbor::to_vec(&original).expect("encode failed");
        let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode failed");
        assert_eq!(decoded, original);
        assert!(decoded.publishes.is_empty());
        assert_eq!(decoded.serves.len(), 1);
    }

    #[test]
    fn node_announce_clone_and_debug() {
        let original = NodeAnnounce {
            node_id: NodeId::new(1),
            name: heapless::String::try_from("debug").unwrap(),
            publishes: heapless::Vec::new(),
            serves: heapless::Vec::new(),
        };
        let cloned = original.clone();
        assert_eq!(cloned, original);
        let debug = format!("{:?}", original);
        assert!(debug.contains("NodeAnnounce"));
    }

    #[test]
    fn decode_ignores_unknown_keys() {
        // Encode with an extra unknown key (99) that should be skipped
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(5).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap().array(0).unwrap();
        // Unknown key — should be skipped
        e.u32(99).unwrap().str("ignored").unwrap();
        drop(e);

        let decoded: NodeAnnounce =
            minicbor::decode(&buf).expect("decode should skip unknown keys");
        assert_eq!(decoded.node_id, NodeId::new(1));
        assert_eq!(decoded.name.as_str(), "test");
    }

    #[test]
    fn decode_missing_node_id_fails() {
        // Map with name, publishes, serves but no node_id (key 0)
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(3).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_missing_name_fails() {
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(3).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_missing_publishes_fails() {
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(3).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(3).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_missing_serves_fails() {
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(3).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_name_too_long_fails() {
        let long_name = "a".repeat(65); // exceeds heapless::String<64>
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(4).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str(&long_name).unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_publishes_overflow_fails() {
        // 17 items exceeds heapless::Vec capacity of 16
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(4).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(17).unwrap();
        for i in 0..17u32 {
            e.u32(i).unwrap();
        }
        e.u32(3).unwrap().array(0).unwrap();
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_serves_overflow_fails() {
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(4).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap().array(17).unwrap();
        for i in 0..17u32 {
            e.u32(i).unwrap();
        }
        drop(e);

        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_indefinite_map_fails() {
        // CBOR byte 0xBF starts an indefinite-length map, which our decoder rejects
        let buf = [0xBF, 0xFF];
        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_indefinite_publishes_array_fails() {
        // Definite map with key 2 (publishes) using indefinite-length array
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(4).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        drop(e);
        // Key 2 with indefinite-length array: CBOR 0x9F ... 0xFF
        buf.extend_from_slice(&[0x02, 0x9F, 0xFF]); // key=2, indef array, break
                                                    // Key 3 with empty definite array
        buf.extend_from_slice(&[0x03]);
        // Patch to use raw CBOR -- we need to manually construct the invalid payload
        // Actually, let's use minicbor to build a valid map but patch the array type
        let mut buf2 = Vec::new();
        let mut e2 = minicbor::Encoder::new(&mut buf2);
        e2.map(4).unwrap();
        e2.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e2.u32(1).unwrap().str("test").unwrap();
        e2.u32(2).unwrap();
        drop(e2);
        // Write indefinite array marker (0x9F)
        buf2.push(0x9F);
        // Write break (0xFF)
        buf2.push(0xFF);
        // Key 3 with definite empty array
        {
            let mut e3 = minicbor::Encoder::new(&mut buf2);
            e3.u32(3).unwrap().array(0).unwrap();
            drop(e3);
        }
        let result = minicbor::decode::<NodeAnnounce>(&buf2);
        assert!(result.is_err());
    }

    #[test]
    fn decode_indefinite_serves_array_fails() {
        let mut buf = Vec::new();
        let mut e = minicbor::Encoder::new(&mut buf);
        e.map(4).unwrap();
        e.u32(0).unwrap().encode(NodeId::new(1)).unwrap();
        e.u32(1).unwrap().str("test").unwrap();
        e.u32(2).unwrap().array(0).unwrap();
        e.u32(3).unwrap();
        drop(e);
        // Write indefinite array marker for serves
        buf.push(0x9F);
        buf.push(0xFF);
        let result = minicbor::decode::<NodeAnnounce>(&buf);
        assert!(result.is_err());
    }
}
