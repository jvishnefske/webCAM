//! Channels connecting block output ports to input ports.

use super::block::BlockId;
use serde::{Deserialize, Serialize};

/// Opaque channel identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u32);

/// A directed connection from one output port to one input port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub from_block: BlockId,
    pub from_port: usize,
    pub to_block: BlockId,
    pub to_port: usize,
}
