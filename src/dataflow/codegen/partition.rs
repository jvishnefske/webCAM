//! Graph partitioner for distributed multi-target codegen.
//!
//! Takes a [`GraphSnapshot`] with per-block target assignments and splits it
//! into one subgraph per [`TargetFamily`], inserting pub/sub bridge blocks
//! at cross-partition channel boundaries.

use std::collections::HashMap;

use serde_json::json;

use crate::dataflow::block::{PortDef, PortKind};
use crate::dataflow::channel::{Channel, ChannelId};
use crate::dataflow::codegen::target::TargetFamily;
use crate::dataflow::graph::{BlockSnapshot, GraphSnapshot};

/// Errors that can occur during graph partitioning.
#[derive(Debug)]
pub enum PartitionError {
    /// A block has no target assignment.
    UnassignedBlock(u32),
    /// The graph has no blocks.
    EmptyGraph,
}

/// Configuration for a pub/sub bridge connection.
#[derive(Debug, Clone)]
pub struct BridgeInfo {
    pub topic: String,
    pub port_kind: PortKind,
    pub source_target: TargetFamily,
    pub sink_target: TargetFamily,
}

/// Result of partitioning a graph.
#[derive(Debug)]
pub struct PartitionResult {
    /// Per-target subgraphs.
    pub partitions: HashMap<TargetFamily, GraphSnapshot>,
    /// Bridge connections that were created.
    pub bridges: Vec<BridgeInfo>,
}

