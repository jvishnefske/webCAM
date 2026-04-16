//! Graph engine: builds a combined DAG from configurable blocks and runs
//! tick-based simulation with pubsub topic injection and readback.
//!
//! This is the simulation backend for the DAG editor and panel widgets.
//! Panel widgets inject values (e.g. slider position) via [`GraphEngine::inject_topic`]
//! and read computed values (e.g. gauge reading) via [`GraphEngine::read_topic`].

use std::collections::BTreeMap;

use configurable_blocks::deployment_profile::DeploymentProfile;
use configurable_blocks::lower::lower_block_set;
use dag_core::eval::SimState;

/// Reactive simulation engine built on top of configurable blocks and DAG
/// evaluation.
///
/// Stores a list of `(block_type, config_json)` block definitions, lowers
/// them into a single DAG, and evaluates tick-by-tick using [`SimState`].
/// External code (e.g. panel widgets) can inject pubsub values before a tick
/// and read them back after.
pub struct GraphEngine {
    /// Block definitions: `(block_type, config_json)`.
    blocks: Vec<(String, serde_json::Value)>,
    /// Persistent simulation state (lazily created on first tick or inject).
    sim: Option<SimState>,
    /// Pre-tick buffer for externally injected topics.
    ///
    /// Values here are merged into `SimState.pubsub` before each tick and also
    /// when the SimState is first created. This ensures injected values survive
    /// DAG rebuilds.
    injected: BTreeMap<String, f64>,
    /// Whether blocks have changed since the last tick (requires DAG rebuild).
    dirty: bool,
}

impl GraphEngine {
    /// Create a new empty graph engine with no blocks.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            sim: None,
            injected: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Add a block to the engine.
    ///
    /// `block_type` must match a registered block name (e.g. `"constant"`,
    /// `"gain"`, `"pubsub_bridge"`, `"pid"`). `config` is the JSON
    /// configuration applied to the block before lowering.
    ///
    /// Returns the index of the newly added block.
    pub fn add_block(&mut self, block_type: &str, config: serde_json::Value) -> usize {
        let idx = self.blocks.len();
        self.blocks.push((block_type.into(), config));
        self.dirty = true;
        idx
    }

    /// Remove all blocks and reset simulation state.
    pub fn reset_sim(&mut self) {
        self.blocks.clear();
        self.sim = None;
        self.injected.clear();
        self.dirty = false;
    }

    /// Externally inject a pubsub topic value (e.g. from a UI slider widget).
    ///
    /// The value is immediately readable via [`read_topic`](Self::read_topic)
    /// and will be visible to `Subscribe` DAG ops on the next
    /// [`tick`](Self::tick).
    pub fn inject_topic(&mut self, topic: &str, value: f64) {
        self.injected.insert(topic.into(), value);
        // Also write through to the live SimState so read_topic works
        // immediately (before any tick).
        if let Some(sim) = &mut self.sim {
            sim.set_topic(topic, value);
        }
    }

    /// Read a pubsub topic's current value (e.g. for a UI gauge widget).
    ///
    /// Returns `None` if the topic has never been published or injected.
    pub fn read_topic(&self, topic: &str) -> Option<f64> {
        // Check live SimState first (has both injected and DAG-published values).
        if let Some(sim) = &self.sim {
            if let Some(v) = sim.pubsub_value(topic) {
                return Some(v);
            }
        }
        // Fall back to the injected buffer (covers the case where no tick has
        // run yet but inject_topic was called).
        self.injected.get(topic).copied()
    }

    /// Return a snapshot of all pubsub topic values.
    pub fn topics(&self) -> BTreeMap<String, f64> {
        let mut result = self.injected.clone();
        if let Some(sim) = &self.sim {
            // SimState topics override injected (they include injected values
            // plus any DAG Publish outputs).
            for (k, v) in sim.topics() {
                result.insert(k.clone(), *v);
            }
        }
        result
    }

    /// Evaluate one simulation tick.
    ///
    /// Lowers all blocks into a combined DAG (rebuilds when blocks have
    /// changed), merges injected topic values into the SimState, then
    /// evaluates one tick.
    ///
    /// Returns `Ok(tick_count)` on success, or `Err(message)` if lowering
    /// fails (e.g. unknown block type).
    pub fn tick(&mut self) -> Result<u64, String> {
        let profile = DeploymentProfile::new("sim");
        let dag = lower_block_set(&self.blocks, &profile)?;

        // If there is no SimState yet, or blocks changed since last tick,
        // create a fresh SimState sized for the current DAG. Preserve any
        // existing pubsub values across the rebuild.
        if self.dirty || self.sim.is_none() {
            let preserved: BTreeMap<String, f64> = self
                .sim
                .as_ref()
                .map(|s| s.topics().clone())
                .unwrap_or_default();

            let mut new_sim = SimState::new(dag.len());
            for (k, v) in &preserved {
                new_sim.set_topic(k, *v);
            }
            self.sim = Some(new_sim);
            self.dirty = false;
        }

        let sim = self.sim.as_mut().expect("sim was just ensured above");

        // Merge injected values into SimState before evaluation.
        for (topic, value) in &self.injected {
            sim.set_topic(topic, *value);
        }

        sim.tick(&dag);
        Ok(sim.tick_count())
    }

