//! Graph partitioner for distributed multi-target codegen.
//!
//! Takes a [`GraphSnapshot`] with an external assignments map and splits it
//! into one subgraph per target, inserting pub/sub bridge blocks at
//! cross-partition channel boundaries.

use std::collections::HashMap;

use graph_model::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot};
use module_traits::value::{PortDef, PortKind};
use serde_json::json;

/// Errors that can occur during graph partitioning.
#[derive(Debug)]
pub enum PartitionError {
    /// A block has no target assignment in the assignments map.
    UnassignedBlock(u32),
    /// The graph has no blocks.
    EmptyGraph,
}

/// Configuration for a pub/sub bridge connection.
#[derive(Debug, Clone)]
pub struct BridgeInfo {
    pub topic: String,
    pub port_kind: PortKind,
    pub source_target: String,
    pub sink_target: String,
}

/// Result of partitioning a graph.
#[derive(Debug)]
pub struct PartitionResult {
    /// Per-target subgraphs.
    pub partitions: HashMap<String, GraphSnapshot>,
    /// Bridge connections that were created.
    pub bridges: Vec<BridgeInfo>,
}

/// Partition a graph snapshot into per-target subgraphs.
///
/// Every block in the snapshot must have an entry in the `assignments` map.
/// Cross-partition channels are replaced with pub/sub bridge block pairs
/// sharing a deterministic topic name.
///
/// # Arguments
///
/// * `snap` — the full graph snapshot
/// * `assignments` — maps each `BlockId` to a target identifier string
///
/// # Errors
///
/// Returns [`PartitionError::EmptyGraph`] if the graph has no blocks, or
/// [`PartitionError::UnassignedBlock`] if any block lacks an assignment.
pub fn partition_graph(
    snap: &GraphSnapshot,
    assignments: &HashMap<BlockId, String>,
) -> Result<PartitionResult, PartitionError> {
    if snap.blocks.is_empty() {
        return Err(PartitionError::EmptyGraph);
    }

    // Validate all blocks are assigned and build lookup.
    let mut block_target: HashMap<u32, String> = HashMap::new();
    for b in &snap.blocks {
        match assignments.get(&b.id) {
            Some(t) => {
                block_target.insert(b.id.0, t.clone());
            }
            None => return Err(PartitionError::UnassignedBlock(b.id.0)),
        }
    }

    // Group blocks by target.
    let mut target_blocks: HashMap<String, Vec<BlockSnapshot>> = HashMap::new();
    for b in &snap.blocks {
        let t = &block_target[&b.id.0];
        target_blocks
            .entry(t.clone())
            .or_default()
            .push(BlockSnapshot {
                id: b.id,
                block_type: b.block_type.clone(),
                name: b.name.clone(),
                inputs: b.inputs.clone(),
                outputs: b.outputs.clone(),
                config: b.config.clone(),
                is_delay: b.is_delay,
            });
    }

    // Determine max block ID for bridge ID allocation.
    let max_id = snap.blocks.iter().map(|b| b.id.0).max().unwrap_or(0);
    let mut next_bridge_id = max_id + 1000;

    let mut target_channels: HashMap<String, Vec<Channel>> = HashMap::new();
    let mut next_channel_id = snap.channels.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
    let mut bridges: Vec<BridgeInfo> = Vec::new();

    for ch in &snap.channels {
        let from_target = &block_target[&ch.from_block.0];
        let to_target = &block_target[&ch.to_block.0];

        if from_target == to_target {
            // Intra-partition: keep as-is.
            target_channels
                .entry(from_target.clone())
                .or_default()
                .push(ch.clone());
        } else {
            // Cross-partition: insert bridge pair.
            let topic = format!("bridge_{}_{}", ch.from_block.0, ch.from_port);

            // Determine port kind from the source block's output port.
            let port_kind = snap
                .blocks
                .iter()
                .find(|b| b.id == ch.from_block)
                .and_then(|source_block| {
                    source_block
                        .outputs
                        .get(ch.from_port)
                        .map(|p| p.kind.clone())
                })
                .unwrap_or(PortKind::Any);

            // Create pubsub_sink in sender's partition.
            let sink_id = next_bridge_id;
            next_bridge_id += 1;
            let sink_block = BlockSnapshot {
                id: BlockId(sink_id),
                block_type: "pubsub_sink".to_string(),
                name: format!("PubSub Sink ({})", topic),
                inputs: vec![PortDef::new("in", port_kind.clone())],
                outputs: vec![],
                config: json!({"topic": topic}),
                is_delay: false,
            };
            target_blocks
                .entry(from_target.clone())
                .or_default()
                .push(sink_block);

            // Wire: original output -> pubsub_sink input.
            let sink_channel = Channel {
                id: ChannelId(next_channel_id),
                from_block: ch.from_block,
                from_port: ch.from_port,
                to_block: BlockId(sink_id),
                to_port: 0,
            };
            next_channel_id += 1;
            target_channels
                .entry(from_target.clone())
                .or_default()
                .push(sink_channel);

            // Create pubsub_source in receiver's partition.
            let source_id = next_bridge_id;
            next_bridge_id += 1;
            let source_block = BlockSnapshot {
                id: BlockId(source_id),
                block_type: "pubsub_source".to_string(),
                name: format!("PubSub Source ({})", topic),
                inputs: vec![],
                outputs: vec![PortDef::new("out", port_kind.clone())],
                config: json!({"topic": topic}),
                is_delay: false,
            };
            target_blocks
                .entry(to_target.clone())
                .or_default()
                .push(source_block);

            // Wire: pubsub_source output -> original input.
            let source_channel = Channel {
                id: ChannelId(next_channel_id),
                from_block: BlockId(source_id),
                from_port: 0,
                to_block: ch.to_block,
                to_port: ch.to_port,
            };
            next_channel_id += 1;
            target_channels
                .entry(to_target.clone())
                .or_default()
                .push(source_channel);

            bridges.push(BridgeInfo {
                topic,
                port_kind,
                source_target: from_target.clone(),
                sink_target: to_target.clone(),
            });
        }
    }

    // Assemble partitions.
    let mut partitions: HashMap<String, GraphSnapshot> = HashMap::new();
    for (target, blocks) in target_blocks {
        let channels = target_channels.remove(&target).unwrap_or_default();
        partitions.insert(target, GraphSnapshot { blocks, channels });
    }

    Ok(PartitionResult {
        partitions,
        bridges,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use graph_model::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot};
    use module_traits::value::{PortDef, PortKind};

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    fn make_block(id: u32, target: &str) -> (BlockSnapshot, (BlockId, String)) {
        let snap = BlockSnapshot {
            id: BlockId(id),
            block_type: "constant".to_string(),
            name: format!("Block {}", id),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({}),
            is_delay: false,
        };
        (snap, (BlockId(id), target.to_string()))
    }

    fn make_source_block(id: u32, target: &str) -> (BlockSnapshot, (BlockId, String)) {
        let snap = BlockSnapshot {
            id: BlockId(id),
            block_type: "constant".to_string(),
            name: format!("Source {}", id),
            inputs: vec![],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({"value": 1.0}),
            is_delay: false,
        };
        (snap, (BlockId(id), target.to_string()))
    }

    fn make_sink_block(id: u32, target: &str) -> (BlockSnapshot, (BlockId, String)) {
        let snap = BlockSnapshot {
            id: BlockId(id),
            block_type: "plot".to_string(),
            name: format!("Sink {}", id),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![],
            config: serde_json::json!({}),
            is_delay: false,
        };
        (snap, (BlockId(id), target.to_string()))
    }

    fn make_channel(
        id: u32,
        from_block: u32,
        from_port: usize,
        to_block: u32,
        to_port: usize,
    ) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from_block),
            from_port,
            to_block: BlockId(to_block),
            to_port,
        }
    }

    fn make_graph(
        block_pairs: Vec<(BlockSnapshot, (BlockId, String))>,
        channels: Vec<Channel>,
    ) -> (GraphSnapshot, HashMap<BlockId, String>) {
        let mut assignments = HashMap::new();
        let mut blocks = Vec::new();
        for (snap, (bid, target)) in block_pairs {
            blocks.push(snap);
            assignments.insert(bid, target);
        }
        (GraphSnapshot { blocks, channels }, assignments)
    }

    // ---------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------

    #[test]
    fn empty_graph_returns_error() {
        let (snap, assignments) = make_graph(vec![], vec![]);
        let result = partition_graph(&snap, &assignments);
        assert!(matches!(result, Err(PartitionError::EmptyGraph)));
    }

    #[test]
    fn unassigned_block_returns_error() {
        let (block, _) = make_block(1, "rp2040");
        let snap = GraphSnapshot {
            blocks: vec![block],
            channels: vec![],
        };
        // Empty assignments — block 1 is unassigned.
        let assignments = HashMap::new();
        let result = partition_graph(&snap, &assignments);
        assert!(matches!(result, Err(PartitionError::UnassignedBlock(1))));
    }

    #[test]
    fn single_target_no_bridges() {
        let (snap, assignments) = make_graph(
            vec![make_source_block(1, "rp2040"), make_block(2, "rp2040")],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.partitions.len(), 1);
        assert!(result.partitions.contains_key("rp2040"));
        assert!(result.bridges.is_empty());

        let part = &result.partitions["rp2040"];
        assert_eq!(part.blocks.len(), 2);
        assert_eq!(part.channels.len(), 1);
        // Original channel preserved.
        assert_eq!(part.channels[0].id, ChannelId(1));
    }

    #[test]
    fn two_targets_no_cross_partition_channels() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_source_block(2, "stm32f4"),
            ],
            vec![],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.partitions.len(), 2);
        assert!(result.partitions.contains_key("rp2040"));
        assert!(result.partitions.contains_key("stm32f4"));
        assert!(result.bridges.is_empty());

        assert_eq!(result.partitions["rp2040"].blocks.len(), 1);
        assert_eq!(result.partitions["stm32f4"].blocks.len(), 1);
    }

    #[test]
    fn two_targets_one_cross_partition_channel() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_sink_block(2, "stm32f4"),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.partitions.len(), 2);
        assert_eq!(result.bridges.len(), 1);

        let bridge = &result.bridges[0];
        assert_eq!(bridge.topic, "bridge_1_0");
        assert_eq!(bridge.source_target, "rp2040");
        assert_eq!(bridge.sink_target, "stm32f4");
        assert_eq!(bridge.port_kind, PortKind::Float);
    }

    #[test]
    fn cross_partition_creates_pubsub_sink_in_sender_partition() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_sink_block(2, "stm32f4"),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        let rp_part = &result.partitions["rp2040"];

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
    }

    #[test]
    fn cross_partition_creates_pubsub_source_in_receiver_partition() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_sink_block(2, "stm32f4"),
            ],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        let stm_part = &result.partitions["stm32f4"];

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
    }

    #[test]
    fn bridge_blocks_have_correct_topic_names() {
        // Block 5, output port 2 -> topic should be "bridge_5_2"
        let (mut src, src_assign) = make_source_block(5, "host");
        // Give it 3 output ports so port index 2 is valid.
        src.outputs = vec![
            PortDef::new("out0", PortKind::Float),
            PortDef::new("out1", PortKind::Float),
            PortDef::new("out2", PortKind::Text),
        ];

        let (snap, assignments) = make_graph(
            vec![(src, src_assign), make_sink_block(6, "esp32c3")],
            vec![make_channel(1, 5, 2, 6, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.bridges[0].topic, "bridge_5_2");
    }

    #[test]
    fn bridge_blocks_have_correct_port_kinds() {
        // Source outputs Text on port 0.
        let (mut src, src_assign) = make_source_block(1, "rp2040");
        src.outputs = vec![PortDef::new("out", PortKind::Text)];

        let (mut sink, sink_assign) = make_sink_block(2, "stm32f4");
        sink.inputs = vec![PortDef::new("in", PortKind::Text)];

        let (snap, assignments) = make_graph(
            vec![(src, src_assign), (sink, sink_assign)],
            vec![make_channel(1, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.bridges[0].port_kind, PortKind::Text);

        // Verify sink block port kind matches.
        let rp_part = &result.partitions["rp2040"];
        let pubsub_sink = rp_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_sink")
            .unwrap();
        assert_eq!(pubsub_sink.inputs[0].kind, PortKind::Text);

        // Verify source block port kind matches.
        let stm_part = &result.partitions["stm32f4"];
        let pubsub_source = stm_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_source")
            .unwrap();
        assert_eq!(pubsub_source.outputs[0].kind, PortKind::Text);
    }

    #[test]
    fn multiple_cross_partition_channels_create_multiple_bridges() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_block(2, "rp2040"),
                make_sink_block(3, "stm32f4"),
                make_sink_block(4, "stm32f4"),
            ],
            vec![
                make_channel(1, 1, 0, 2, 0), // intra-partition
                make_channel(2, 1, 0, 3, 0), // cross-partition
                make_channel(3, 2, 0, 4, 0), // cross-partition
            ],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Each bridge should have a unique topic.
        let topics: Vec<&str> = result.bridges.iter().map(|b| b.topic.as_str()).collect();
        assert!(topics.contains(&"bridge_1_0"));
        assert!(topics.contains(&"bridge_2_0"));
    }

    #[test]
    fn three_target_graph() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_block(2, "stm32f4"),
                make_sink_block(3, "esp32c3"),
            ],
            vec![make_channel(1, 1, 0, 2, 0), make_channel(2, 2, 0, 3, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.partitions.len(), 3);
        assert!(result.partitions.contains_key("rp2040"));
        assert!(result.partitions.contains_key("stm32f4"));
        assert!(result.partitions.contains_key("esp32c3"));
        assert_eq!(result.bridges.len(), 2);
    }

    #[test]
    fn fan_out_across_partitions() {
        // One source block fans out to two blocks on different targets.
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_sink_block(2, "stm32f4"),
                make_sink_block(3, "esp32c3"),
            ],
            vec![make_channel(1, 1, 0, 2, 0), make_channel(2, 1, 0, 3, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Both bridges originate from rp2040.
        for bridge in &result.bridges {
            assert_eq!(bridge.source_target, "rp2040");
        }

        // Sender partition should have 2 pubsub_sink blocks (one per fan-out).
        let rp_part = &result.partitions["rp2040"];
        let sink_count = rp_part
            .blocks
            .iter()
            .filter(|b| b.block_type == "pubsub_sink")
            .count();
        assert_eq!(sink_count, 2);
    }

    #[test]
    fn intra_partition_channels_preserved() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_block(2, "rp2040"),
                make_sink_block(3, "rp2040"),
            ],
            vec![make_channel(1, 1, 0, 2, 0), make_channel(2, 2, 0, 3, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();
        let rp_part = &result.partitions["rp2040"];
        assert_eq!(rp_part.channels.len(), 2);
        // Original channel IDs preserved.
        assert!(rp_part.channels.iter().any(|c| c.id == ChannelId(1)));
        assert!(rp_part.channels.iter().any(|c| c.id == ChannelId(2)));
        assert!(result.bridges.is_empty());
    }

    #[test]
    fn channel_ids_correctly_assigned_in_subgraphs() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(1, "rp2040"),
                make_sink_block(2, "stm32f4"),
            ],
            vec![make_channel(10, 1, 0, 2, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();

        // Sender partition: should have a new channel wiring source -> pubsub_sink.
        let rp_part = &result.partitions["rp2040"];
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
        assert_eq!(ch.to_block, sink_block.id);
        assert_eq!(ch.to_port, 0);

        // Receiver partition: should have a new channel wiring pubsub_source -> sink.
        let stm_part = &result.partitions["stm32f4"];
        assert_eq!(stm_part.channels.len(), 1);
        let ch = &stm_part.channels[0];
        let source_block = stm_part
            .blocks
            .iter()
            .find(|b| b.block_type == "pubsub_source")
            .unwrap();
        assert_eq!(ch.from_block, source_block.id);
        assert_eq!(ch.from_port, 0);
        assert_eq!(ch.to_block, BlockId(2));
        assert_eq!(ch.to_port, 0);
    }

    #[test]
    fn bridge_block_ids_start_from_max_plus_1000() {
        let (snap, assignments) = make_graph(
            vec![
                make_source_block(5, "rp2040"),
                make_sink_block(10, "stm32f4"),
            ],
            vec![make_channel(1, 5, 0, 10, 0)],
        );

        let result = partition_graph(&snap, &assignments).unwrap();

        // max block id = 10, so bridges start at 1010.
        let all_bridge_ids: Vec<u32> = result
            .partitions
            .values()
            .flat_map(|p| p.blocks.iter())
            .filter(|b| b.block_type == "pubsub_sink" || b.block_type == "pubsub_source")
            .map(|b| b.id.0)
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