/// Partition a graph snapshot into per-target subgraphs.
///
/// Every block in the snapshot must have a `target` assignment. Cross-partition
/// channels are replaced with pub/sub bridge block pairs sharing a deterministic
/// topic name.
pub fn partition_graph(snap: &GraphSnapshot) -> Result<PartitionResult, PartitionError> {
    if snap.blocks.is_empty() {
        return Err(PartitionError::EmptyGraph);
    }

    // Validate all blocks are assigned and build lookup.
    let mut block_target: HashMap<u32, TargetFamily> = HashMap::new();
    for b in &snap.blocks {
        match b.target {
            Some(t) => {
                block_target.insert(b.id, t);
            }
            None => return Err(PartitionError::UnassignedBlock(b.id)),
        }
    }

    // Group blocks by target.
    let mut target_blocks: HashMap<TargetFamily, Vec<BlockSnapshot>> = HashMap::new();
    for b in &snap.blocks {
        let t = block_target[&b.id];
        target_blocks.entry(t).or_default().push(BlockSnapshot {
            id: b.id,
            block_type: b.block_type.clone(),
            name: b.name.clone(),
            inputs: b.inputs.clone(),
            outputs: b.outputs.clone(),
            config: b.config.clone(),
            output_values: b.output_values.clone(),
            target: b.target,
        });
    }

    // Determine max block ID for bridge ID allocation.
    let max_id = snap.blocks.iter().map(|b| b.id).max().unwrap_or(0);
    let mut next_bridge_id = max_id + 1000;

    let mut target_channels: HashMap<TargetFamily, Vec<Channel>> = HashMap::new();
    let mut next_channel_id = snap
        .channels
        .iter()
        .map(|c| c.id.0)
        .max()
        .unwrap_or(0)
        + 1;
    let mut bridges: Vec<BridgeInfo> = Vec::new();

    for ch in &snap.channels {
        let from_target = block_target[&ch.from_block.0];
        let to_target = block_target[&ch.to_block.0];

        if from_target == to_target {
            // Intra-partition: keep as-is.
            target_channels
                .entry(from_target)
                .or_default()
                .push(ch.clone());
        } else {
            // Cross-partition: insert bridge pair.
            let topic = format!("bridge_{}_{}", ch.from_block.0, ch.from_port);

            // Determine port kind from the source block's output port.
            let source_block = snap.blocks.iter().find(|b| b.id == ch.from_block.0).unwrap();
            let port_kind = if ch.from_port < source_block.outputs.len() {
                source_block.outputs[ch.from_port].kind.clone()
            } else {
                PortKind::Any
            };

            // Create pubsub_sink in sender's partition.
            let sink_id = next_bridge_id;
            next_bridge_id += 1;
            let sink_block = BlockSnapshot {
                id: sink_id,
                block_type: "pubsub_sink".to_string(),
                name: format!("PubSub Sink ({})", topic),
                inputs: vec![PortDef::new("in", port_kind.clone())],
                outputs: vec![],
                config: json!({"topic": topic}),
                output_values: vec![],
                target: Some(from_target),
            };
            target_blocks
                .entry(from_target)
                .or_default()
                .push(sink_block);

            // Wire: original output -> pubsub_sink input.
            let sink_channel = Channel {
                id: ChannelId(next_channel_id),
                from_block: ch.from_block,
                from_port: ch.from_port,
                to_block: crate::dataflow::block::BlockId(sink_id),
                to_port: 0,
            };
            next_channel_id += 1;
            target_channels
                .entry(from_target)
                .or_default()
                .push(sink_channel);

            // Create pubsub_source in receiver's partition.
            let source_id = next_bridge_id;
            next_bridge_id += 1;
            let source_block = BlockSnapshot {
                id: source_id,
                block_type: "pubsub_source".to_string(),
                name: format!("PubSub Source ({})", topic),
                inputs: vec![],
                outputs: vec![PortDef::new("out", port_kind.clone())],
                config: json!({"topic": topic}),
                output_values: vec![None],
                target: Some(to_target),
            };
            target_blocks
                .entry(to_target)
                .or_default()
                .push(source_block);

            // Wire: pubsub_source output -> original input.
            let source_channel = Channel {
                id: ChannelId(next_channel_id),
                from_block: crate::dataflow::block::BlockId(source_id),
                from_port: 0,
                to_block: ch.to_block,
                to_port: ch.to_port,
            };
            next_channel_id += 1;
            target_channels
                .entry(to_target)
                .or_default()
                .push(source_channel);

            bridges.push(BridgeInfo {
                topic,
                port_kind,
                source_target: from_target,
                sink_target: to_target,
            });
        }
    }

    // Assemble partitions.
    let mut partitions: HashMap<TargetFamily, GraphSnapshot> = HashMap::new();
    for (target, blocks) in target_blocks {
        let channels = target_channels.remove(&target).unwrap_or_default();
        partitions.insert(
            target,
            GraphSnapshot {
                blocks,
                channels,
                tick_count: snap.tick_count,
                time: snap.time,
            },
        );
    }

    Ok(PartitionResult {
        partitions,
        bridges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::block::{BlockId, PortDef, PortKind};
    use crate::dataflow::channel::{Channel, ChannelId};
    use crate::dataflow::codegen::target::TargetFamily;
    use crate::dataflow::graph::{BlockSnapshot, GraphSnapshot};

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    fn make_block(id: u32, target: TargetFamily) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "constant".to_string(),
            name: format!("Block {}", id),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({}),
            output_values: vec![None],
            target: Some(target),
        }
    }

    fn make_source_block(id: u32, target: TargetFamily) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "constant".to_string(),
            name: format!("Source {}", id),
            inputs: vec![],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({"value": 1.0}),
            output_values: vec![None],
            target: Some(target),
        }
    }

    fn make_sink_block(id: u32, target: TargetFamily) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "plot".to_string(),
            name: format!("Sink {}", id),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![],
            config: serde_json::json!({}),
            output_values: vec![],
            target: Some(target),
        }
    }

    fn make_channel(id: u32, from_block: u32, from_port: usize, to_block: u32, to_port: usize) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from_block),
            from_port,
            to_block: BlockId(to_block),
            to_port,
        }
    }

    fn make_graph(blocks: Vec<BlockSnapshot>, channels: Vec<Channel>) -> GraphSnapshot {
        GraphSnapshot {
            blocks,
            channels,
            tick_count: 0,
            time: 0.0,
        }
    }

    // ---------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------

    #[test]
    fn empty_graph_returns_error() {
        let snap = make_graph(vec![], vec![]);
        let result = partition_graph(&snap);
        assert!(matches!(result, Err(PartitionError::EmptyGraph)));
    }

    #[test]
    fn unassigned_block_returns_error() {
        let mut block = make_block(1, TargetFamily::Rp2040);
        block.target = None; // unassigned
        let snap = make_graph(vec![block], vec![]);
        let result = partition_graph(&snap);
        assert!(matches!(result, Err(PartitionError::UnassignedBlock(1))));
    }

    #[test]
    fn single_target_no_bridges() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_block(2, TargetFamily::Rp2040),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.partitions.len(), 1);
        assert!(result.partitions.contains_key(&TargetFamily::Rp2040));
        assert!(result.bridges.is_empty());

        let part = &result.partitions[&TargetFamily::Rp2040];
        assert_eq!(part.blocks.len(), 2);
        assert_eq!(part.channels.len(), 1);
        // Original channel preserved.
        assert_eq!(part.channels[0].id, ChannelId(1));
    }

    #[test]
    fn two_targets_no_cross_partition_channels() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_source_block(2, TargetFamily::Stm32f4),
            ],
            vec![],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.partitions.len(), 2);
        assert!(result.partitions.contains_key(&TargetFamily::Rp2040));
        assert!(result.partitions.contains_key(&TargetFamily::Stm32f4));
        assert!(result.bridges.is_empty());

        assert_eq!(result.partitions[&TargetFamily::Rp2040].blocks.len(), 1);
        assert_eq!(result.partitions[&TargetFamily::Stm32f4].blocks.len(), 1);
    }

    #[test]
    fn two_targets_one_cross_partition_channel() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_sink_block(2, TargetFamily::Stm32f4),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.partitions.len(), 2);
        assert_eq!(result.bridges.len(), 1);

        let bridge = &result.bridges[0];
        assert_eq!(bridge.topic, "bridge_1_0");
        assert_eq!(bridge.source_target, TargetFamily::Rp2040);
        assert_eq!(bridge.sink_target, TargetFamily::Stm32f4);
        assert_eq!(bridge.port_kind, PortKind::Float);
    }

    #[test]
    fn cross_partition_creates_pubsub_sink_in_sender_partition() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_sink_block(2, TargetFamily::Stm32f4),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap).unwrap();
        let rp_part = &result.partitions[&TargetFamily::Rp2040];

        // Should have original block + pubsub_sink
        assert_eq!(rp_part.blocks.len(), 2);
        let sink = rp_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_sink")
            .expect("pubsub_sink should exist in sender partition");
        assert_eq!(sink.inputs.len(), 1);
        assert_eq!(sink.inputs[0].name, "in");
        assert_eq!(sink.inputs[0].kind, PortKind::Float);
        assert_eq!(sink.outputs.len(), 0);
        assert_eq!(sink.config["topic"], "bridge_1_0");
        assert_eq!(sink.target, Some(TargetFamily::Rp2040));
    }

    #[test]
    fn cross_partition_creates_pubsub_source_in_receiver_partition() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_sink_block(2, TargetFamily::Stm32f4),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap).unwrap();
        let stm_part = &result.partitions[&TargetFamily::Stm32f4];

        // Should have original block + pubsub_source
        assert_eq!(stm_part.blocks.len(), 2);
        let source = stm_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_source")
            .expect("pubsub_source should exist in receiver partition");
        assert_eq!(source.inputs.len(), 0);
        assert_eq!(source.outputs.len(), 1);
        assert_eq!(source.outputs[0].name, "out");
        assert_eq!(source.outputs[0].kind, PortKind::Float);
        assert_eq!(source.config["topic"], "bridge_1_0");
        assert_eq!(source.output_values, vec![None]);
        assert_eq!(source.target, Some(TargetFamily::Stm32f4));
    }

    #[test]
    fn bridge_blocks_have_correct_topic_names() {
        // Block 5, output port 2 -> topic should be "bridge_5_2"
        let mut src = make_source_block(5, TargetFamily::Host);
        // Give it 3 output ports so port index 2 is valid.
        src.outputs = vec![
            PortDef::new("out0", PortKind::Float),
            PortDef::new("out1", PortKind::Float),
            PortDef::new("out2", PortKind::Text),
        ];

        let snap = make_graph(
            vec![src, make_sink_block(6, TargetFamily::Esp32c3)],
            vec![make_channel(1, 5, 2, 6, 0)],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.bridges[0].topic, "bridge_5_2");
    }

    #[test]
    fn bridge_blocks_have_correct_port_kinds() {
        // Source outputs Text on port 0.
        let mut src = make_source_block(1, TargetFamily::Rp2040);
        src.outputs = vec![PortDef::new("out", PortKind::Text)];

        let mut sink = make_sink_block(2, TargetFamily::Stm32f4);
        sink.inputs = vec![PortDef::new("in", PortKind::Text)];

        let snap = make_graph(vec![src, sink], vec![make_channel(1, 1, 0, 2, 0)]);

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.bridges[0].port_kind, PortKind::Text);

        // Verify sink block port kind matches.
        let rp_part = &result.partitions[&TargetFamily::Rp2040];
        let pubsub_sink = rp_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_sink")
            .unwrap();
        assert_eq!(pubsub_sink.inputs[0].kind, PortKind::Text);

        // Verify source block port kind matches.
        let stm_part = &result.partitions[&TargetFamily::Stm32f4];
        let pubsub_source = stm_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_source")
            .unwrap();
        assert_eq!(pubsub_source.outputs[0].kind, PortKind::Text);
    }

    #[test]
    fn multiple_cross_partition_channels_create_multiple_bridges() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_block(2, TargetFamily::Rp2040),
                make_sink_block(3, TargetFamily::Stm32f4),
                make_sink_block(4, TargetFamily::Stm32f4),
            ],
            vec![
                make_channel(1, 1, 0, 2, 0), // intra-partition
                make_channel(2, 1, 0, 3, 0), // cross-partition
                make_channel(3, 2, 0, 4, 0), // cross-partition
            ],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Each bridge should have a unique topic.
        let topics: Vec<&str> = result.bridges.iter().map(|b| b.topic.as_str()).collect();
        assert!(topics.contains(&"bridge_1_0"));
        assert!(topics.contains(&"bridge_2_0"));
    }

    #[test]
    fn three_target_graph() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_block(2, TargetFamily::Stm32f4),
                make_sink_block(3, TargetFamily::Esp32c3),
            ],
            vec![
                make_channel(1, 1, 0, 2, 0),
                make_channel(2, 2, 0, 3, 0),
            ],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.partitions.len(), 3);
        assert!(result.partitions.contains_key(&TargetFamily::Rp2040));
        assert!(result.partitions.contains_key(&TargetFamily::Stm32f4));
        assert!(result.partitions.contains_key(&TargetFamily::Esp32c3));
        assert_eq!(result.bridges.len(), 2);
    }

    #[test]
    fn fan_out_across_partitions() {
        // One source block fans out to two blocks on different targets.
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_sink_block(2, TargetFamily::Stm32f4),
                make_sink_block(3, TargetFamily::Esp32c3),
            ],
            vec![
                make_channel(1, 1, 0, 2, 0),
                make_channel(2, 1, 0, 3, 0),
            ],
        );

        let result = partition_graph(&snap).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Both bridges originate from Rp2040.
        for bridge in &result.bridges {
            assert_eq!(bridge.source_target, TargetFamily::Rp2040);
        }

        // Sender partition should have 2 pubsub_sink blocks (one per fan-out).
        let rp_part = &result.partitions[&TargetFamily::Rp2040];
        let sink_count = rp_part
            .blocks
            .iter()
            .filter(|b| b.block_type == "pubsub_sink")
            .count();
        assert_eq!(sink_count, 2);
    }

    #[test]
    fn intra_partition_channels_preserved() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_block(2, TargetFamily::Rp2040),
                make_sink_block(3, TargetFamily::Rp2040),
            ],
            vec![
                make_channel(1, 1, 0, 2, 0),
                make_channel(2, 2, 0, 3, 0),
            ],
        );

        let result = partition_graph(&snap).unwrap();
        let rp_part = &result.partitions[&TargetFamily::Rp2040];
        assert_eq!(rp_part.channels.len(), 2);
        // Original channel IDs preserved.
        assert!(rp_part.channels.iter().any(|c| c.id == ChannelId(1)));
        assert!(rp_part.channels.iter().any(|c| c.id == ChannelId(2)));
        assert!(result.bridges.is_empty());
    }

    #[test]
    fn channel_ids_correctly_assigned_in_subgraphs() {
        let snap = make_graph(
            vec![
                make_source_block(1, TargetFamily::Rp2040),
                make_sink_block(2, TargetFamily::Stm32f4),
            ],
            vec![make_channel(10, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap).unwrap();

        // Sender partition: should have a new channel wiring source -> pubsub_sink.
        let rp_part = &result.partitions[&TargetFamily::Rp2040];
        assert_eq!(rp_part.channels.len(), 1);
        let ch = &rp_part.channels[0];
        assert_eq!(ch.from_block, BlockId(1));
        assert_eq!(ch.from_port, 0);
        // The to_block should be the pubsub_sink bridge block.
        let sink_block = rp_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_sink")
            .unwrap();
        assert_eq!(ch.to_block, BlockId(sink_block.id));
        assert_eq!(ch.to_port, 0);

        // Receiver partition: should have a new channel wiring pubsub_source -> sink.
        let stm_part = &result.partitions[&TargetFamily::Stm32f4];
        assert_eq!(stm_part.channels.len(), 1);
        let ch = &stm_part.channels[0];
        let source_block = stm_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_source")
            .unwrap();
        assert_eq!(ch.from_block, BlockId(source_block.id));
        assert_eq!(ch.from_port, 0);
        assert_eq!(ch.to_block, BlockId(2));
        assert_eq!(ch.to_port, 0);
    }

    #[test]
    fn bridge_block_ids_start_from_max_plus_1000() {
        let snap = make_graph(
            vec![
                make_source_block(5, TargetFamily::Rp2040),
                make_sink_block(10, TargetFamily::Stm32f4),
            ],
            vec![make_channel(1, 5, 0, 10, 0)],
        );

        let result = partition_graph(&snap).unwrap();

        // max block id = 10, so bridges start at 1010.
        let all_bridge_ids: Vec<u32> = result
            .partitions
            .values()
            .flat_map(|p| p.blocks.iter())
            .filter(|b| b.block_type == "pubsub_sink" || b.block_type == "pubsub_source")
            .map(|b| b.id)
            .collect();

        assert_eq!(all_bridge_ids.len(), 2);
        for id in &all_bridge_ids {
            assert!(*id >= 1010, "bridge id {} should be >= 1010", id);
        }
        // First bridge block should be exactly 1010.
        assert!(all_bridge_ids.contains(&1010));
        assert!(all_bridge_ids.contains(&1011));
    }
}