    /// Current tick count (0 if no ticks have run).
    pub fn tick_count(&self) -> u64 {
        self.sim.as_ref().map_or(0, |s| s.tick_count())
    }
}

impl Default for GraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- inject / read basics ----

    #[test]
    fn test_inject_topic_sets_value() {
        let mut engine = GraphEngine::new();
        engine.inject_topic("input/slider", 5.0);
        assert_eq!(engine.read_topic("input/slider"), Some(5.0));
    }

    #[test]
    fn test_read_unknown_topic_returns_none() {
        let engine = GraphEngine::new();
        assert_eq!(engine.read_topic("nonexistent"), None);
    }

    #[test]
    fn test_inject_no_dag_stores_value() {
        let mut engine = GraphEngine::new();
        // No blocks added, no ticks -- value should still be readable.
        engine.inject_topic("orphan", 99.0);
        assert_eq!(engine.read_topic("orphan"), Some(99.0));
    }

    #[test]
    fn test_inject_topic_overwrites_previous() {
        let mut engine = GraphEngine::new();
        engine.inject_topic("x", 1.0);
        engine.inject_topic("x", 2.0);
        assert_eq!(engine.read_topic("x"), Some(2.0));
    }

    // ---- tick interaction ----

    #[test]
    fn test_inject_topic_persists_across_ticks() {
        let mut engine = GraphEngine::new();
        // Add a constant block so tick() has something to evaluate.
        engine.add_block("constant", serde_json::json!({"value": 1.0}));
        engine.inject_topic("external", 42.0);
        let _ = engine.tick();
        assert_eq!(engine.read_topic("external"), Some(42.0));
    }

    #[test]
    fn test_dag_publish_readable() {
        let mut engine = GraphEngine::new();
        // Constant(5.0) with publish_topic "result".
        engine.add_block(
            "constant",
            serde_json::json!({"value": 5.0, "publish_topic": "result"}),
        );
        let _ = engine.tick();
        assert_eq!(engine.read_topic("result"), Some(5.0));
    }

    #[test]
    fn test_inject_topic_read_by_subscribe() {
        let mut engine = GraphEngine::new();
        // PublishBlock subscribes to input_topic and publishes to output_topic.
        engine.add_block(
            "publish",
            serde_json::json!({"input_topic": "input", "output_topic": "output"}),
        );
        engine.inject_topic("input", 7.0);
        let _ = engine.tick();
        assert_eq!(engine.read_topic("output"), Some(7.0));
    }

    #[test]
    fn test_full_loop_inject_gain_read() {
        let mut engine = GraphEngine::new();
        // PubSubBridgeBlock: subscribe("input") * gain -> publish("output").
        engine.add_block(
            "pubsub_bridge",
            serde_json::json!({
                "subscribe_topic": "input",
                "publish_topic": "output",
                "gain": 2.0
            }),
        );
        engine.inject_topic("input", 5.0);
        let _ = engine.tick();
        assert_eq!(engine.read_topic("output"), Some(10.0));
    }

    #[test]
    fn test_multiple_inject_tick_cycles() {
        let mut engine = GraphEngine::new();
        engine.add_block(
            "pubsub_bridge",
            serde_json::json!({
                "subscribe_topic": "x",
                "publish_topic": "y",
                "gain": 3.0
            }),
        );

        engine.inject_topic("x", 1.0);
        let _ = engine.tick();
        assert_eq!(engine.read_topic("y"), Some(3.0));

        engine.inject_topic("x", 2.0);
        let _ = engine.tick();
        assert_eq!(engine.read_topic("y"), Some(6.0));
    }

    // ---- topics() snapshot ----

    #[test]
    fn test_topics_includes_injected_and_published() {
        let mut engine = GraphEngine::new();
        engine.add_block(
            "constant",
            serde_json::json!({"value": 10.0, "publish_topic": "from_dag"}),
        );
        engine.inject_topic("from_ui", 77.0);
        let _ = engine.tick();

        let topics = engine.topics();
        assert_eq!(topics.get("from_dag"), Some(&10.0));
        assert_eq!(topics.get("from_ui"), Some(&77.0));
    }

    // ---- reset ----

    #[test]
    fn test_reset_sim_clears_everything() {
        let mut engine = GraphEngine::new();
        engine.add_block(
            "constant",
            serde_json::json!({"value": 1.0, "publish_topic": "t"}),
        );
        engine.inject_topic("ext", 5.0);
        let _ = engine.tick();
        assert!(engine.read_topic("t").is_some());
        assert!(engine.read_topic("ext").is_some());

        engine.reset_sim();
        assert_eq!(engine.read_topic("t"), None);
        assert_eq!(engine.read_topic("ext"), None);
        assert_eq!(engine.tick_count(), 0);
    }

    // ---- tick_count ----

    #[test]
    fn test_tick_count_increments() {
        let mut engine = GraphEngine::new();
        engine.add_block("constant", serde_json::json!({"value": 0.0}));
        assert_eq!(engine.tick_count(), 0);
        let _ = engine.tick();
        assert_eq!(engine.tick_count(), 1);
        let _ = engine.tick();
        assert_eq!(engine.tick_count(), 2);
    }
}
