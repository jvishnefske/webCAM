//! Reactive wrapper around [`GraphEngine`].
//!
//! All mutations bump the `revision` counter, which Leptos Effects/Memos
//! can watch to trigger UI updates. The struct is stored in a thread_local
//! `RefCell` (same pattern as `GraphEngine` was) and accessed via helper
//! functions.

use std::collections::{BTreeMap, HashMap};

use crate::graph_engine::GraphEngine;

/// Reactive wrapper around [`GraphEngine`].
///
/// Every mutating method increments the internal `revision` counter so that
/// Leptos signals can detect state changes cheaply. UI-only state (block
/// positions, selection) lives here rather than in the engine.
pub struct GraphState {
    engine: GraphEngine,
    /// Monotonically increasing revision counter -- bumped on every mutation.
    revision: u64,
    /// Block positions (UI-only, not stored in engine).
    positions: HashMap<u32, (f64, f64)>,
    /// Currently selected block id.
    selected_block: Option<u32>,
    /// Currently selected edge/channel id.
    selected_edge: Option<u32>,
}

impl GraphState {
    /// Create a new empty state with revision 0.
    pub fn new() -> Self {
        Self {
            engine: GraphEngine::new(),
            revision: 0,
            positions: HashMap::new(),
            selected_block: None,
            selected_edge: None,
        }
    }

    /// Current revision number. Leptos signals watch this for changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // -- Block CRUD -------------------------------------------------------

    /// Add a block and record its canvas position.
    ///
    /// Returns `None` if the block type is unknown (revision is **not**
    /// bumped on failure).
    pub fn add_block(
        &mut self,
        block_type: &str,
        config: serde_json::Value,
        x: f64,
        y: f64,
    ) -> Option<u32> {
        let id = self.engine.add_block(block_type, config)?;
        self.positions.insert(id, (x, y));
        self.revision += 1;
        Some(id)
    }

    /// Remove a block, its position, and clear selection if it was selected.
    pub fn remove_block(&mut self, id: u32) {
        self.engine.remove_block(id);
        self.positions.remove(&id);
        if self.selected_block == Some(id) {
            self.selected_block = None;
        }
        self.revision += 1;
    }

    /// Update a single config key on a block.
    pub fn update_config(&mut self, id: u32, key: String, value: serde_json::Value) {
        self.engine.update_config(id, key, value);
        self.revision += 1;
    }

    // -- Channel CRUD -----------------------------------------------------

    /// Connect an output port to an input port. Returns the channel id.
    pub fn connect(
        &mut self,
        from_block: u32,
        from_port: usize,
        to_block: u32,
        to_port: usize,
    ) -> Option<u32> {
        let id = self
            .engine
            .connect(from_block, from_port, to_block, to_port)?;
        self.revision += 1;
        Some(id)
    }

    /// Disconnect a channel. Returns `true` if a channel was actually removed.
    pub fn disconnect(&mut self, channel_id: u32) -> bool {
        let ok = self.engine.disconnect(channel_id);
        if ok {
            self.revision += 1;
        }
        ok
    }

    // -- Simulation -------------------------------------------------------

    /// Evaluate one simulation tick.
    pub fn tick(&mut self) -> Result<(), String> {
        self.engine.tick()?;
        self.revision += 1;
        Ok(())
    }

    /// Reset the simulation, positions, and selection.
    pub fn reset(&mut self) {
        self.engine.reset_sim();
        self.positions.clear();
        self.selected_block = None;
        self.selected_edge = None;
        self.revision += 1;
    }

    /// Inject a topic value for the next tick.
    ///
    /// Does **not** bump revision -- inject is for live mode where `tick()`
    /// bumps it.
    pub fn inject_topic(&mut self, topic: &str, value: f64) {
        self.engine.inject_topic(topic, value);
    }

    /// Read the current value of a topic.
    pub fn read_topic(&self, topic: &str) -> Option<f64> {
        self.engine.read_topic(topic)
    }

    // -- Selection --------------------------------------------------------

    /// Currently selected block id.
    pub fn selected_block(&self) -> Option<u32> {
        self.selected_block
    }

    /// Set the selected block (bumps revision).
    pub fn set_selected_block(&mut self, id: Option<u32>) {
        self.selected_block = id;
        self.revision += 1;
    }

    /// Currently selected edge/channel id.
    pub fn selected_edge(&self) -> Option<u32> {
        self.selected_edge
    }

    /// Set the selected edge (bumps revision).
    pub fn set_selected_edge(&mut self, id: Option<u32>) {
        self.selected_edge = id;
        self.revision += 1;
    }

    // -- Read-only accessors ----------------------------------------------

    /// Block positions on the canvas.
    pub fn positions(&self) -> &HashMap<u32, (f64, f64)> {
        &self.positions
    }

    /// Set a block's canvas position (bumps revision).
    pub fn set_position(&mut self, id: u32, x: f64, y: f64) {
        self.positions.insert(id, (x, y));
        self.revision += 1;
    }

    /// Current simulation tick count.
    pub fn tick_count(&self) -> u64 {
        self.engine.tick_count()
    }

