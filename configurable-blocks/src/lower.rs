//! Lower a [`ConfigurableBlock`] to DAG IL.

use dag_core::op::{Dag, DagError, NodeId, Op};
use dag_core::templates::BlockPorts;

use crate::deployment_profile::{ChannelMap, DeploymentProfile};
use crate::schema::{ChannelDirection, ChannelKind, ConfigField, DeclaredChannel};

/// Result of lowering a configurable block into DAG ops.
pub struct LowerResult {
    /// Named input/output ports for wiring to other blocks.
    pub ports: BlockPorts,
    /// The DAG containing the lowered ops (caller may merge into a larger DAG).
    pub dag: Dag,
}

/// Trait implemented by configurable blocks to lower their logic to DAG IL.
///
/// The block reads its current configuration, then emits a sequence of
/// [`dag_core::op::Op`] nodes into a fresh DAG. The returned [`LowerResult`]
/// contains the DAG and named ports for external wiring.
pub trait ConfigurableBlock {
    /// Unique block type identifier (e.g. "pid", "moving_average").
    fn block_type(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Category for sub-menu placement.
    fn category(&self) -> crate::schema::BlockCategory;

    /// Configuration schema — the list of editable fields.
    fn config_schema(&self) -> Vec<ConfigField>;

    /// Current configuration as JSON.
    fn config_json(&self) -> serde_json::Value;

    /// Apply a JSON configuration (partial or full update).
    fn apply_config(&mut self, config: &serde_json::Value);

    /// Declared channels (pubsub topics and hardware I/O names).
    ///
    /// These are derived from the current config — e.g. changing the
    /// "setpoint_topic" field changes the declared input channel.
    fn declared_channels(&self) -> Vec<DeclaredChannel>;

    /// Lower this block to DAG IL.
    ///
    /// Emits ops into a fresh `Dag` and returns named ports. The caller
    /// can merge this DAG into a larger graph or CBOR-encode it directly
    /// for MCU deployment.
    fn lower(&self) -> Result<LowerResult, DagError>;
}

/// Convenience: lower a block and CBOR-encode the resulting DAG.
pub fn lower_and_encode(block: &dyn ConfigurableBlock) -> Result<Vec<u8>, String> {
    let result = block.lower().map_err(|e| format!("{:?}", e))?;
    Ok(dag_core::cbor::encode_dag(&result.dag))
}

/// Lower a block and return a human-readable IL listing of the DAG ops.
pub fn lower_to_il_text(block: &dyn ConfigurableBlock) -> Result<String, String> {
    let result = block.lower().map_err(|e| format!("{:?}", e))?;
    let mut lines = Vec::new();
    for (i, op) in result.dag.nodes().iter().enumerate() {
        lines.push(format!("  %{} = {:?}", i, op));
    }

    let mut text = String::new();
    text.push_str(&format!("block @{} {{\n", block.block_type()));

    // Inputs
    let inputs: Vec<_> = block
        .declared_channels()
        .into_iter()
        .filter(|ch| ch.direction == ChannelDirection::Input)
        .collect();
    if !inputs.is_empty() {
        text.push_str("  // inputs\n");
        for ch in &inputs {
            let kind_label = match ch.kind {
                ChannelKind::PubSub => "pubsub",
                ChannelKind::Hardware => "hw",
            };
            text.push_str(&format!(
                "  //   {} \"{}\" ({})\n",
                kind_label, ch.name, "in"
            ));
        }
    }

    // Outputs
    let outputs: Vec<_> = block
        .declared_channels()
        .into_iter()
        .filter(|ch| ch.direction == ChannelDirection::Output)
        .collect();
    if !outputs.is_empty() {
        text.push_str("  // outputs\n");
        for ch in &outputs {
            let kind_label = match ch.kind {
                ChannelKind::PubSub => "pubsub",
                ChannelKind::Hardware => "hw",
            };
            text.push_str(&format!(
                "  //   {} \"{}\" ({})\n",
                kind_label, ch.name, "out"
            ));
        }
    }

    text.push('\n');
    for line in &lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("}\n");
    Ok(text)
}

// ── Channel remapping ────────────────────────────────────────────────────────

/// Rebuild a DAG with Subscribe/Publish topic names remapped through `channel_map`.
///
/// All other ops are copied unchanged. NodeId references are preserved as-is
/// (no offsetting — this is a pure topic-rename pass).
pub fn remap_dag_channels(dag: &Dag, channel_map: &ChannelMap) -> Result<Dag, DagError> {
    let mut remapped = Dag::new();
    for op in dag.nodes() {
        let new_op = match op {
            Op::Subscribe(topic) => Op::Subscribe(channel_map.remap(topic).to_string()),
            Op::Publish(topic, src) => Op::Publish(channel_map.remap(topic).to_string(), *src),
            other => other.clone(),
        };
        remapped.add_op(new_op)?;
    }
    Ok(remapped)
}

// ── Profile-aware lowering ───────────────────────────────────────────────────

/// Lower a block and remap its channels using the profile's channel map.
pub fn lower_with_profile(
    block: &dyn ConfigurableBlock,
    profile: &DeploymentProfile,
) -> Result<LowerResult, DagError> {
    let result = block.lower()?;
    let remapped_dag = remap_dag_channels(&result.dag, &profile.channel_map)?;
    Ok(LowerResult {
        ports: result.ports,
        dag: remapped_dag,
    })
}

// ── Block set merging ────────────────────────────────────────────────────────

/// Offset all NodeId references in an Op by `offset`.
///
/// Source ops (Const, Input, Subscribe) have no references and are cloned as-is.
fn offset_op(op: &Op, offset: NodeId) -> Op {
    match op {
        Op::Const(v) => Op::Const(*v),
        Op::Input(name) => Op::Input(name.clone()),
        Op::Output(name, src) => Op::Output(name.clone(), src + offset),
        Op::Add(a, b) => Op::Add(a + offset, b + offset),
        Op::Mul(a, b) => Op::Mul(a + offset, b + offset),
        Op::Sub(a, b) => Op::Sub(a + offset, b + offset),
        Op::Div(a, b) => Op::Div(a + offset, b + offset),
        Op::Pow(a, b) => Op::Pow(a + offset, b + offset),
        Op::Neg(a) => Op::Neg(a + offset),
        Op::Relu(a) => Op::Relu(a + offset),
        Op::Subscribe(topic) => Op::Subscribe(topic.clone()),
        Op::Publish(topic, src) => Op::Publish(topic.clone(), src + offset),
    }
}

/// Lower a set of `(block_type, config_json)` pairs and merge them into a single DAG.
///
/// For each block:
/// 1. Creates the block instance via the registry.
/// 2. Applies the provided JSON config.
/// 3. Lowers with `lower_with_profile` (channel remapping applied).
/// 4. Appends ops to the combined DAG, adjusting all NodeId references by the
///    current offset (number of nodes already in the combined DAG).
///
/// Returns `Err(String)` if any block type is unknown, lowering fails, or the
/// combined DAG would exceed the NodeId address space.
pub fn lower_block_set(
    blocks: &[(String, serde_json::Value)],
    profile: &DeploymentProfile,
) -> Result<Dag, String> {
    let mut combined = Dag::new();

    for (block_type, config) in blocks {
        let mut block = crate::registry::create_block(block_type)
            .ok_or_else(|| format!("unknown block type: {block_type}"))?;

        block.apply_config(config);

        let result = lower_with_profile(block.as_ref(), profile)
            .map_err(|e| format!("lower failed for {block_type}: {e}"))?;

        let offset = combined.len() as NodeId;

        for op in result.dag.nodes() {
            let adjusted = offset_op(op, offset);
            combined
                .add_op(adjusted)
                .map_err(|e| format!("merge failed for {block_type}: {e}"))?;
        }
    }

    Ok(combined)
}

/// Lower a block set into per-node DAGs, partitioned by the profile's
/// `node_assignments`.
///
/// Blocks that have no node assignment are placed in a `"_default"` DAG.
/// Each node gets its own independent DAG with ops offset from zero.
pub fn lower_block_set_per_node(
    blocks: &[(String, serde_json::Value)],
    profile: &DeploymentProfile,
) -> Result<std::collections::HashMap<String, Dag>, String> {
    use std::collections::HashMap;

    let mut node_dags: HashMap<String, Dag> = HashMap::new();

    for (block_idx, (block_type, config)) in blocks.iter().enumerate() {
        let mut block = crate::registry::create_block(block_type)
            .ok_or_else(|| format!("unknown block type: {block_type}"))?;
        block.apply_config(config);

        let result = lower_with_profile(block.as_ref(), profile)
            .map_err(|e| format!("lower failed for {block_type}: {e}"))?;

        let node_id = profile
            .node_assignments
            .get(&(block_idx as u32))
            .cloned()
            .unwrap_or_else(|| "_default".to_string());

        let dag = node_dags.entry(node_id).or_default();
        let offset = dag.len() as NodeId;

        for op in result.dag.nodes() {
            let adjusted = offset_op(op, offset);
            dag.add_op(adjusted)
                .map_err(|e| format!("merge failed for {block_type}: {e}"))?;
        }
    }

    Ok(node_dags)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lower_to_il_text() {
        let pid = crate::blocks::pid::PidBlock::default();
        let text = lower_to_il_text(&pid).expect("lower failed");
        assert!(text.contains("block @pid"));
        assert!(text.contains("Subscribe"));
        assert!(text.contains("Publish"));
    }

    #[test]
    fn test_lower_and_encode() {
        let pid = crate::blocks::pid::PidBlock::default();
        let bytes = lower_and_encode(&pid).expect("encode failed");
        // Should produce valid CBOR
        let decoded = dag_core::cbor::decode_dag(&bytes).expect("decode failed");
        assert!(!decoded.is_empty());
    }

    // ── remap_dag_channels ───────────────────────────────────────────────────

    #[test]
    fn test_remap_dag_channels_remaps_subscribe_and_publish() {
        let mut dag = Dag::new();
        dag.add_op(Op::Subscribe("motor/setpoint".into())).unwrap(); // node 0
        dag.add_op(Op::Const(1.0)).unwrap(); // node 1
        dag.add_op(Op::Publish("motor/output".into(), 1)).unwrap(); // node 2

        let mut map = ChannelMap::new();
        map.insert("motor/setpoint".into(), "robot/joint1/setpoint".into());
        map.insert("motor/output".into(), "robot/joint1/output".into());

        let remapped = remap_dag_channels(&dag, &map).expect("remap failed");
        assert_eq!(remapped.len(), 3);

        match &remapped.nodes()[0] {
            Op::Subscribe(t) => assert_eq!(t, "robot/joint1/setpoint"),
            other => panic!("expected Subscribe, got {:?}", other),
        }
        match &remapped.nodes()[2] {
            Op::Publish(t, src) => {
                assert_eq!(t, "robot/joint1/output");
                assert_eq!(*src, 1);
            }
            other => panic!("expected Publish, got {:?}", other),
        }
    }

    #[test]
    fn test_remap_dag_channels_leaves_non_pubsub_ops_unchanged() {
        let mut dag = Dag::new();
        dag.add_op(Op::Const(42.0)).unwrap(); // node 0
        dag.add_op(Op::Const(10.0)).unwrap(); // node 1
        dag.add_op(Op::Add(0, 1)).unwrap(); // node 2
        dag.add_op(Op::Mul(0, 2)).unwrap(); // node 3
        dag.add_op(Op::Sub(3, 1)).unwrap(); // node 4
        dag.add_op(Op::Div(4, 1)).unwrap(); // node 5
        dag.add_op(Op::Neg(5)).unwrap(); // node 6
        dag.add_op(Op::Relu(6)).unwrap(); // node 7
        dag.add_op(Op::Input("sensor".into())).unwrap(); // node 8
        dag.add_op(Op::Output("out".into(), 7)).unwrap(); // node 9

        let map = ChannelMap::new(); // empty — no remappings
        let remapped = remap_dag_channels(&dag, &map).expect("remap failed");

        assert_eq!(remapped.len(), dag.len());
        // All ops should be identical since map is empty
        for (orig, new) in dag.nodes().iter().zip(remapped.nodes().iter()) {
            assert_eq!(orig, new);
        }
    }

    // ── lower_with_profile ───────────────────────────────────────────────────

    #[test]
    fn test_lower_with_profile_applies_channel_remapping() {
        let pid = crate::blocks::pid::PidBlock::default();

        // Find what topics PID uses by lowering without a profile first
        let base = pid.lower().expect("lower failed");
        let base_topics: Vec<String> = base
            .dag
            .nodes()
            .iter()
            .filter_map(|op| match op {
                Op::Subscribe(t) => Some(t.clone()),
                Op::Publish(t, _) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(!base_topics.is_empty(), "PID should have pub/sub topics");

        // Build a profile that remaps the first subscribe topic
        let first_sub = base
            .dag
            .nodes()
            .iter()
            .find_map(|op| {
                if let Op::Subscribe(t) = op {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .expect("no Subscribe in PID DAG");

        let mut profile = DeploymentProfile::new("test");
        profile
            .channel_map
            .insert(first_sub.clone(), "remapped/topic".into());

        let result = lower_with_profile(&pid, &profile).expect("lower_with_profile failed");

        // The remapped DAG should have the new topic name
        let has_remapped = result
            .dag
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Subscribe(t) if t == "remapped/topic"));
        assert!(
            has_remapped,
            "expected remapped Subscribe topic in output DAG"
        );

        // The original topic name should no longer appear
        let has_original = result
            .dag
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Subscribe(t) if *t == first_sub));
        assert!(
            !has_original,
            "original topic should have been remapped away"
        );
    }

    // ── lower_block_set ──────────────────────────────────────────────────────

    #[test]
    fn test_lower_block_set_merges_multiple_blocks() {
        let profile = DeploymentProfile::new("bench");

        // Two blocks: a gain block and a pid block
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("gain".into(), serde_json::json!({})),
            ("pid".into(), serde_json::json!({})),
        ];

        let combined = lower_block_set(&blocks, &profile).expect("lower_block_set failed");

        // Get individual sizes
        let gain_block = crate::registry::create_block("gain").unwrap();
        let gain_len = gain_block.lower().unwrap().dag.len();

        let pid_block = crate::registry::create_block("pid").unwrap();
        let pid_len = pid_block.lower().unwrap().dag.len();

        assert_eq!(
            combined.len(),
            gain_len + pid_len,
            "merged DAG should have sum of individual sizes"
        );
    }

    #[test]
    fn test_lower_block_set_correct_node_id_offsets() {
        let profile = DeploymentProfile::new("bench");

        // Two constant blocks — simplest possible blocks
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("constant".into(), serde_json::json!({"value": 1.0})),
            ("constant".into(), serde_json::json!({"value": 2.0})),
        ];

        let combined = lower_block_set(&blocks, &profile).expect("lower_block_set failed");

        // All NodeId references must be valid (Dag::add_op validates this)
        // If we got here without error, offsets are correct.
        assert!(!combined.is_empty(), "combined DAG should be non-empty");

        // Verify no invalid cross-references exist by checking each op is well-formed
        for (i, op) in combined.nodes().iter().enumerate() {
            let refs: Vec<u16> = match op {
                Op::Output(_, src) | Op::Neg(src) | Op::Relu(src) | Op::Publish(_, src) => {
                    vec![*src]
                }
                Op::Add(a, b) | Op::Mul(a, b) | Op::Sub(a, b) | Op::Div(a, b) | Op::Pow(a, b) => {
                    vec![*a, *b]
                }
                _ => vec![],
            };
            for r in refs {
                assert!(
                    (r as usize) < i,
                    "op {i} references node {r} which is not before it"
                );
            }
        }
    }

