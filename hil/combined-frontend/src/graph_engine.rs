//! In-browser DAG simulation engine.
//!
//! Manages a set of configurable blocks, lowers them to a combined DAG,
//! and evaluates the DAG tick-by-tick using [`dag_core::eval::SimState`].

use std::collections::BTreeMap;

use configurable_blocks::deployment_profile::DeploymentProfile;
use configurable_blocks::lower::{self, ConfigurableBlock};
use configurable_blocks::registry;
use dag_core::eval::SimState;
use dag_core::op::Dag;

/// A placed block instance tracked by the engine.
struct EngineBlock {
    /// Unique id assigned by the engine.
    id: u32,
    /// Block type name (e.g. "constant", "pid").
    block_type: String,
    /// Current configuration as JSON.
    config: serde_json::Value,
}

impl EngineBlock {
    /// Reconstruct the [`ConfigurableBlock`] trait object from the registry.
    #[allow(dead_code)]
    fn reconstruct(&self) -> Option<Box<dyn ConfigurableBlock>> {
        let mut block = registry::create_block(&self.block_type)?;
        block.apply_config(&self.config);
        Some(block)
    }
}

/// A channel (edge) connecting an output port on one block to an input port
/// on another.
struct EngineChannel {
    id: u32,
    from_block: u32,
    from_port: usize,
    to_block: u32,
    to_port: usize,
}

/// In-browser DAG simulation engine.
///
/// Holds configurable blocks, channels between them, and a [`SimState`]
/// that persists pubsub values across ticks. On each `tick()`, the engine
/// re-lowers all blocks into a combined DAG and evaluates one step.
pub struct GraphEngine {
    blocks: Vec<EngineBlock>,
    channels: Vec<EngineChannel>,
    next_block_id: u32,
    next_channel_id: u32,
    /// Combined DAG (rebuilt on tick if dirty).
    dag: Option<Dag>,
    /// Simulation state for the current DAG.
    sim: Option<SimState>,
    /// Manual topic injections (fed into SimState before tick).
    injected: BTreeMap<String, f64>,
    /// Accumulated tick count (survives DAG rebuilds).
    tick_count: u64,
    /// True when blocks/channels have changed since last DAG build.
    dirty: bool,
}

