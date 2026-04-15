//! Pure-Rust graph engine: block management, DAG evaluation, snapshot export.
//!
//! This module is intentionally free of `web-sys`, `leptos`, and any
//! WASM-specific API so that it can be tested on the host target with
//! `cargo test -p combined-frontend`.

use std::collections::BTreeMap;

use dag_core::eval::SimState;
use dag_core::op::{Dag, Op};
use graph_model::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot};

/// Entry stored for each block placed in the graph.
pub struct BlockEntry {
    id: BlockId,
    block_type: String,
    config: serde_json::Value,
    name: String,
}

impl BlockEntry {
    /// Block identifier.
    pub fn id(&self) -> BlockId {
        self.id
    }

    /// Block type string (e.g. `"constant"`, `"pid"`).
    pub fn block_type(&self) -> &str {
        &self.block_type
    }

    /// Current JSON configuration.
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// Human-readable name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Pure-Rust dataflow graph engine.
///
/// Manages blocks and channels, builds a merged DAG on each tick, and
/// evaluates it with [`SimState`] for pubsub-based cross-tick feedback.
pub struct GraphEngine {
    blocks: Vec<BlockEntry>,
    channels: Vec<Channel>,
    next_block_id: u32,
    next_channel_id: u32,
    dt: f64,
    tick_count: u64,
    time: f64,
    sim_state: Option<SimState>,
}

impl GraphEngine {
    /// Create a new engine with the given time-step (seconds per tick).
    pub fn new(dt: f64) -> Self {
        Self {
            blocks: Vec::new(),
            channels: Vec::new(),
            next_block_id: 1,
            next_channel_id: 1,
            dt,
            tick_count: 0,
            time: 0.0,
            sim_state: None,
        }
    }

    /// Add a block of the given type with the provided config.
    ///
    /// Returns `None` if the block type is unknown (validated via
    /// `block_registry::create_block`).
    pub fn add_block(&mut self, block_type: &str, config: serde_json::Value) -> Option<BlockId> {
        let config_str = config.to_string();
        // Validate the block type exists in the registry.
        let module = block_registry::create_block(block_type, &config_str).ok()?;
        let name = module.name().to_string();

        let id = BlockId(self.next_block_id);
        self.next_block_id += 1;

        self.blocks.push(BlockEntry {
            id,
            block_type: block_type.to_string(),
            config,
            name,
        });

        // Invalidate cached sim state since the graph changed.
        self.sim_state = None;

        Some(id)
    }

    /// Remove a block by id. Also removes any channels attached to it.
    /// Returns `true` if the block existed.
    pub fn remove_block(&mut self, id: BlockId) -> bool {
        let before = self.blocks.len();
        self.blocks.retain(|b| b.id != id);
        let removed = self.blocks.len() < before;
        if removed {
            // Remove channels that reference this block.
            self.channels
                .retain(|ch| ch.from_block != id && ch.to_block != id);
            self.sim_state = None;
        }
        removed
    }

    /// Update a block's type and config in-place.
    /// Returns `true` if the block was found and the new type is valid.
    pub fn update_block(
        &mut self,
        id: BlockId,
        block_type: &str,
        config: serde_json::Value,
    ) -> bool {
        let config_str = config.to_string();
        let module = match block_registry::create_block(block_type, &config_str) {
            Ok(m) => m,
            Err(_) => return false,
        };

        if let Some(entry) = self.blocks.iter_mut().find(|b| b.id == id) {
            entry.block_type = block_type.to_string();
            entry.config = config;
            entry.name = module.name().to_string();
            self.sim_state = None;
            true
        } else {
            false
        }
    }

    /// Connect an output port on one block to an input port on another.
    ///
    /// Returns `None` if either block id is unknown.
    pub fn connect(
        &mut self,
        from_block: BlockId,
        from_port: usize,
        to_block: BlockId,
        to_port: usize,
    ) -> Option<ChannelId> {
        // Validate both blocks exist.
        let from_exists = self.blocks.iter().any(|b| b.id == from_block);
        let to_exists = self.blocks.iter().any(|b| b.id == to_block);
        if !from_exists || !to_exists {
            return None;
        }

        let id = ChannelId(self.next_channel_id);
        self.next_channel_id += 1;

        self.channels.push(Channel {
            id,
            from_block,
            from_port,
            to_block,
            to_port,
        });

        self.sim_state = None;
        Some(id)
    }

