//! Graph-level simulation state: blocks, DAG building, ticking, topic readout.
//!
//! `GraphState` owns a list of placed blocks and a `dag_core::eval::SimState`
//! for local (in-browser) DAG evaluation.  It is not `Send`/`Sync` (due to
//! trait-object reconstruction), but it is **not** wasm-specific — tests run
//! on the host target.

use std::collections::BTreeMap;

use configurable_blocks::lower::ConfigurableBlock;
use configurable_blocks::registry;
use configurable_blocks::schema::ChannelDirection;
use dag_core::eval::SimState;
use dag_core::op::{Dag, Op};

/// A block placed on the editor canvas.
#[derive(Clone, Debug)]
pub struct PlacedBlock {
    /// Unique id within this graph.
    pub id: usize,
    /// Registry block-type key (e.g. "constant", "pid").
    pub block_type: String,
    /// Current configuration JSON.
    pub config: serde_json::Value,
    /// Canvas x position.
    pub x: f64,
    /// Canvas y position.
    pub y: f64,
}

impl PlacedBlock {
    /// Reconstruct the `ConfigurableBlock` trait object from the registry.
    pub fn reconstruct(&self) -> Option<Box<dyn ConfigurableBlock>> {
        let mut block = registry::create_block(&self.block_type)?;
        block.apply_config(&self.config);
        Some(block)
    }
}

/// Monotonically increasing revision counter — lets reactive views know when
/// the underlying graph has changed.
pub type Revision = u64;