    #[test]
    fn test_lower_block_set_applies_channel_remapping() {
        let mut profile = DeploymentProfile::new("remapped");

        // Find PID's default setpoint topic and remap it
        let pid_block = crate::registry::create_block("pid").unwrap();
        let pid_lower = pid_block.lower().unwrap();
        let default_sub = pid_lower
            .dag
            .nodes()
            .iter()
            .find_map(|op| {
                if let Op::Subscribe(t) = op {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .expect("pid should have Subscribe");

        profile
            .channel_map
            .insert(default_sub.clone(), "deployed/setpoint".into());

        let blocks: Vec<(String, serde_json::Value)> = vec![("pid".into(), serde_json::json!({}))];

        let combined = lower_block_set(&blocks, &profile).expect("lower_block_set failed");

        let has_remapped = combined
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Subscribe(t) if t == "deployed/setpoint"));
        assert!(has_remapped, "combined DAG should contain remapped topic");

        let has_original = combined
            .nodes()
            .iter()
            .any(|op| matches!(op, Op::Subscribe(t) if *t == default_sub));
        assert!(!has_original, "original topic should be remapped away");
    }

    #[test]
    fn test_lower_block_set_empty_returns_empty_dag() {
        let profile = DeploymentProfile::new("bench");
        let combined = lower_block_set(&[], &profile).expect("should succeed");
        assert!(combined.is_empty());
    }

    #[test]
    fn test_lower_block_set_per_node_partitions_by_assignment() {
        let mut profile = DeploymentProfile::new("multi");
        profile.add_board("board_a", "Rp2040");
        profile.add_board("board_b", "Stm32f4");
        profile.assign_block(0, "board_a");
        profile.assign_block(1, "board_b");

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("constant".into(), serde_json::json!({"value": 1.0})),
            ("constant".into(), serde_json::json!({"value": 2.0})),
        ];

        let node_dags = lower_block_set_per_node(&blocks, &profile).expect("should succeed");

        assert!(node_dags.contains_key("board_a"), "should have board_a DAG");
        assert!(node_dags.contains_key("board_b"), "should have board_b DAG");
        assert!(!node_dags.get("board_a").unwrap().is_empty());
        assert!(!node_dags.get("board_b").unwrap().is_empty());
        // Each node gets only its own block, not both
        assert!(!node_dags.contains_key("_default"), "no unassigned blocks");
    }

