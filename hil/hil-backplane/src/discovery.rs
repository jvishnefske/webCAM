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
