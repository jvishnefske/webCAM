//! Graph engine: manages blocks, channels (edges), simulation, and snapshots.
//!
//! This is a pure-Rust data structure (no Leptos signals, no DOM). It holds:
//! - A set of blocks identified by `u32` ids.
//! - Explicit channels (edges) connecting output ports to input ports.
//! - An optional `SimState` for in-browser DAG simulation.
//!
//! The editor component stores a `GraphEngine` in a `thread_local! RefCell`
//! and drives it through Leptos event handlers.

use std::collections::BTreeMap;

use configurable_blocks::lower;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;
use dag_core::op::{Dag, Op};

/// Unique block identifier.
pub type BlockId = u32;

/// Unique channel (edge) identifier.
pub type ChannelId = u32;

/// A block stored in the engine.
#[derive(Clone, Debug)]
pub struct EngineBlock {
    pub id: BlockId,
    pub block_type: String,
    pub config: serde_json::Value,
}

impl EngineBlock {
    /// Reconstruct the `ConfigurableBlock` trait object from the registry.
    pub fn reconstruct(&self) -> Option<Box<dyn lower::ConfigurableBlock>> {
        let mut block = registry::create_block(&self.block_type)?;
        block.apply_config(&self.config);
        Some(block)
    }
}

/// A channel (edge) connecting an output port on one block to an input port on another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub id: ChannelId,
    pub from_block: BlockId,
    pub from_port: usize,
    pub to_block: BlockId,
    pub to_port: usize,
    /// Auto-generated topic name used for codegen wiring.
    pub topic: String,
}

/// Serializable snapshot of the entire graph (blocks + channels).
///
/// Used for undo/redo and persistence.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphSnapshot {
    pub blocks: Vec<SnapshotBlock>,
    pub channels: Vec<SnapshotChannel>,
    pub next_block_id: u32,
    pub next_channel_id: u32,
}

/// Serializable block in a snapshot.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SnapshotBlock {
    pub id: BlockId,
    pub block_type: String,
    pub config: serde_json::Value,
}

/// Serializable channel in a snapshot.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SnapshotChannel {
    pub id: ChannelId,
    pub from_block: BlockId,
    pub from_port: usize,
    pub to_block: BlockId,
    pub to_port: usize,
    pub topic: String,
}

/// The main graph engine.
pub struct GraphEngine {
    blocks: Vec<EngineBlock>,
    channels: Vec<Channel>,
    next_block_id: BlockId,
    next_channel_id: ChannelId,
    /// Simulation state -- lazily created on first tick.
    sim: Option<dag_core::eval::SimState>,
    /// Externally injected pubsub values (from panel widgets).
    /// Merged into SimState before each tick.
    injected: BTreeMap<String, f64>,
}