    #[test]
    fn test_remap_dag_channels_unmapped_topic_preserved() {
        let mut dag = Dag::new();
        dag.add_op(Op::Subscribe("unmapped/topic".into())).unwrap();
        dag.add_op(Op::Publish("also/unmapped".into(), 0)).unwrap();

        let map = ChannelMap::new(); // empty — no remappings
        let remapped = remap_dag_channels(&dag, &map).expect("remap failed");

        match &remapped.nodes()[0] {
            Op::Subscribe(t) => assert_eq!(t, "unmapped/topic"),
            other => panic!("expected Subscribe, got {:?}", other),
        }
        match &remapped.nodes()[1] {
            Op::Publish(t, _) => assert_eq!(t, "also/unmapped"),
            other => panic!("expected Publish, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_block_set_unknown_block_type_returns_error() {
        let profile = DeploymentProfile::new("bench");
        let blocks: Vec<(String, serde_json::Value)> =
            vec![("nonexistent_block_xyz".into(), serde_json::json!({}))];
        let result = lower_block_set(&blocks, &profile);
        match result {
            Err(e) => assert!(e.contains("unknown block type"), "unexpected error: {e}"),
            Ok(_) => panic!("expected an error for unknown block type"),
        }
    }

    // ── offset_op coverage for various Op variants ──────────────────────

    #[test]
    fn test_offset_op_div() {
        let op = Op::Div(2, 3);
        let shifted = offset_op(&op, 10);
        assert_eq!(shifted, Op::Div(12, 13));
    }

    #[test]
    fn test_offset_op_pow() {
        let op = Op::Pow(1, 4);
        let shifted = offset_op(&op, 5);
        assert_eq!(shifted, Op::Pow(6, 9));
    }

    #[test]
    fn test_offset_op_input() {
        let op = Op::Input("sensor".into());
        let shifted = offset_op(&op, 100);
        assert_eq!(shifted, Op::Input("sensor".into()));
    }

    #[test]
    fn test_offset_op_output() {
        let op = Op::Output("out".into(), 3);
        let shifted = offset_op(&op, 10);
        assert_eq!(shifted, Op::Output("out".into(), 13));
    }

    #[test]
    fn test_offset_op_const() {
        let op = Op::Const(42.0);
        let shifted = offset_op(&op, 99);
        assert_eq!(shifted, Op::Const(42.0));
    }

    #[test]
    fn test_offset_op_subscribe() {
        let op = Op::Subscribe("topic".into());
        let shifted = offset_op(&op, 50);
        assert_eq!(shifted, Op::Subscribe("topic".into()));
    }

    #[test]
    fn test_offset_op_add() {
        let op = Op::Add(1, 2);
        let shifted = offset_op(&op, 10);
        assert_eq!(shifted, Op::Add(11, 12));
    }

    #[test]
    fn test_offset_op_mul() {
        let op = Op::Mul(0, 3);
        let shifted = offset_op(&op, 5);
        assert_eq!(shifted, Op::Mul(5, 8));
    }

    #[test]
    fn test_offset_op_sub() {
        let op = Op::Sub(2, 0);
        let shifted = offset_op(&op, 7);
        assert_eq!(shifted, Op::Sub(9, 7));
    }

    #[test]
    fn test_offset_op_neg() {
        let op = Op::Neg(3);
        let shifted = offset_op(&op, 10);
        assert_eq!(shifted, Op::Neg(13));
    }

    #[test]
    fn test_offset_op_relu() {
        let op = Op::Relu(5);
        let shifted = offset_op(&op, 2);
        assert_eq!(shifted, Op::Relu(7));
    }

    #[test]
    fn test_offset_op_publish() {
        let op = Op::Publish("out".into(), 3);
        let shifted = offset_op(&op, 10);
        assert_eq!(shifted, Op::Publish("out".into(), 13));
    }

    // ── lower_block_set_per_node with unassigned blocks ─────────────────

    #[test]
    fn test_lower_block_set_per_node_unassigned_goes_to_default() {
        let profile = DeploymentProfile::new("no_assignments");
        // No node_assignments set -- blocks should go to "_default"
        let blocks: Vec<(String, serde_json::Value)> =
            vec![("constant".into(), serde_json::json!({"value": 1.0}))];
        let node_dags = lower_block_set_per_node(&blocks, &profile).expect("should succeed");
        assert!(
            node_dags.contains_key("_default"),
            "unassigned blocks should go to _default"
        );
        assert!(!node_dags.get("_default").unwrap().is_empty());
    }

    // ── lower_to_il_text with a hardware channel ────────────────────────

    #[test]
    fn test_lower_to_il_text_with_hardware_channel() {
        // ADC block has a Hardware-kind channel
        let adc = crate::blocks::basic::AdcBlock::default();
        let text = lower_to_il_text(&adc).expect("lower failed");
        assert!(text.contains("block @adc"), "should contain block type");
        assert!(
            text.contains("hw"),
            "should contain hw label for hardware channel"
        );
        assert!(text.contains("adc0"), "should contain channel name");
    }

    #[test]
    fn test_lower_to_il_text_with_pwm() {
        // PWM block has a Hardware-kind output channel
        let pwm = crate::blocks::basic::PwmBlock::default();
        let text = lower_to_il_text(&pwm).expect("lower failed");
        assert!(text.contains("block @pwm"), "should contain block type");
        assert!(
            text.contains("hw"),
            "should contain hw label for hardware channel"
        );
        assert!(text.contains("pwm0"), "should contain channel name");
    }

    #[test]
    fn test_lower_block_set_per_node_unknown_block_type() {
        let profile = DeploymentProfile::new("bench");
        let blocks: Vec<(String, serde_json::Value)> =
            vec![("nonexistent_block_xyz".into(), serde_json::json!({}))];
        let result = lower_block_set_per_node(&blocks, &profile);
        match result {
            Err(e) => assert!(e.contains("unknown block type"), "unexpected error: {e}"),
            Ok(_) => panic!("expected an error for unknown block type"),
        }
    }

    #[test]
    fn test_lower_block_set_per_node_empty() {
        let profile = DeploymentProfile::new("bench");
        let node_dags = lower_block_set_per_node(&[], &profile).expect("should succeed");
        assert!(node_dags.is_empty());
    }
}
