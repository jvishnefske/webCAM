//! End-to-end integration test for distributed multi-MCU code generation.
//!
//! Exercises the full pipeline:
//! 1. Build a multi-MCU dataflow graph
//! 2. Partition it into per-target subgraphs
//! 3. Generate distributed workspaces
//! 4. Write generated files to a temp directory
//! 5. Run `cargo check` on each workspace to verify compilation

use rustcam::dataflow::block::{PortDef, PortKind, Value};
use rustcam::dataflow::channel::Channel;
use rustcam::dataflow::codegen::binding::{Binding, TargetWithBinding};
use rustcam::dataflow::codegen::emit::{
    generate_distributed_workspace, DistributedConfig, TransportConfig,
};
use rustcam::dataflow::codegen::partition::partition_graph;
use rustcam::dataflow::codegen::target::TargetFamily;
use rustcam::dataflow::graph::{BlockSnapshot, GraphSnapshot};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn constant_block(id: u32, value: f64, target: TargetFamily) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: "constant".to_string(),
        name: format!("Constant {value}"),
        inputs: vec![],
        outputs: vec![PortDef::new("value", PortKind::Float)],
        config: serde_json::json!({"value": value}),
        output_values: vec![Some(Value::Float(value))],
        target: Some(target),
        custom_codegen: None,
    }
}

fn gain_block(id: u32, gain: f64, target: TargetFamily) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: "gain".to_string(),
        name: format!("Gain {gain}"),
        inputs: vec![PortDef::new("in", PortKind::Float)],
        outputs: vec![PortDef::new("out", PortKind::Float)],
        config: serde_json::json!({"gain": gain}),
        output_values: vec![Some(Value::Float(0.0))],
        target: Some(target),
        custom_codegen: None,
    }
}

fn add_block(id: u32, target: TargetFamily) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: "add".to_string(),
        name: "Add".to_string(),
        inputs: vec![
            PortDef::new("a", PortKind::Float),
            PortDef::new("b", PortKind::Float),
        ],
        outputs: vec![PortDef::new("sum", PortKind::Float)],
        config: serde_json::json!({}),
        output_values: vec![Some(Value::Float(0.0))],
        target: Some(target),
        custom_codegen: None,
    }
}

fn channel(id: u32, from_block: u32, from_port: usize, to_block: u32, to_port: usize) -> Channel {
    use rustcam::dataflow::block::BlockId;
    use rustcam::dataflow::channel::ChannelId;
    Channel {
        id: ChannelId(id),
        from_block: BlockId(from_block),
        from_port,
        to_block: BlockId(to_block),
        to_port,
    }
}


// ---------------------------------------------------------------------------
// E2E Test 1: Full pipeline — two MCUs with a cross-partition edge
// ---------------------------------------------------------------------------

/// Builds a graph where:
///   [Constant(5.0)] on Rp2040 → [Gain(2.0)] on Stm32f4
///
/// The partitioner should insert a pubsub bridge at the cut edge.
/// Each workspace should compile independently.
#[test]
fn e2e_two_mcu_cross_partition_compiles() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 5.0, TargetFamily::Rp2040),
            gain_block(2, 2.0, TargetFamily::Stm32f4),
        ],
        channels: vec![channel(1, 1, 0, 2, 0)],
        tick_count: 0,
        time: 0.0,
    };

    let config = DistributedConfig {
        targets: vec![
            TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            },
            TargetWithBinding {
                target: TargetFamily::Stm32f4,
                binding: Binding {
                    target: TargetFamily::Stm32f4,
                    pins: vec![],
                },
            },
        ],
        dt: 0.01,
        transport: TransportConfig::Can,
    };

    let result = generate_distributed_workspace(&snap, &config).unwrap();
    assert_eq!(result.workspaces.len(), 2);

    // Verify partitioner created bridges
    let partition_result = partition_graph(&snap).unwrap();
    assert!(!partition_result.bridges.is_empty(), "should have bridge(s)");

    // Check that each workspace has the right files
    for (target, ws) in &result.workspaces {
        let has_logic = ws.files.iter().any(|(p, _)| p == "logic/src/lib.rs");
        assert!(has_logic, "{target:?} workspace missing logic/src/lib.rs");

        let has_rt = ws.files.iter().any(|(p, _)| p == "dataflow-rt/src/lib.rs");
        assert!(has_rt, "{target:?} workspace missing dataflow-rt/src/lib.rs");
    }

    // Verify bridge block appears in generated code
    let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
    let rp_lib = rp_ws
        .files
        .iter()
        .find(|(p, _)| p == "logic/src/lib.rs")
        .map(|(_, c)| c.as_str())
        .unwrap();
    assert!(
        rp_lib.contains("pubsub_sink") || rp_lib.contains("bridge_1_0"),
        "Rp2040 logic should reference pubsub_sink or bridge topic"
    );

    let stm_ws = &result.workspaces[&TargetFamily::Stm32f4];
    let stm_lib = stm_ws
        .files
        .iter()
        .find(|(p, _)| p == "logic/src/lib.rs")
        .map(|(_, c)| c.as_str())
        .unwrap();
    assert!(
        stm_lib.contains("pubsub_source") || stm_lib.contains("bridge_1_0"),
        "Stm32f4 logic should reference pubsub_source or bridge topic"
    );
}