    /// Remove a channel by id. Returns `true` if it existed.
    pub fn disconnect(&mut self, channel_id: ChannelId) -> bool {
        let before = self.channels.len();
        self.channels.retain(|ch| ch.id != channel_id);
        let removed = self.channels.len() < before;
        if removed {
            self.sim_state = None;
        }
        removed
    }

    /// Advance the simulation by one tick.
    ///
    /// Builds a merged DAG from all blocks (via `configurable_blocks`),
    /// evaluates it with [`SimState`], and increments the tick counter.
    pub fn tick(&mut self) {
        let dag = match self.build_dag() {
            Some(d) => d,
            None => return,
        };

        let sim = self
            .sim_state
            .get_or_insert_with(|| SimState::new(dag.len()));

        sim.tick(&dag);
        self.tick_count = sim.tick_count();
        self.time = self.tick_count as f64 * self.dt;
    }

    /// Reset the simulation: clear tick counter, time, and SimState.
    pub fn reset(&mut self) {
        self.tick_count = 0;
        self.time = 0.0;
        self.sim_state = None;
    }

    /// Export a snapshot of the current graph state.
    pub fn snapshot(&self) -> GraphSnapshot {
        let blocks = self
            .blocks
            .iter()
            .map(|entry| {
                let config_str = entry.config.to_string();
                // Re-create the Module to query ports.
                let (inputs, outputs, is_delay) =
                    match block_registry::create_block(&entry.block_type, &config_str) {
                        Ok(module) => (
                            module.input_ports(),
                            module.output_ports(),
                            module.is_delay(),
                        ),
                        Err(_) => (Vec::new(), Vec::new(), false),
                    };

                BlockSnapshot {
                    id: entry.id,
                    block_type: entry.block_type.clone(),
                    name: entry.name.clone(),
                    inputs,
                    outputs,
                    config: entry.config.clone(),
                    is_delay,
                }
            })
            .collect();

        GraphSnapshot {
            blocks,
            channels: self.channels.clone(),
        }
    }

    /// List all available block types from the registry.
    pub fn block_types() -> Vec<block_registry::BlockTypeInfo> {
        block_registry::available_block_types()
    }

    /// Current pubsub topic values from the SimState.
    pub fn topics(&self) -> BTreeMap<String, f64> {
        self.sim_state
            .as_ref()
            .map(|s| s.topics().clone())
            .unwrap_or_default()
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Slice of all block entries (for UI access).
    pub fn blocks(&self) -> &[BlockEntry] {
        &self.blocks
    }

    /// Slice of all channels.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    // ---- internal helpers ----

    /// Build a merged DAG from all blocks using `configurable_blocks`.
    fn build_dag(&self) -> Option<Dag> {
        if self.blocks.is_empty() {
            return None;
        }

        let mut combined = Dag::new();
        for entry in &self.blocks {
            // Try to create via configurable-blocks registry (for DAG lowering).
            let mut block = match configurable_blocks::registry::create_block(&entry.block_type) {
                Some(b) => b,
                None => continue, // Skip types not in configurable-blocks.
            };
            block.apply_config(&entry.config);

            let result = match block.lower() {
                Ok(r) => r,
                Err(_) => continue,
            };

            let offset = combined.len() as u16;
            for op in result.dag.nodes() {
                let adjusted = offset_op(op, offset);
                if combined.add_op(adjusted).is_err() {
                    return None;
                }
            }
        }

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }
}

/// Offset all `NodeId` references in an Op by `offset`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_engine() {
        let engine = GraphEngine::new(0.01);
        let snap = engine.snapshot();
        assert!(snap.blocks.is_empty());
        assert!(snap.channels.is_empty());
        assert_eq!(engine.tick_count(), 0);
    }

    #[test]
    fn test_add_remove_block() {
        let mut engine = GraphEngine::new(0.01);

        // Add a constant block.
        let id = engine
            .add_block("constant", serde_json::json!({"value": 42.0}))
            .expect("constant should be a valid block type");

        let snap = engine.snapshot();
        assert_eq!(snap.blocks.len(), 1);
        assert_eq!(snap.blocks[0].id, id);

        // Remove it.
        assert!(engine.remove_block(id));
        let snap = engine.snapshot();
        assert!(snap.blocks.is_empty());

        // Removing again returns false.
        assert!(!engine.remove_block(id));
    }