impl GraphEngine {
    /// Create a new empty graph engine.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            channels: Vec::new(),
            next_block_id: 1,
            next_channel_id: 1,
            sim: None,
            injected: BTreeMap::new(),
        }
    }

    // -- Block CRUD ---------------------------------------------------------

    /// Add a block of the given type with the given config. Returns the assigned `BlockId`.
    ///
    /// Returns `None` if `block_type` is not in the registry.
    pub fn add_block(&mut self, block_type: &str, config: serde_json::Value) -> Option<BlockId> {
        // Validate that the block type exists.
        let _ = registry::create_block(block_type)?;
        let id = self.next_block_id;
        self.next_block_id += 1;
        self.blocks.push(EngineBlock {
            id,
            block_type: block_type.to_string(),
            config,
        });
        self.sim = None; // invalidate sim
        Some(id)
    }

    /// Remove a block and all channels connected to it.
    pub fn remove_block(&mut self, id: BlockId) {
        self.blocks.retain(|b| b.id != id);
        self.channels
            .retain(|ch| ch.from_block != id && ch.to_block != id);
        self.sim = None;
    }

    /// Get a reference to a block by id.
    pub fn block(&self, id: BlockId) -> Option<&EngineBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Get a mutable reference to a block by id.
    pub fn block_mut(&mut self, id: BlockId) -> Option<&mut EngineBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// All blocks.
    pub fn blocks(&self) -> &[EngineBlock] {
        &self.blocks
    }

    // -- Channel (edge) CRUD ------------------------------------------------

    /// Connect an output port on `from_block` to an input port on `to_block`.
    ///
    /// Returns the `ChannelId` on success, or `None` if either block does not exist
    /// or the connection would be a self-loop.
    pub fn connect(
        &mut self,
        from_block: BlockId,
        from_port: usize,
        to_block: BlockId,
        to_port: usize,
    ) -> Option<ChannelId> {
        if from_block == to_block {
            return None;
        }
        // Verify both blocks exist.
        if self.block(from_block).is_none() || self.block(to_block).is_none() {
            return None;
        }
        // Prevent duplicate connections to the same input port.
        if self
            .channels
            .iter()
            .any(|ch| ch.to_block == to_block && ch.to_port == to_port)
        {
            return None;
        }
        let id = self.next_channel_id;
        self.next_channel_id += 1;
        let topic = format!("wire_{}_{}", from_block, from_port);
        self.channels.push(Channel {
            id,
            from_block,
            from_port,
            to_block,
            to_port,
            topic,
        });
        self.sim = None;
        Some(id)
    }

    /// Disconnect (remove) a channel by its id.
    pub fn disconnect(&mut self, channel_id: ChannelId) -> bool {
        let before = self.channels.len();
        self.channels.retain(|ch| ch.id != channel_id);
        let removed = self.channels.len() < before;
        if removed {
            self.sim = None;
        }
        removed
    }

    /// All channels.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Find a channel by id.
    pub fn channel(&self, id: ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|ch| ch.id == id)
    }

    /// Channels connected to a block (either direction).
    pub fn channels_for_block(&self, block_id: BlockId) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|ch| ch.from_block == block_id || ch.to_block == block_id)
            .collect()
    }

    // -- Simulation ---------------------------------------------------------

    /// Build a merged DAG from all blocks, wiring channels via auto-topic names.
    ///
    /// For each channel, the source block's output config key is set to the
    /// channel topic, and the target block's input config key is set to the
    /// same topic. This makes `Subscribe`/`Publish` ops match up.
    pub fn build_dag(&self) -> Result<Dag, String> {
        if self.blocks.is_empty() {
            return Err("No blocks".into());
        }

        // Build configs with channel topics applied.
        let configs = self.configs_with_channels();

        let mut combined = Dag::new();
        for (blk, config) in self.blocks.iter().zip(configs.iter()) {
            let mut block = blk
                .reconstruct()
                .ok_or_else(|| format!("Unknown block type: {}", blk.block_type))?;
            block.apply_config(config);
            let result = block.lower().map_err(|e| format!("Lower error: {:?}", e))?;
            let offset = combined.len() as u16;
            for op in result.dag.nodes() {
                let adjusted = offset_op(op, offset);
                combined
                    .add_op(adjusted)
                    .map_err(|e| format!("Merge error: {:?}", e))?;
            }
        }
        Ok(combined)
    }

    /// Build configs for all blocks with channel topic names injected.
    fn configs_with_channels(&self) -> Vec<serde_json::Value> {
        let mut configs: Vec<serde_json::Value> =
            self.blocks.iter().map(|b| b.config.clone()).collect();

        for ch in &self.channels {
            // Source block: find the output config key and set it to the channel topic.
            if let Some(src_idx) = self.blocks.iter().position(|b| b.id == ch.from_block) {
                if let Some(src_block) = self.blocks[src_idx].reconstruct() {
                    let channels = src_block.declared_channels();
                    let out_channels: Vec<_> = channels
                        .iter()
                        .filter(|c| c.direction == ChannelDirection::Output)
                        .collect();
                    if let Some(out_ch) = out_channels.get(ch.from_port) {
                        if let Some(key) =
                            find_config_key_for_channel(&configs[src_idx], &out_ch.name)
                        {
                            if let serde_json::Value::Object(ref mut map) = configs[src_idx] {
                                map.insert(key, serde_json::Value::String(ch.topic.clone()));
                            }
                        }
                    }
                }
            }
            // Target block: find the input config key and set it to the channel topic.
            if let Some(dst_idx) = self.blocks.iter().position(|b| b.id == ch.to_block) {
                if let Some(dst_block) = self.blocks[dst_idx].reconstruct() {
                    let channels = dst_block.declared_channels();
                    let in_channels: Vec<_> = channels
                        .iter()
                        .filter(|c| c.direction == ChannelDirection::Input)
                        .collect();
                    if let Some(in_ch) = in_channels.get(ch.to_port) {
                        if let Some(key) =
                            find_config_key_for_channel(&configs[dst_idx], &in_ch.name)
                        {
                            if let serde_json::Value::Object(ref mut map) = configs[dst_idx] {
                                map.insert(key, serde_json::Value::String(ch.topic.clone()));
                            }
                        }
                    }
                }
            }
        }
        configs
    }

    /// Execute one simulation tick. Creates SimState on first call.
    pub fn tick(&mut self) -> Result<(), String> {
        let dag = self.build_dag()?;
        let sim = self
            .sim
            .get_or_insert_with(|| dag_core::eval::SimState::new(dag.len()));
        sim.tick(&dag);
        Ok(())
    }

    /// Externally inject a pubsub topic value (e.g. from a UI slider widget).
    /// The value will be visible to `read_topic` immediately.
    pub fn inject_topic(&mut self, topic: &str, value: f64) {
        self.injected.insert(topic.into(), value);
    }

    /// Read a pubsub topic's current value (e.g. for a UI gauge widget).
    pub fn read_topic(&self, topic: &str) -> Option<f64> {
        // Check live SimState first (has both injected and DAG-published values).
        if let Some(sim) = &self.sim {
            if let Some(v) = sim.pubsub_value(topic) {
                return Some(v);
            }
        }
        // Fall back to injected values (before first tick).
        self.injected.get(topic).copied()
    }

    /// Current pubsub topics from the simulation.
    pub fn topics(&self) -> BTreeMap<String, f64> {
        self.sim
            .as_ref()
            .map(|s| s.topics().clone())
            .unwrap_or_default()
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.sim.as_ref().map(|s| s.tick_count()).unwrap_or(0)
    }

    /// Reset simulation state and injected values.
    pub fn reset_sim(&mut self) {
        self.sim = None;
        self.injected.clear();
    }

    // -- Snapshot ------------------------------------------------------------

    /// Take a serializable snapshot of the entire graph.
    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            blocks: self
                .blocks
                .iter()
                .map(|b| SnapshotBlock {
                    id: b.id,
                    block_type: b.block_type.clone(),
                    config: b.config.clone(),
                })
                .collect(),
            channels: self
                .channels
                .iter()
                .map(|ch| SnapshotChannel {
                    id: ch.id,
                    from_block: ch.from_block,
                    from_port: ch.from_port,
                    to_block: ch.to_block,
                    to_port: ch.to_port,
                    topic: ch.topic.clone(),
                })
                .collect(),
            next_block_id: self.next_block_id,
            next_channel_id: self.next_channel_id,
        }
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snap: &GraphSnapshot) {
        self.blocks = snap
            .blocks
            .iter()
            .map(|b| EngineBlock {
                id: b.id,
                block_type: b.block_type.clone(),
                config: b.config.clone(),
            })
            .collect();
        self.channels = snap
            .channels
            .iter()
            .map(|ch| Channel {
                id: ch.id,
                from_block: ch.from_block,
                from_port: ch.from_port,
                to_block: ch.to_block,
                to_port: ch.to_port,
                topic: ch.topic.clone(),
            })
            .collect();
        self.next_block_id = snap.next_block_id;
        self.next_channel_id = snap.next_channel_id;
        self.sim = None;
    }

    /// Number of blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Update a block's config.
    pub fn update_config(&mut self, block_id: BlockId, key: String, value: serde_json::Value) {
        if let Some(blk) = self.block_mut(block_id) {
            if let serde_json::Value::Object(ref mut map) = blk.config {
                map.insert(key, value);
            }
            self.sim = None;
        }
    }
}