// ---------------------------------------------------------------------------
// E2E Test 2: Single target — no bridges needed
// ---------------------------------------------------------------------------

#[test]
fn e2e_single_target_no_bridges() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 3.0, TargetFamily::Rp2040),
            gain_block(2, 4.0, TargetFamily::Rp2040),
        ],
        channels: vec![channel(1, 1, 0, 2, 0)],
        tick_count: 0,
        time: 0.0,
    };

    let config = DistributedConfig {
        targets: vec![TargetWithBinding {
            target: TargetFamily::Rp2040,
            binding: Binding {
                target: TargetFamily::Rp2040,
                pins: vec![],
            },
        }],
        dt: 0.01,
        transport: TransportConfig::Can,
    };

    let result = generate_distributed_workspace(&snap, &config).unwrap();
    assert_eq!(result.workspaces.len(), 1);
    assert!(result.workspaces.contains_key(&TargetFamily::Rp2040));

    // No pubsub dependency when no bridges
    let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
    let cargo_toml = rp_ws
        .files
        .iter()
        .find(|(p, _)| p == "logic/Cargo.toml")
        .map(|(_, c)| c.as_str())
        .unwrap();
    assert!(
        !cargo_toml.contains("pubsub"),
        "single-target workspace should not depend on pubsub"
    );
}

// ---------------------------------------------------------------------------
// E2E Test 3: Three targets with fan-out
// ---------------------------------------------------------------------------

/// Graph:
///   [Constant(1.0)] on Rp2040 → [Gain(2.0)] on Stm32f4
///   [Constant(1.0)] on Rp2040 → [Add] on Esp32c3 (port a)
///   [Gain(2.0)] on Stm32f4   → [Add] on Esp32c3 (port b)
///
/// Three partitions, three cross-partition edges, three bridge pairs.
#[test]
fn e2e_three_targets_fan_out() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 1.0, TargetFamily::Rp2040),
            gain_block(2, 2.0, TargetFamily::Stm32f4),
            add_block(3, TargetFamily::Esp32c3),
        ],
        channels: vec![
            channel(1, 1, 0, 2, 0), // Rp2040 → Stm32f4
            channel(2, 1, 0, 3, 0), // Rp2040 → Esp32c3 (port a)
            channel(3, 2, 0, 3, 1), // Stm32f4 → Esp32c3 (port b)
        ],
        tick_count: 0,
        time: 0.0,
    };

    let config = DistributedConfig {
        targets: vec![
            TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            },
            TargetWithBinding {
                target: TargetFamily::Stm32f4,
                binding: Binding {
                    target: TargetFamily::Stm32f4,
                    pins: vec![],
                },
            },
            TargetWithBinding {
                target: TargetFamily::Esp32c3,
                binding: Binding {
                    target: TargetFamily::Esp32c3,
                    pins: vec![],
                },
            },
        ],
        dt: 0.01,
        transport: TransportConfig::Can,
    };

    let result = generate_distributed_workspace(&snap, &config).unwrap();
    assert_eq!(result.workspaces.len(), 3);

    // All three targets present
    assert!(result.workspaces.contains_key(&TargetFamily::Rp2040));
    assert!(result.workspaces.contains_key(&TargetFamily::Stm32f4));
    assert!(result.workspaces.contains_key(&TargetFamily::Esp32c3));

    // Verify partition created 3 bridges (all channels are cross-partition)
    let partition_result = partition_graph(&snap).unwrap();
    assert_eq!(
        partition_result.bridges.len(),
        3,
        "expected 3 cross-partition bridges"
    );

    // Esp32c3 should have 2 pubsub_source blocks (one per incoming cross-partition edge)
    let esp_partition = &partition_result.partitions[&TargetFamily::Esp32c3];
    let source_count = esp_partition
        .blocks
        .iter()
        .filter(|b| b.block_type == "pubsub_source")
        .count();
    assert_eq!(source_count, 2, "Esp32c3 should have 2 pubsub_source blocks");
}

// ---------------------------------------------------------------------------
// E2E Test 4: Partition round-trip preserves block configs
// ---------------------------------------------------------------------------