    #[test]
    fn test_add_block_invalid_type() {
        let mut engine = GraphEngine::new(0.01);
        let result = engine.add_block("nonexistent_block_xyz", serde_json::json!({}));
        assert!(result.is_none());
    }

    #[test]
    fn test_connect_disconnect() {
        let mut engine = GraphEngine::new(0.01);

        let a = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .expect("add constant");
        let b = engine
            .add_block("gain", serde_json::json!({"gain": 2.0}))
            .expect("add gain");

        // Connect output 0 of A to input 0 of B.
        let ch_id = engine.connect(a, 0, b, 0).expect("connect should succeed");

        let snap = engine.snapshot();
        assert_eq!(snap.channels.len(), 1);
        assert_eq!(snap.channels[0].id, ch_id);
        assert_eq!(snap.channels[0].from_block, a);
        assert_eq!(snap.channels[0].to_block, b);

        // Disconnect.
        assert!(engine.disconnect(ch_id));
        let snap = engine.snapshot();
        assert!(snap.channels.is_empty());

        // Disconnect again returns false.
        assert!(!engine.disconnect(ch_id));
    }

    #[test]
    fn test_connect_invalid_block() {
        let mut engine = GraphEngine::new(0.01);
        let a = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .expect("add constant");
        // Non-existent block id.
        let result = engine.connect(a, 0, BlockId(9999), 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_tick_produces_values() {
        let mut engine = GraphEngine::new(0.01);

        // A constant block with a publish_topic publishes its value via pubsub.
        // The configurable-blocks ConstantBlock uses `publish_topic` for the
        // output topic name.
        engine
            .add_block(
                "constant",
                serde_json::json!({"value": 5.0, "publish_topic": "test_val"}),
            )
            .expect("add constant");

        assert_eq!(engine.tick_count(), 0);
        engine.tick();
        assert_eq!(engine.tick_count(), 1);

        // The constant block should have produced a pubsub topic.
        let topics = engine.topics();
        assert!(!topics.is_empty(), "topics should be populated after tick");
    }

    #[test]
    fn test_reset_clears_state() {
        let mut engine = GraphEngine::new(0.01);

        engine
            .add_block(
                "constant",
                serde_json::json!({"value": 5.0, "publish_topic": "x"}),
            )
            .expect("add constant");

        engine.tick();
        engine.tick();
        assert!(engine.tick_count() > 0);

        engine.reset();
        assert_eq!(engine.tick_count(), 0);
        assert!(engine.topics().is_empty());
    }

    #[test]
    fn test_snapshot_block_fields() {
        let mut engine = GraphEngine::new(0.01);

        let id = engine
            .add_block("constant", serde_json::json!({"value": 3.25}))
            .expect("add constant");

        let snap = engine.snapshot();
        assert_eq!(snap.blocks.len(), 1);

        let bs = &snap.blocks[0];
        assert_eq!(bs.id, id);
        assert_eq!(bs.block_type, "constant");
        // Name should be non-empty (set by Module::name()).
        assert!(!bs.name.is_empty());
    }

    #[test]
    fn test_remove_block_removes_channels() {
        let mut engine = GraphEngine::new(0.01);

        let a = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .expect("add constant");
        let b = engine
            .add_block("gain", serde_json::json!({"gain": 2.0}))
            .expect("add gain");

        engine.connect(a, 0, b, 0).expect("connect");
        assert_eq!(engine.channels().len(), 1);

        // Removing block A should also remove the channel.
        engine.remove_block(a);
        assert!(engine.channels().is_empty());
    }

    #[test]
    fn test_update_block() {
        let mut engine = GraphEngine::new(0.01);

        let id = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .expect("add constant");

        // Update to a different config.
        assert!(engine.update_block(id, "constant", serde_json::json!({"value": 99.0})));

        let snap = engine.snapshot();
        assert_eq!(snap.blocks[0].config["value"], 99.0);
    }

    #[test]
    fn test_update_block_invalid_type() {
        let mut engine = GraphEngine::new(0.01);

        let id = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .expect("add constant");

        assert!(!engine.update_block(id, "nonexistent_xyz", serde_json::json!({})));
    }

    #[test]
    fn test_update_block_nonexistent_id() {
        let mut engine = GraphEngine::new(0.01);
        assert!(!engine.update_block(BlockId(999), "constant", serde_json::json!({})));
    }

    #[test]
    fn test_block_types_nonempty() {
        let types = GraphEngine::block_types();
        assert!(!types.is_empty(), "should list available block types");
    }
}