impl Default for GraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the config key whose current value matches `channel_name`.
fn find_config_key_for_channel(config: &serde_json::Value, channel_name: &str) -> Option<String> {
    let obj = config.as_object()?;
    for (key, val) in obj {
        if let Some(s) = val.as_str() {
            if s == channel_name {
                return Some(key.clone());
            }
        }
    }
    None
}

/// Offset all NodeId references in an Op by a given amount.
fn offset_op(op: &Op, offset: u16) -> Op {
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

// -- Tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_engine_is_empty() {
        let engine = GraphEngine::new();
        assert_eq!(engine.block_count(), 0);
        assert_eq!(engine.channel_count(), 0);
        assert_eq!(engine.tick_count(), 0);
    }

    #[test]
    fn test_add_block() {
        let mut engine = GraphEngine::new();
        let id = engine
            .add_block("constant", serde_json::json!({"value": 42.0}))
            .expect("should add constant block");
        assert_eq!(engine.block_count(), 1);
        assert_eq!(engine.block(id).unwrap().block_type, "constant");
    }

    #[test]
    fn test_add_block_unknown_type_returns_none() {
        let mut engine = GraphEngine::new();
        assert!(engine
            .add_block("nonexistent_xyz", serde_json::json!({}))
            .is_none());
        assert_eq!(engine.block_count(), 0);
    }

    #[test]
    fn test_remove_block() {
        let mut engine = GraphEngine::new();
        let id = engine.add_block("constant", serde_json::json!({})).unwrap();
        engine.remove_block(id);
        assert_eq!(engine.block_count(), 0);
    }

    #[test]
    fn test_remove_block_removes_channels() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block(
                "constant",
                serde_json::json!({"value": 1.0, "publish_topic": "t"}),
            )
            .unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);
        assert_eq!(engine.channel_count(), 1);
        engine.remove_block(a);
        assert_eq!(engine.channel_count(), 0);
    }

    #[test]
    fn test_connect_and_disconnect() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        let ch_id = engine.connect(a, 0, b, 0).expect("should connect");
        assert_eq!(engine.channel_count(), 1);
        assert!(engine.disconnect(ch_id));
        assert_eq!(engine.channel_count(), 0);
    }

    #[test]
    fn test_connect_self_loop_rejected() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        assert!(engine.connect(a, 0, a, 0).is_none());
    }

    #[test]
    fn test_connect_nonexistent_block_rejected() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        assert!(engine.connect(a, 0, 999, 0).is_none());
        assert!(engine.connect(999, 0, a, 0).is_none());
    }

    #[test]
    fn test_connect_duplicate_input_rejected() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        let c = engine.add_block("constant", serde_json::json!({})).unwrap();
        assert!(engine.connect(a, 0, b, 0).is_some());
        // Same input port on b should be rejected.
        assert!(engine.connect(c, 0, b, 0).is_none());
    }

    #[test]
    fn test_disconnect_nonexistent_returns_false() {
        let mut engine = GraphEngine::new();
        assert!(!engine.disconnect(999));
    }

    #[test]
    fn test_channels_for_block() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        let c = engine.add_block("gain", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);
        engine.connect(a, 0, c, 0);
        assert_eq!(engine.channels_for_block(a).len(), 2);
        assert_eq!(engine.channels_for_block(b).len(), 1);
        assert_eq!(engine.channels_for_block(c).len(), 1);
    }

    #[test]
    fn test_snapshot_and_restore() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block("constant", serde_json::json!({"value": 5.0}))
            .unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);

        let snap = engine.snapshot();
        assert_eq!(snap.blocks.len(), 2);
        assert_eq!(snap.channels.len(), 1);

        // Restore into a fresh engine.
        let mut engine2 = GraphEngine::new();
        engine2.restore(&snap);
        assert_eq!(engine2.block_count(), 2);
        assert_eq!(engine2.channel_count(), 1);
        assert_eq!(engine2.block(a).unwrap().block_type, "constant");
    }

    #[test]
    fn test_snapshot_roundtrip_json() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block("constant", serde_json::json!({"value": 2.5}))
            .unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);

        let snap = engine.snapshot();
        let json = serde_json::to_string(&snap).expect("serialize");
        let restored: GraphSnapshot = serde_json::from_str(&json).expect("deserialize");

        let mut engine2 = GraphEngine::new();
        engine2.restore(&restored);
        assert_eq!(engine2.block_count(), 2);
        assert_eq!(engine2.channel_count(), 1);
    }

    #[test]
    fn test_update_config() {
        let mut engine = GraphEngine::new();
        let id = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .unwrap();
        engine.update_config(id, "value".into(), serde_json::json!(99.0));
        let blk = engine.block(id).unwrap();
        assert_eq!(blk.config["value"], 99.0);
    }

    #[test]
    fn test_tick_with_constant_block() {
        let mut engine = GraphEngine::new();
        engine
            .add_block(
                "constant",
                serde_json::json!({"value": 7.0, "publish_topic": "test/val"}),
            )
            .unwrap();
        engine.tick().expect("tick should succeed");
        assert_eq!(engine.tick_count(), 1);
        let topics = engine.topics();
        assert_eq!(topics.get("test/val"), Some(&7.0));
    }

    #[test]
    fn test_reset_sim() {
        let mut engine = GraphEngine::new();
        engine
            .add_block(
                "constant",
                serde_json::json!({"value": 1.0, "publish_topic": "x"}),
            )
            .unwrap();
        engine.tick().unwrap();
        assert_eq!(engine.tick_count(), 1);
        engine.reset_sim();
        assert_eq!(engine.tick_count(), 0);
        assert!(engine.topics().is_empty());
    }

    #[test]
    fn test_build_dag_empty_returns_error() {
        let engine = GraphEngine::new();
        assert!(engine.build_dag().is_err());
    }

    #[test]
    fn test_default_impl() {
        let engine = GraphEngine::default();
        assert_eq!(engine.block_count(), 0);
    }

    #[test]
    fn test_inject_and_read_topic() {
        let mut engine = GraphEngine::new();
        engine.inject_topic("test", 42.0);
        assert_eq!(engine.read_topic("test"), Some(42.0));
    }

    #[test]
    fn test_read_topic_none_when_empty() {
        let engine = GraphEngine::new();
        assert_eq!(engine.read_topic("nonexistent"), None);
    }
}