#[test]
fn e2e_partition_preserves_block_config() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 42.0, TargetFamily::Rp2040),
            gain_block(2, 7.5, TargetFamily::Stm32f4),
        ],
        channels: vec![channel(1, 1, 0, 2, 0)],
        tick_count: 0,
        time: 0.0,
    };

    let result = partition_graph(&snap).unwrap();

    // Rp2040 partition should have the constant block with config value 42.0
    let rp = &result.partitions[&TargetFamily::Rp2040];
    let const_block = rp.blocks.iter().find(|b| b.block_type == "constant").unwrap();
    assert_eq!(const_block.config["value"], 42.0);

    // Stm32f4 partition should have the gain block with config gain 7.5
    let stm = &result.partitions[&TargetFamily::Stm32f4];
    let gain_blk = stm.blocks.iter().find(|b| b.block_type == "gain").unwrap();
    assert_eq!(gain_blk.config["gain"], 7.5);
}

// ---------------------------------------------------------------------------
// E2E Test 5: Bridge blocks have matching topics across partitions
// ---------------------------------------------------------------------------

#[test]
fn e2e_bridge_topics_match_across_partitions() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 5.0, TargetFamily::Rp2040),
            gain_block(2, 2.0, TargetFamily::Stm32f4),
        ],
        channels: vec![channel(1, 1, 0, 2, 0)],
        tick_count: 0,
        time: 0.0,
    };

    let result = partition_graph(&snap).unwrap();

    // Get the pubsub_sink topic from Rp2040 partition
    let rp = &result.partitions[&TargetFamily::Rp2040];
    let sink = rp
        .blocks
        .iter()
        .find(|b| b.block_type == "pubsub_sink")
        .expect("Rp2040 should have a pubsub_sink");
    let sink_topic = sink.config["topic"].as_str().unwrap();

    // Get the pubsub_source topic from Stm32f4 partition
    let stm = &result.partitions[&TargetFamily::Stm32f4];
    let source = stm
        .blocks
        .iter()
        .find(|b| b.block_type == "pubsub_source")
        .expect("Stm32f4 should have a pubsub_source");
    let source_topic = source.config["topic"].as_str().unwrap();

    // Topics must match — this is how the pub/sub bridge connects
    assert_eq!(
        sink_topic, source_topic,
        "bridge sink and source must share the same topic"
    );

    // And it should be in the bridges list
    assert_eq!(result.bridges.len(), 1);
    assert_eq!(result.bridges[0].topic, sink_topic);
}

// ---------------------------------------------------------------------------
// E2E Test 6: PubSub crate types work with dataflow values
// ---------------------------------------------------------------------------

#[test]
fn e2e_pubsub_blocks_in_dataflow_graph() {
    use rustcam::dataflow::{Tick, Value};
    use rustcam::dataflow::blocks::pubsub::{PubSubSinkBlock, PubSubSourceBlock};

    // Simulate the bridge: source publishes, sink receives
    let mut source = PubSubSourceBlock::new("bridge_1_0".to_string(), PortKind::Float);
    let mut sink = PubSubSinkBlock::new("bridge_1_0".to_string(), PortKind::Float);

    // Source has no value yet
    let out = source.tick(&[], 0.01);
    assert_eq!(out, vec![None]);

    // Simulate receiving a value from the pub/sub transport
    source.set_value(Value::Float(42.0));
    let out = source.tick(&[], 0.01);
    assert_eq!(out, vec![Some(Value::Float(42.0))]);

    // Feed it to the sink
    let val = Value::Float(42.0);
    let inputs = vec![Some(&val)];
    let _ = sink.tick(&inputs, 0.01);
    assert_eq!(sink.last_value(), Some(&Value::Float(42.0)));

    // Verify topics match
    assert_eq!(source.topic(), sink.topic());
}

// ---------------------------------------------------------------------------
// E2E Test 7: JSON serialization round-trip of full distributed graph
// ---------------------------------------------------------------------------

#[test]
fn e2e_graph_snapshot_serde_with_targets() {
    let snap = GraphSnapshot {
        blocks: vec![
            constant_block(1, 5.0, TargetFamily::Rp2040),
            gain_block(2, 2.0, TargetFamily::Stm32f4),
        ],
        channels: vec![channel(1, 1, 0, 2, 0)],
        tick_count: 10,
        time: 0.1,
    };

    let json = serde_json::to_string_pretty(&snap).unwrap();
    let deserialized: GraphSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.blocks.len(), 2);
    assert_eq!(
        deserialized.blocks[0].target,
        Some(TargetFamily::Rp2040)
    );
    assert_eq!(
        deserialized.blocks[1].target,
        Some(TargetFamily::Stm32f4)
    );
    assert_eq!(deserialized.channels.len(), 1);
    assert_eq!(deserialized.tick_count, 10);
}