    /// All current pubsub topic values.
    pub fn topics(&self) -> BTreeMap<String, f64> {
        self.engine.topics()
    }

    /// Borrow the underlying engine (read-only).
    pub fn engine(&self) -> &GraphEngine {
        &self.engine
    }

    /// Borrow the underlying engine (mutable).
    pub fn engine_mut(&mut self) -> &mut GraphEngine {
        &mut self.engine
    }
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_revision_zero() {
        let state = GraphState::new();
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn test_add_block_bumps_revision() {
        let mut state = GraphState::new();
        state.add_block("constant", serde_json::json!({"value": 1.0}), 10.0, 20.0);
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn test_add_block_stores_position() {
        let mut state = GraphState::new();
        let id = state
            .add_block("constant", serde_json::json!({}), 50.0, 100.0)
            .unwrap();
        assert_eq!(state.positions().get(&id), Some(&(50.0, 100.0)));
    }

    #[test]
    fn test_remove_block_bumps_revision() {
        let mut state = GraphState::new();
        let id = state
            .add_block("constant", serde_json::json!({}), 0.0, 0.0)
            .unwrap();
        let rev = state.revision();
        state.remove_block(id);
        assert!(state.revision() > rev);
    }

    #[test]
    fn test_remove_block_clears_selection() {
        let mut state = GraphState::new();
        let id = state
            .add_block("constant", serde_json::json!({}), 0.0, 0.0)
            .unwrap();
        state.set_selected_block(Some(id));
        state.remove_block(id);
        assert_eq!(state.selected_block(), None);
    }

    #[test]
    fn test_connect_disconnect() {
        let mut state = GraphState::new();
        let a = state
            .add_block("pubsub_bridge", serde_json::json!({}), 0.0, 0.0)
            .unwrap();
        let b = state
            .add_block("pubsub_bridge", serde_json::json!({}), 200.0, 0.0)
            .unwrap();
        let ch = state.connect(a, 0, b, 0);
        assert!(ch.is_some());
        let rev = state.revision();
        state.disconnect(ch.unwrap());
        assert!(state.revision() > rev);
    }

    #[test]
    fn test_tick_increments() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 1.0, "publish_topic": "x"}),
            0.0,
            0.0,
        );
        state.tick().unwrap();
        assert_eq!(state.tick_count(), 1);
        assert!(state.revision() > 0);
    }

    #[test]
    fn test_inject_read_topic() {
        let mut state = GraphState::new();
        state.inject_topic("test", 42.0);
        assert_eq!(state.read_topic("test"), Some(42.0));
    }

    #[test]
    fn test_reset_clears_everything() {
        let mut state = GraphState::new();
        state.add_block("constant", serde_json::json!({}), 0.0, 0.0);
        state.set_selected_block(Some(1));
        state.reset();
        assert_eq!(state.selected_block(), None);
        assert_eq!(state.tick_count(), 0);
        assert!(state.positions().is_empty());
    }

    #[test]
    fn test_set_position_bumps_revision() {
        let mut state = GraphState::new();
        let rev = state.revision();
        state.set_position(1, 100.0, 200.0);
        assert!(state.revision() > rev);
    }

    #[test]
    fn test_invalid_block_type_returns_none() {
        let mut state = GraphState::new();
        assert!(state
            .add_block("nonexistent", serde_json::json!({}), 0.0, 0.0)
            .is_none());
        // No revision bump on failure.
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn test_default_impl() {
        let state = GraphState::default();
        assert_eq!(state.revision(), 0);
        assert!(state.positions().is_empty());
    }

    #[test]
    fn test_update_config_bumps_revision() {
        let mut state = GraphState::new();
        let id = state
            .add_block("constant", serde_json::json!({"value": 1.0}), 0.0, 0.0)
            .unwrap();
        let rev = state.revision();
        state.update_config(id, "value".to_string(), serde_json::json!(99.0));
        assert!(state.revision() > rev);
    }

    #[test]
    fn test_set_selected_edge() {
        let mut state = GraphState::new();
        assert_eq!(state.selected_edge(), None);
        state.set_selected_edge(Some(42));
        assert_eq!(state.selected_edge(), Some(42));
        state.set_selected_edge(None);
        assert_eq!(state.selected_edge(), None);
    }

    #[test]
    fn test_engine_accessors() {
        let mut state = GraphState::new();
        // Read-only accessor.
        assert!(state.engine().blocks().is_empty());
        // Mutable accessor.
        state
            .engine_mut()
            .add_block("constant", serde_json::json!({}));
        assert_eq!(state.engine().blocks().len(), 1);
    }

    #[test]
    fn test_disconnect_nonexistent_no_revision_bump() {
        let mut state = GraphState::new();
        let rev = state.revision();
        let ok = state.disconnect(999);
        assert!(!ok);
        assert_eq!(state.revision(), rev);
    }

    #[test]
    fn test_topics_after_tick() {
        let mut state = GraphState::new();
        state.add_block(
            "constant",
            serde_json::json!({"value": 7.5, "publish_topic": "out"}),
            0.0,
            0.0,
        );
        state.tick().unwrap();
        let topics = state.topics();
        assert_eq!(topics.get("out"), Some(&7.5));
    }
}
