//! Canonical dataflow graph model types.
//!
//! This crate provides the single source-of-truth definitions for
//! [`BlockSnapshot`], [`GraphSnapshot`], [`Channel`], [`BlockId`], and
//! [`ChannelId`]. All crates that need these types import from here,
//! eliminating duplication.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub use module_traits::value::{PortDef, PortKind};

/// Opaque block identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u32);

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

/// Pure logical block description. No deployment, no simulation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub id: BlockId,
    pub block_type: String,
    pub name: String,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub is_delay: bool,
}

/// Pure logical graph. Target-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub blocks: Vec<BlockSnapshot>,
    pub channels: Vec<Channel>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn block_id_equality() {
        assert_eq!(BlockId(1), BlockId(1));
        assert_ne!(BlockId(1), BlockId(2));

        // Hash consistency
        let mut set = alloc::collections::BTreeSet::new();
        set.insert(BlockId(1).0);
        set.insert(BlockId(2).0);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn channel_connects_blocks() {
        let ch = Channel {
            id: ChannelId(10),
            from_block: BlockId(1),
            from_port: 0,
            to_block: BlockId(2),
            to_port: 1,
        };
        assert_eq!(ch.from_block, BlockId(1));
        assert_eq!(ch.to_block, BlockId(2));
        assert_eq!(ch.from_port, 0);
        assert_eq!(ch.to_port, 1);
        assert_eq!(ch.id, ChannelId(10));
    }

    #[test]
    fn block_snapshot_serde_roundtrip() {
        let snap = BlockSnapshot {
            id: BlockId(42),
            block_type: "constant".to_string(),
            name: "my_const".to_string(),
            inputs: vec![],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({"value": 3.25}),
            is_delay: false,
        };
        let json = serde_json::to_string(&snap).expect("serialize BlockSnapshot");
        let restored: BlockSnapshot =
            serde_json::from_str(&json).expect("deserialize BlockSnapshot");
        assert_eq!(restored.id, BlockId(42));
        assert_eq!(restored.block_type, "constant");
        assert_eq!(restored.name, "my_const");
        assert_eq!(restored.outputs.len(), 1);
        assert_eq!(restored.outputs[0].name, "out");
        assert!(!restored.is_delay);
    }

    #[test]
    fn graph_snapshot_serde_roundtrip() {
        let snap = GraphSnapshot {
            blocks: vec![
                BlockSnapshot {
                    id: BlockId(1),
                    block_type: "constant".to_string(),
                    name: "src".to_string(),
                    inputs: vec![],
                    outputs: vec![PortDef::new("out", PortKind::Float)],
                    config: serde_json::json!({"value": 5.0}),
                    is_delay: false,
                },
                BlockSnapshot {
                    id: BlockId(2),
                    block_type: "gain".to_string(),
                    name: "amp".to_string(),
                    inputs: vec![PortDef::new("in", PortKind::Float)],
                    outputs: vec![PortDef::new("out", PortKind::Float)],
                    config: serde_json::json!({"gain": 2.0}),
                    is_delay: false,
                },
            ],
            channels: vec![Channel {
                id: ChannelId(1),
                from_block: BlockId(1),
                from_port: 0,
                to_block: BlockId(2),
                to_port: 0,
            }],
        };
        let json = serde_json::to_string(&snap).expect("serialize GraphSnapshot");
        let restored: GraphSnapshot =
            serde_json::from_str(&json).expect("deserialize GraphSnapshot");
        assert_eq!(restored.blocks.len(), 2);
        assert_eq!(restored.channels.len(), 1);
        assert_eq!(restored.channels[0].from_block, BlockId(1));
        assert_eq!(restored.channels[0].to_block, BlockId(2));
    }

    #[test]
    fn block_snapshot_default_config() {
        // Deserialize with missing config and is_delay — defaults should apply
        let json = r#"{
            "id": 1,
            "block_type": "add",
            "name": "adder",
            "inputs": [],
            "outputs": []
        }"#;
        let snap: BlockSnapshot = serde_json::from_str(json).expect("deserialize with defaults");
        assert_eq!(snap.config, serde_json::Value::Null);
        assert!(!snap.is_delay);
    }

    #[test]
    fn block_snapshot_ignores_unknown_fields() {
        // Extra fields like output_values, target, custom_codegen should be
        // silently ignored during deserialization.
        let json = r#"{
            "id": 1,
            "block_type": "constant",
            "name": "c",
            "inputs": [],
            "outputs": [{"name": "out", "kind": "Float"}],
            "config": {"value": 1.0},
            "output_values": [null],
            "target": "RP2040",
            "custom_codegen": "something"
        }"#;
        let snap: BlockSnapshot = serde_json::from_str(json).expect("deserialize with extra fields");
        assert_eq!(snap.id, BlockId(1));
        assert_eq!(snap.block_type, "constant");
    }
}