impl GraphEngine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            channels: Vec::new(),
            next_block_id: 1,
            next_channel_id: 1,
            dag: None,
            sim: None,
            injected: BTreeMap::new(),
            tick_count: 0,
            dirty: true,
        }
    }

    /// Add a block of the given type with the provided config.
    ///
    /// Returns `None` if the block type is unknown.
    pub fn add_block(&mut self, block_type: &str, config: serde_json::Value) -> Option<u32> {
        // Validate that the block type exists in the registry.
        let _ = registry::create_block(block_type)?;

        let id = self.next_block_id;
        self.next_block_id += 1;
        self.blocks.push(EngineBlock {
            id,
            block_type: block_type.to_string(),
            config,
        });
        self.dirty = true;
        Some(id)
    }

    /// Remove a block by id. Also removes any channels connected to it.
    pub fn remove_block(&mut self, id: u32) {
        self.blocks.retain(|b| b.id != id);
        self.channels
            .retain(|ch| ch.from_block != id && ch.to_block != id);
        self.dirty = true;
    }

    /// Update a single config key on a block.
    pub fn update_config(&mut self, id: u32, key: String, value: serde_json::Value) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == id) {
            if let Some(obj) = block.config.as_object_mut() {
                obj.insert(key, value);
            } else {
                let mut obj = serde_json::Map::new();
                obj.insert(key, value);
                block.config = serde_json::Value::Object(obj);
            }
            self.dirty = true;
        }
    }

    /// Connect an output port on one block to an input port on another.
    ///
    /// Returns the channel id, or `None` if either block doesn't exist.
    pub fn connect(
        &mut self,
        from_block: u32,
        from_port: usize,
        to_block: u32,
        to_port: usize,
    ) -> Option<u32> {
        // Validate both blocks exist.
        if !self.blocks.iter().any(|b| b.id == from_block) {
            return None;
        }
        if !self.blocks.iter().any(|b| b.id == to_block) {
            return None;
        }

        let id = self.next_channel_id;
        self.next_channel_id += 1;
        self.channels.push(EngineChannel {
            id,
            from_block,
            from_port,
            to_block,
            to_port,
        });
        self.dirty = true;
        Some(id)
    }

    /// Remove a channel by id. Returns true if a channel was removed.
    pub fn disconnect(&mut self, channel_id: u32) -> bool {
        let before = self.channels.len();
        self.channels.retain(|ch| ch.id != channel_id);
        let removed = self.channels.len() < before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// Rebuild the combined DAG from current blocks, then evaluate one tick.
    pub fn tick(&mut self) -> Result<(), String> {
        if self.dirty || self.dag.is_none() {
            self.rebuild_dag()?;
        }

        if let (Some(dag), Some(sim)) = (&self.dag, &mut self.sim) {
            // Inject any manual topic values before tick.
            for (topic, value) in &self.injected {
                sim.inject(topic, *value);
            }
            sim.tick(dag);
            self.tick_count = sim.tick_count();
        }

        Ok(())
    }

    /// Reset simulation state: clear tick count, pubsub store.
    pub fn reset_sim(&mut self) {
        self.sim = None;
        self.dag = None;
        self.tick_count = 0;
        self.injected.clear();
        self.dirty = true;
    }

    /// Inject a topic value for the next tick.
    pub fn inject_topic(&mut self, topic: &str, value: f64) {
        self.injected.insert(topic.to_string(), value);
    }

    /// Read the current value of a topic from the simulation state.
    pub fn read_topic(&self, topic: &str) -> Option<f64> {
        // Check injected values first, then sim state.
        if let Some(&v) = self.injected.get(topic) {
            return Some(v);
        }
        self.sim.as_ref().and_then(|s| s.pubsub_value(topic))
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// All current pubsub topic values.
    pub fn topics(&self) -> BTreeMap<String, f64> {
        let mut result = BTreeMap::new();
        if let Some(sim) = &self.sim {
            for (k, v) in sim.topics() {
                result.insert(k.clone(), *v);
            }
        }
        // Overlay injected values.
        for (k, v) in &self.injected {
            result.insert(k.clone(), *v);
        }
        result
    }

    /// Access the current block list (id, block_type, config).
    pub fn blocks(&self) -> Vec<(u32, &str, &serde_json::Value)> {
        self.blocks
            .iter()
            .map(|b| (b.id, b.block_type.as_str(), &b.config))
            .collect()
    }

    /// Access the current channel list.
    pub fn channels(&self) -> Vec<(u32, u32, usize, u32, usize)> {
        self.channels
            .iter()
            .map(|ch| (ch.id, ch.from_block, ch.from_port, ch.to_block, ch.to_port))
            .collect()
    }

    // ── Private ─────────────────────────────────────────────────────────

    /// Rebuild the combined DAG from the current set of blocks.
    fn rebuild_dag(&mut self) -> Result<(), String> {
        let block_set: Vec<(String, serde_json::Value)> = self
            .blocks
            .iter()
            .map(|b| (b.block_type.clone(), b.config.clone()))
            .collect();

        let profile = DeploymentProfile::new("sim");
        let dag = lower::lower_block_set(&block_set, &profile)?;
        let node_count = dag.len();

        // Preserve injected values across rebuilds by seeding the new SimState.
        let mut sim = SimState::new(node_count);
        for (topic, value) in &self.injected {
            sim.inject(topic, *value);
        }

        self.dag = Some(dag);
        self.sim = Some(sim);
        self.dirty = false;
        Ok(())
    }
}

impl Default for GraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_engine() {
        let engine = GraphEngine::new();
        assert_eq!(engine.tick_count(), 0);
        assert!(engine.blocks().is_empty());
        assert!(engine.channels().is_empty());
    }

    #[test]
    fn test_add_block_valid() {
        let mut engine = GraphEngine::new();
        let id = engine.add_block("constant", serde_json::json!({"value": 42.0}));
        assert!(id.is_some());
        assert_eq!(engine.blocks().len(), 1);
    }

    #[test]
    fn test_add_block_invalid_type() {
        let mut engine = GraphEngine::new();
        let id = engine.add_block("nonexistent", serde_json::json!({}));
        assert!(id.is_none());
        assert!(engine.blocks().is_empty());
    }

    #[test]
    fn test_remove_block() {
        let mut engine = GraphEngine::new();
        let id = engine.add_block("constant", serde_json::json!({})).unwrap();
        engine.remove_block(id);
        assert!(engine.blocks().is_empty());
    }

    #[test]
    fn test_connect_disconnect() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block("pubsub_bridge", serde_json::json!({}))
            .unwrap();
        let b = engine
            .add_block("pubsub_bridge", serde_json::json!({}))
            .unwrap();
        let ch = engine.connect(a, 0, b, 0);
        assert!(ch.is_some());
        assert_eq!(engine.channels().len(), 1);

        let ok = engine.disconnect(ch.unwrap());
        assert!(ok);
        assert!(engine.channels().is_empty());
    }

    #[test]
    fn test_connect_invalid_block() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        assert!(engine.connect(a, 0, 999, 0).is_none());
    }

    #[test]
    fn test_disconnect_nonexistent() {
        let mut engine = GraphEngine::new();
        assert!(!engine.disconnect(42));
    }

    #[test]
    fn test_tick_empty_engine() {
        let mut engine = GraphEngine::new();
        // Ticking with no blocks should succeed (empty DAG).
        assert!(engine.tick().is_ok());
    }

    #[test]
    fn test_tick_with_constant() {
        let mut engine = GraphEngine::new();
        engine.add_block(
            "constant",
            serde_json::json!({"value": 5.0, "publish_topic": "x"}),
        );
        engine.tick().unwrap();
        assert_eq!(engine.tick_count(), 1);
        assert_eq!(engine.read_topic("x"), Some(5.0));
    }

    #[test]
    fn test_inject_and_read_topic() {
        let mut engine = GraphEngine::new();
        engine.inject_topic("test", 42.0);
        assert_eq!(engine.read_topic("test"), Some(42.0));
    }

    #[test]
    fn test_reset_sim() {
        let mut engine = GraphEngine::new();
        engine.add_block(
            "constant",
            serde_json::json!({"value": 1.0, "publish_topic": "a"}),
        );
        engine.tick().unwrap();
        assert_eq!(engine.tick_count(), 1);

        engine.reset_sim();
        assert_eq!(engine.tick_count(), 0);
        assert!(engine.topics().is_empty());
    }

    #[test]
    fn test_topics_includes_injected() {
        let mut engine = GraphEngine::new();
        engine.inject_topic("manual", 99.0);
        let topics = engine.topics();
        assert_eq!(topics.get("manual"), Some(&99.0));
    }

    #[test]
    fn test_update_config() {
        let mut engine = GraphEngine::new();
        let id = engine
            .add_block("constant", serde_json::json!({"value": 1.0}))
            .unwrap();
        engine.update_config(id, "value".to_string(), serde_json::json!(99.0));
        let blocks = engine.blocks();
        let (_, _, config) = blocks.iter().find(|(bid, _, _)| *bid == id).unwrap();
        assert_eq!(config["value"], 99.0);
    }

    #[test]
    fn test_remove_block_also_removes_channels() {
        let mut engine = GraphEngine::new();
        let a = engine.add_block("constant", serde_json::json!({})).unwrap();
        let b = engine.add_block("constant", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);
        assert_eq!(engine.channels().len(), 1);

        engine.remove_block(a);
        assert!(engine.channels().is_empty());
    }
}