/// Central simulation state for the DAG editor.
///
/// Holds placed blocks, manages DAG lowering + merging, owns a `SimState` for
/// tick-based evaluation, and exposes a revision counter for reactive UI
/// invalidation.
pub struct GraphState {
    blocks: Vec<PlacedBlock>,
    next_id: usize,
    revision: Revision,
    sim: Option<SimState>,
    topics: BTreeMap<String, f64>,
    tick_count: u64,
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphState {
    /// Create a new, empty graph state.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            next_id: 1,
            revision: 0,
            sim: None,
            topics: BTreeMap::new(),
            tick_count: 0,
        }
    }

    /// Current revision — bumped on every mutation.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Immutable view of placed blocks.
    pub fn blocks(&self) -> &[PlacedBlock] {
        &self.blocks
    }

    /// Current pubsub topics and their latest values.
    pub fn topics(&self) -> &BTreeMap<String, f64> {
        &self.topics
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    // ── Mutation ─────────────────────────────────────────────────────────

    /// Add a block to the canvas.  Returns the assigned block id.
    pub fn add_block(
        &mut self,
        block_type: &str,
        config: serde_json::Value,
        x: f64,
        y: f64,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.blocks.push(PlacedBlock {
            id,
            block_type: block_type.to_string(),
            config,
            x,
            y,
        });
        self.revision += 1;
        // Invalidate sim — DAG shape changed.
        self.sim = None;
        id
    }

    /// Remove a block by id.  Returns `true` if found and removed.
    pub fn remove_block(&mut self, id: usize) -> bool {
        let before = self.blocks.len();
        self.blocks.retain(|b| b.id != id);
        let removed = self.blocks.len() < before;
        if removed {
            self.revision += 1;
            self.sim = None;
        }
        removed
    }

    /// Update a single config key on a block.
    pub fn update_config(&mut self, id: usize, key: &str, value: serde_json::Value) {
        if let Some(pb) = self.blocks.iter_mut().find(|b| b.id == id) {
            if let serde_json::Value::Object(ref mut map) = pb.config {
                map.insert(key.to_string(), value);
            }
            self.revision += 1;
            self.sim = None;
        }
    }

    // ── DAG building ────────────────────────────────────────────────────

    /// Lower all blocks and merge into a single DAG.
    pub fn build_dag(&self) -> Result<Dag, String> {
        if self.blocks.is_empty() {
            return Err("No blocks".into());
        }
        let mut combined = Dag::new();
        for pb in &self.blocks {
            let block = pb
                .reconstruct()
                .ok_or_else(|| format!("Unknown block type: {}", pb.block_type))?;
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

    // ── Simulation ──────────────────────────────────────────────────────

    /// Evaluate one tick of the merged DAG.
    ///
    /// Rebuilds the DAG and SimState if they have been invalidated (e.g.
    /// after adding/removing/reconfiguring a block).
    pub fn tick(&mut self) -> Result<(), String> {
        let dag = self.build_dag()?;

        if self.sim.is_none() {
            self.sim = Some(SimState::new(dag.len()));
        }

        if let Some(ref mut s) = self.sim {
            // Resize values buffer if DAG length changed.
            if s.topics().is_empty() && self.tick_count == 0 {
                // Fresh sim — already sized correctly from `new()`.
            }
            s.tick(&dag);
            self.topics = s.topics().clone();
            self.tick_count = s.tick_count();
        }

        self.revision += 1;
        Ok(())
    }

    /// Reset the simulation: clear tick counter and all pubsub topics.
    pub fn reset(&mut self) {
        if let Some(ref mut s) = self.sim {
            s.reset();
        }
        self.topics.clear();
        self.tick_count = 0;
        self.revision += 1;
    }

    // ── Output values for port display ──────────────────────────────────

    /// For each block, return a `Vec<String>` of formatted output-port values.
    ///
    /// The ordering matches the output ports returned by
    /// `declared_channels()` (filtered to `Output` direction).  If a
    /// topic has not been published yet the string is empty.
    pub fn output_values_for_block(&self, id: usize) -> Vec<String> {
        let pb = match self.blocks.iter().find(|b| b.id == id) {
            Some(pb) => pb,
            None => return Vec::new(),
        };
        let block = match pb.reconstruct() {
            Some(b) => b,
            None => return Vec::new(),
        };
        block
            .declared_channels()
            .iter()
            .filter(|ch| ch.direction == ChannelDirection::Output)
            .map(|ch| {
                self.topics
                    .get(&ch.name)
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_default()
            })
            .collect()
    }
}

/// Offset all `NodeId` references in an `Op` by `offset`.
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_graph_state_is_empty() {
        let state = GraphState::new();
        assert!(state.blocks().is_empty());
        assert!(state.topics().is_empty());
        assert_eq!(state.tick_count(), 0);
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn test_add_block_increments_revision() {
        let mut state = GraphState::new();
        let r0 = state.revision();
        state.add_block("constant", serde_json::json!({"value": 1.0}), 0.0, 0.0);
        assert!(state.revision() > r0);
        assert_eq!(state.blocks().len(), 1);
    }

    #[test]
    fn test_add_block_returns_unique_ids() {
        let mut state = GraphState::new();
        let id1 = state.add_block("constant", serde_json::json!({}), 0.0, 0.0);
        let id2 = state.add_block("constant", serde_json::json!({}), 0.0, 0.0);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_remove_block() {
        let mut state = GraphState::new();
        let id = state.add_block("constant", serde_json::json!({}), 0.0, 0.0);
        assert!(state.remove_block(id));
        assert!(state.blocks().is_empty());
        // Removing a nonexistent id returns false.
        assert!(!state.remove_block(id));
    }

    #[test]
    fn test_update_config() {
        let mut state = GraphState::new();
        let id = state.add_block("constant", serde_json::json!({"value": 1.0}), 0.0, 0.0);
        let r_before = state.revision();
        state.update_config(id, "value", serde_json::json!(42.0));
        assert!(state.revision() > r_before);
        let pb = state.blocks().iter().find(|b| b.id == id).unwrap();
        assert_eq!(pb.config["value"], 42.0);
    }

    #[test]
    fn test_build_dag_empty_returns_error() {
        let state = GraphState::new();
        assert!(state.build_dag().is_err());
    }

    #[test]
    fn test_build_dag_with_constant() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 5.0, "publish_topic": "out"}),
            0.0,
            0.0,
        );
        let dag = state.build_dag().unwrap();
        assert!(!dag.is_empty());
    }

    #[test]
    fn test_tick_updates_topics() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 5.0, "publish_topic": "out"}),
            0.0,
            0.0,
        );
        state.tick().unwrap();
        assert!(state.topics().contains_key("out"));
        assert_eq!(state.topics()["out"], 5.0);
        assert_eq!(state.tick_count(), 1);
    }

    #[test]
    fn test_tick_increments_tick_count() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 1.0, "publish_topic": "x"}),
            0.0,
            0.0,
        );
        state.tick().unwrap();
        state.tick().unwrap();
        state.tick().unwrap();
        assert_eq!(state.tick_count(), 3);
    }

    #[test]
    fn test_reset_clears_simulation() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 5.0, "publish_topic": "out"}),
            0.0,
            0.0,
        );
        state.tick().unwrap();
        assert_eq!(state.tick_count(), 1);
        assert!(!state.topics().is_empty());

        state.reset();
        assert_eq!(state.tick_count(), 0);
        assert!(state.topics().is_empty());
    }

    #[test]
    fn test_output_values_for_block() {
        let mut state = GraphState::new();
        let id = state.add_block(
            "constant",
            serde_json::json!({"value": 3.14, "publish_topic": "pi"}),
            0.0,
            0.0,
        );
        // Before any tick, output values are empty strings.
        let vals = state.output_values_for_block(id);
        assert_eq!(vals, vec![String::new()]);

        state.tick().unwrap();
        let vals = state.output_values_for_block(id);
        assert_eq!(vals.len(), 1);
        assert_eq!(vals[0], "3.1400");
    }

    #[test]
    fn test_output_values_nonexistent_block() {
        let state = GraphState::new();
        assert!(state.output_values_for_block(999).is_empty());
    }

    #[test]
    fn test_multiple_blocks_tick() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 2.0, "publish_topic": "a"}),
            0.0,
            0.0,
        );
        state.add_block(
            "constant",
            serde_json::json!({"value": 3.0, "publish_topic": "b"}),
            100.0,
            0.0,
        );
        state.tick().unwrap();
        assert_eq!(state.topics().len(), 2);
        assert_eq!(state.topics()["a"], 2.0);
        assert_eq!(state.topics()["b"], 3.0);
    }

    #[test]
    fn test_default_is_new() {
        let state = GraphState::default();
        assert!(state.blocks().is_empty());
        assert_eq!(state.tick_count(), 0);
    }
}
