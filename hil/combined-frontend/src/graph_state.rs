//! Persistent DAG editor state that survives tab switches.
//!
//! [`GraphState`] holds reactive signals for the block canvas, selection, and
//! project management. It is created once in `App` and provided via Leptos
//! context so the editor component can read/write it without owning it.
//!
//! This module deliberately avoids `web_sys` view types so it compiles and
//! tests on native targets.

use configurable_blocks::lower;
use configurable_blocks::registry;
use leptos::prelude::*;

use crate::types::BlockSet;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Instance of a placed block on the canvas.
///
/// Stores block type + config as serializable data (Send+Sync safe).
/// The trait object is reconstructed from the registry when needed.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacedBlock {
    pub id: usize,
    pub block_type: String,
    pub config: serde_json::Value,
    pub x: f64,
    pub y: f64,
}

impl PlacedBlock {
    /// Reconstruct the ConfigurableBlock trait object from the registry.
    pub fn reconstruct(&self) -> Option<Box<dyn lower::ConfigurableBlock>> {
        let mut block = registry::create_block(&self.block_type)?;
        block.apply_config(&self.config);
        Some(block)
    }
}

/// Serializable project snapshot for localStorage persistence.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectSnapshot {
    pub blocks: Vec<PlacedBlock>,
    pub next_id: usize,
}

/// localStorage key prefix for saved projects.
const STORAGE_PREFIX: &str = "dag_project_";

/// localStorage key for the auto-save slot.
const AUTOSAVE_KEY: &str = "dag_autosave";

// ---------------------------------------------------------------------------
// GraphState
// ---------------------------------------------------------------------------

/// Reactive graph state that survives tab switches.
///
/// Created once in `App` and provided via `provide_context`. The editor reads
/// it with `use_context::<GraphState>()`.
#[derive(Clone)]
pub struct GraphState {
    /// All placed blocks on the canvas.
    pub blocks: ReadSignal<Vec<PlacedBlock>>,
    pub set_blocks: WriteSignal<Vec<PlacedBlock>>,
    /// Monotonic ID counter for new blocks.
    pub next_id: ReadSignal<usize>,
    pub set_next_id: WriteSignal<usize>,
    /// Currently selected block id.
    pub selected_id: ReadSignal<Option<usize>>,
    pub set_selected_id: WriteSignal<Option<usize>>,
    /// Revision counter incremented on every mutation (for auto-save debounce).
    pub revision: ReadSignal<u64>,
    set_revision: WriteSignal<u64>,
    /// Current project name (None = unsaved / new).
    pub project_name: ReadSignal<Option<String>>,
    pub set_project_name: WriteSignal<Option<String>>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphState {
    /// Create a new GraphState with empty canvas.
    pub fn new() -> Self {
        let (blocks, set_blocks) = signal(Vec::<PlacedBlock>::new());
        let (next_id, set_next_id) = signal(1_usize);
        let (selected_id, set_selected_id) = signal(None::<usize>);
        let (revision, set_revision) = signal(0_u64);
        let (project_name, set_project_name) = signal(None::<String>);

        Self {
            blocks,
            set_blocks,
            next_id,
            set_next_id,
            selected_id,
            set_selected_id,
            revision,
            set_revision,
            project_name,
            set_project_name,
        }
    }

    /// Bump the revision counter (triggers auto-save).
    pub fn bump_revision(&self) {
        self.set_revision.set(self.revision.get_untracked() + 1);
    }

    /// Add a block of the given type to the canvas. Returns the new block id.
    pub fn add_block(&self, block_type: &str) -> Option<usize> {
        let block = registry::create_block(block_type)?;
        let id = self.next_id.get_untracked();
        self.set_next_id.set(id + 1);
        let count = self.blocks.get_untracked().len();
        let x = 30.0 + (count % 3) as f64 * 220.0;
        let y = 30.0 + (count / 3) as f64 * 120.0;
        let config = block.config_json();
        self.set_blocks.update(|v| {
            v.push(PlacedBlock {
                id,
                block_type: block_type.to_string(),
                config,
                x,
                y,
            });
        });
        self.set_selected_id.set(Some(id));
        self.bump_revision();
        Some(id)
    }

    /// Update a config key on the currently selected block.
    pub fn update_config(&self, key: String, value: serde_json::Value) {
        let sel = match self.selected_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        self.set_blocks.update(|blks| {
            if let Some(pb) = blks.iter_mut().find(|b| b.id == sel) {
                if let serde_json::Value::Object(ref mut map) = pb.config {
                    map.insert(key, value);
                }
            }
        });
        self.bump_revision();
        publish_block_updated(&self.blocks, sel);
    }

    /// Move a block to a new position.
    pub fn move_block(&self, id: usize, x: f64, y: f64) {
        self.set_blocks.update(|blks| {
            if let Some(pb) = blks.iter_mut().find(|b| b.id == id) {
                pb.x = x;
                pb.y = y;
            }
        });
        // Don't bump revision on every drag frame — caller bumps on drag end.
    }

    /// Disconnect an edge by clearing the auto-topic on the source block's output.
    ///
    /// Edges are "virtual" — they exist because two blocks share a matching topic
    /// name. Disconnecting means clearing the source's output topic to break the
    /// name match.
    pub fn disconnect_edge(&self, from_block_id: usize, from_port: usize) {
        use configurable_blocks::schema::ChannelDirection;

        self.set_blocks.update(|blks| {
            let pb = match blks.iter().find(|b| b.id == from_block_id) {
                Some(b) => b.clone(),
                None => return,
            };
            let ch = match channel_at(&pb, ChannelDirection::Output, from_port) {
                Some(c) => c,
                None => return,
            };
            if let Some(key) = find_config_key_for_channel(&pb.config, &ch.name) {
                if let Some(src) = blks.iter_mut().find(|b| b.id == from_block_id) {
                    if let serde_json::Value::Object(ref mut map) = src.config {
                        map.insert(key, serde_json::Value::String(String::new()));
                    }
                }
            }
        });
        self.set_selected_id.set(None);
        self.bump_revision();
    }

    /// Connect an output port to an input port by writing a shared auto-topic
    /// into both blocks' config JSON.
    ///
    /// Edges in this model are "virtual": two blocks share a matching topic
    /// name. This method generates `wire_{from_block}_{from_port}` and sets it
    /// on both the source block's output config key and the destination block's
    /// input config key. On success, bumps the revision (triggers auto-save).
    ///
    /// Returns `Err` with a short description on self-loop, missing block,
    /// out-of-range port, type mismatch, or missing config key.
    pub fn connect_edge(
        &self,
        from_block: usize,
        from_port: usize,
        to_block: usize,
        to_port: usize,
    ) -> Result<(), String> {
        use configurable_blocks::schema::ChannelDirection;

        if from_block == to_block {
            return Err("self-loop".to_string());
        }

        let blks = self.blocks.get_untracked();
        let src_pb = blks
            .iter()
            .find(|b| b.id == from_block)
            .ok_or_else(|| "source block not found".to_string())?
            .clone();
        let dst_pb = blks
            .iter()
            .find(|b| b.id == to_block)
            .ok_or_else(|| "dest block not found".to_string())?
            .clone();
        drop(blks);

        let src_ch = channel_at(&src_pb, ChannelDirection::Output, from_port)
            .ok_or_else(|| "source port out of range".to_string())?;
        let dst_ch = channel_at(&dst_pb, ChannelDirection::Input, to_port)
            .ok_or_else(|| "dest port out of range".to_string())?;

        if !types_compatible(&src_ch.channel_type, &dst_ch.channel_type) {
            let s = src_ch.channel_type.as_deref().unwrap_or("any");
            let d = dst_ch.channel_type.as_deref().unwrap_or("any");
            return Err(format!("type mismatch: {s} -> {d}"));
        }

        let src_key = find_config_key_for_channel(&src_pb.config, &src_ch.name)
            .ok_or_else(|| "source config key missing".to_string())?;
        let dst_key = find_config_key_for_channel(&dst_pb.config, &dst_ch.name)
            .ok_or_else(|| "dest config key missing".to_string())?;

        let auto_topic = format!("wire_{from_block}_{from_port}");

        self.set_blocks.update(|blks| {
            if let Some(src_mut) = blks.iter_mut().find(|b| b.id == from_block) {
                if let serde_json::Value::Object(ref mut map) = src_mut.config {
                    map.insert(src_key, serde_json::Value::String(auto_topic.clone()));
                }
            }
            if let Some(dst_mut) = blks.iter_mut().find(|b| b.id == to_block) {
                if let serde_json::Value::Object(ref mut map) = dst_mut.config {
                    map.insert(dst_key, serde_json::Value::String(auto_topic.clone()));
                }
            }
        });
        self.bump_revision();
        publish_connection_created(from_block, from_port, to_block, to_port);
        Ok(())
    }

    /// Delete the currently selected block.
    pub fn delete_selected(&self) {
        if let Some(sel) = self.selected_id.get_untracked() {
            self.set_blocks.update(|v| v.retain(|b| b.id != sel));
            self.set_selected_id.set(None);
            self.bump_revision();
        }
    }

    /// Clear all blocks and reset state for a new project.
    pub fn clear(&self) {
        self.set_blocks.set(Vec::new());
        self.set_next_id.set(1);
        self.set_selected_id.set(None);
        self.set_project_name.set(None);
        self.bump_revision();
    }

    /// Build a `BlockSet` (for the deploy panel bridge).
    pub fn to_block_set(&self) -> BlockSet {
        self.blocks
            .get_untracked()
            .iter()
            .map(|pb| (pb.block_type.clone(), pb.config.clone()))
            .collect()
    }

    // -----------------------------------------------------------------------
    // localStorage persistence
    // -----------------------------------------------------------------------

    /// Save current state to localStorage under the given project name.
    pub fn save_to_storage(&self, name: &str) {
        let snapshot = ProjectSnapshot {
            blocks: self.blocks.get_untracked(),
            next_id: self.next_id.get_untracked(),
        };
        let json = match serde_json::to_string(&snapshot) {
            Ok(j) => j,
            Err(_) => return,
        };
        let key = format!("{}{}", STORAGE_PREFIX, name);
        let _ = set_local_storage(&key, &json);
        self.set_project_name.set(Some(name.to_string()));
    }

    /// Auto-save to the dedicated auto-save slot.
    pub fn auto_save(&self) {
        let snapshot = ProjectSnapshot {
            blocks: self.blocks.get_untracked(),
            next_id: self.next_id.get_untracked(),
        };
        if let Ok(json) = serde_json::to_string(&snapshot) {
            let _ = set_local_storage(AUTOSAVE_KEY, &json);
        }
    }

    /// Load a project from localStorage by name.
    pub fn load_from_storage(&self, name: &str) {
        let key = format!("{}{}", STORAGE_PREFIX, name);
        let json = match get_local_storage(&key) {
            Some(j) => j,
            None => return,
        };
        let snapshot: ProjectSnapshot = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(_) => return,
        };
        self.set_blocks.set(snapshot.blocks);
        self.set_next_id.set(snapshot.next_id);
        self.set_selected_id.set(None);
        self.set_project_name.set(Some(name.to_string()));
        self.bump_revision();
    }

    /// Try to restore from auto-save on startup.
    pub fn restore_autosave(&self) {
        let json = match get_local_storage(AUTOSAVE_KEY) {
            Some(j) => j,
            None => return,
        };
        let snapshot: ProjectSnapshot = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(_) => return,
        };
        if !snapshot.blocks.is_empty() {
            self.set_blocks.set(snapshot.blocks);
            self.set_next_id.set(snapshot.next_id);
        }
    }

    /// Delete a project from localStorage.
    pub fn delete_project(name: &str) {
        let key = format!("{}{}", STORAGE_PREFIX, name);
        let _ = remove_local_storage(&key);
    }

    /// List all saved project names.
    pub fn list_projects() -> Vec<String> {
        list_local_storage_keys(STORAGE_PREFIX)
    }

    /// Create a new empty project in localStorage with an auto-generated name,
    /// leaving the in-memory canvas untouched. Returns the new project's name.
    ///
    /// "New" in most editors means "wipe the canvas"; here the user asked for
    /// the opposite: create a new entry in the sidebar without discarding the
    /// current document. Pair this with `save_to_storage(current_name)` in the
    /// caller to checkpoint unsaved work before advancing.
    pub fn create_new_project(&self) -> String {
        let name = next_project_name();
        let snapshot = ProjectSnapshot {
            blocks: Vec::new(),
            next_id: 1,
        };
        if let Ok(json) = serde_json::to_string(&snapshot) {
            let key = format!("{STORAGE_PREFIX}{name}");
            let _ = set_local_storage(&key, &json);
        }
        name
    }
}

/// Pick the lowest unused "project N" name among existing projects. Scans
/// only names that match the `project <n>` pattern so user-chosen names
/// (e.g. "experiment-2025") never collide.
fn next_project_name() -> String {
    let taken: std::collections::BTreeSet<u32> = GraphState::list_projects()
        .iter()
        .filter_map(|n| n.strip_prefix("project "))
        .filter_map(|rest| rest.parse::<u32>().ok())
        .collect();
    let mut n = 1_u32;
    while taken.contains(&n) {
        n += 1;
    }
    format!("project {n}")
}

// ---------------------------------------------------------------------------
// localStorage helpers (no-op on non-wasm)
// ---------------------------------------------------------------------------

fn get_local_storage(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok().flatten()?;
        storage.get_item(key).ok().flatten()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        None
    }
}

fn set_local_storage(key: &str, value: &str) -> Result<(), ()> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().ok_or(())?;
        let storage = window.local_storage().map_err(|_| ())?.ok_or(())?;
        storage.set_item(key, value).map_err(|_| ())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (key, value);
        Err(())
    }
}

fn remove_local_storage(key: &str) -> Result<(), ()> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().ok_or(())?;
        let storage = window.local_storage().map_err(|_| ())?.ok_or(())?;
        storage.remove_item(key).map_err(|_| ())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        Err(())
    }
}

fn list_local_storage_keys(prefix: &str) -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return Vec::new(),
        };
        let storage = match window.local_storage() {
            Ok(Some(s)) => s,
            _ => return Vec::new(),
        };
        let len = match storage.length() {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };
        let mut names = Vec::new();
        for i in 0..len {
            if let Ok(Some(key)) = storage.key(i) {
                if let Some(name) = key.strip_prefix(prefix) {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        names
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = prefix;
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Pure helper functions used by the editor
// ---------------------------------------------------------------------------

/// Find the config key whose current value matches `channel_name`.
///
/// Blocks store channel topic names as string values in their config JSON.
/// For example, `{"input_topic": "add/a", "output_topic": "add/out"}`.
/// Given channel_name="add/a", this returns Some("input_topic").
pub fn find_config_key_for_channel(
    config: &serde_json::Value,
    channel_name: &str,
) -> Option<String> {
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

/// Reconstruct a block, filter by direction, and return the nth declared channel.
fn channel_at(
    pb: &PlacedBlock,
    dir: configurable_blocks::schema::ChannelDirection,
    idx: usize,
) -> Option<configurable_blocks::schema::DeclaredChannel> {
    let block = pb.reconstruct()?;
    block
        .declared_channels()
        .into_iter()
        .filter(|c| c.direction == dir)
        .nth(idx)
}

/// Decide whether two port type tags can be wired together.
///
/// Rule: if both sides declare a concrete type, they must match exactly.
/// If either side is `None` (untyped — currently the default for all blocks),
/// the connection is permitted.
fn types_compatible(src: &Option<String>, dst: &Option<String>) -> bool {
    match (src, dst) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

/// Publish a `TelemetryBlockUpdated` CBOR frame over the WebSocket. Best-effort
/// — silently no-ops if no server is connected. Only active under WASM; on
/// native targets this is a no-op so unit tests do not depend on `web_sys`.
fn publish_block_updated(blocks: &ReadSignal<Vec<PlacedBlock>>, block_id: usize) {
    #[cfg(target_arch = "wasm32")]
    {
        let blks = blocks.get_untracked();
        if let Some(pb) = blks.iter().find(|b| b.id == block_id) {
            let config_json = serde_json::to_string(&pb.config).unwrap_or_default();
            let _ =
                crate::ws_client::send_request(&crate::messages::Request::TelemetryBlockUpdated {
                    block_id: pb.id as u32,
                    block_type: pb.block_type.clone(),
                    config_json,
                });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (blocks, block_id);
    }
}

/// Publish a `TelemetryConnectionCreated` CBOR frame. Best-effort; no-op on
/// native. `channel_id` is a deterministic pack of the four endpoints so the
/// same edge always reports the same id within a session.
fn publish_connection_created(
    from_block: usize,
    from_port: usize,
    to_block: usize,
    to_port: usize,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let channel_id = ((from_block as u32 & 0xFF) << 24)
            | ((from_port as u32 & 0xFF) << 16)
            | ((to_block as u32 & 0xFF) << 8)
            | (to_port as u32 & 0xFF);
        let _ =
            crate::ws_client::send_request(&crate::messages::Request::TelemetryConnectionCreated {
                from_block: from_block as u32,
                from_port: from_port as u32,
                to_block: to_block as u32,
                to_port: to_port as u32,
                channel_id,
            });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (from_block, from_port, to_block, to_port);
    }
}

/// Offset all NodeId references in an Op by a given amount.
pub fn offset_op(op: &dag_core::op::Op, offset: u16) -> dag_core::op::Op {
    use dag_core::op::Op;
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_state_add_block_increments_id() {
        let gs = GraphState::new();
        let id1 = gs.add_block("constant");
        let id2 = gs.add_block("constant");
        assert!(id1.is_some());
        assert!(id2.is_some());
        assert_ne!(id1, id2);
        assert_eq!(gs.blocks.get_untracked().len(), 2);
    }

    #[test]
    fn graph_state_delete_selected() {
        let gs = GraphState::new();
        let id = gs.add_block("constant");
        assert!(id.is_some());
        assert_eq!(gs.blocks.get_untracked().len(), 1);
        gs.delete_selected();
        assert_eq!(gs.blocks.get_untracked().len(), 0);
        assert_eq!(gs.selected_id.get_untracked(), None);
    }

    #[test]
    fn graph_state_clear() {
        let gs = GraphState::new();
        gs.add_block("constant");
        gs.add_block("constant");
        assert_eq!(gs.blocks.get_untracked().len(), 2);
        gs.clear();
        assert_eq!(gs.blocks.get_untracked().len(), 0);
        assert_eq!(gs.next_id.get_untracked(), 1);
        assert_eq!(gs.selected_id.get_untracked(), None);
        assert_eq!(gs.project_name.get_untracked(), None);
    }

    #[test]
    fn graph_state_update_config() {
        let gs = GraphState::new();
        gs.add_block("constant");
        gs.update_config("value".to_string(), serde_json::json!(42.0));
        let blks = gs.blocks.get_untracked();
        let config = &blks[0].config;
        assert_eq!(config.get("value"), Some(&serde_json::json!(42.0)));
    }

    #[test]
    fn graph_state_to_block_set() {
        let gs = GraphState::new();
        gs.add_block("constant");
        let bs = gs.to_block_set();
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].0, "constant");
    }

    #[test]
    fn graph_state_revision_increments() {
        let gs = GraphState::new();
        let r0 = gs.revision.get_untracked();
        gs.add_block("constant");
        let r1 = gs.revision.get_untracked();
        assert!(r1 > r0);
        gs.delete_selected();
        let r2 = gs.revision.get_untracked();
        assert!(r2 > r1);
    }

    #[test]
    fn next_project_name_picks_lowest_free_slot() {
        // On non-wasm, list_projects() returns empty, so the first name is always
        // "project 1". The interesting collision cases live behind the wasm cfg,
        // so here we just verify the fallback name and that the helper is pure.
        assert_eq!(next_project_name(), "project 1");
        // Repeated calls with no state change produce the same name (no side
        // effects until create_new_project actually writes storage).
        assert_eq!(next_project_name(), "project 1");
    }

    #[test]
    fn create_new_project_leaves_graph_state_untouched() {
        let gs = GraphState::new();
        gs.add_block("constant");
        let blocks_before = gs.blocks.get_untracked().len();
        let next_id_before = gs.next_id.get_untracked();
        let name_before = gs.project_name.get_untracked();

        let created = gs.create_new_project();
        // Name scheme is "project N" — non-empty and prefixed.
        assert!(created.starts_with("project "));

        // In-memory state is unchanged; the current document survives.
        assert_eq!(gs.blocks.get_untracked().len(), blocks_before);
        assert_eq!(gs.next_id.get_untracked(), next_id_before);
        assert_eq!(gs.project_name.get_untracked(), name_before);
    }

    #[test]
    fn project_snapshot_round_trip() {
        let snap = ProjectSnapshot {
            blocks: vec![PlacedBlock {
                id: 1,
                block_type: "constant".to_string(),
                config: serde_json::json!({"value": 3.14}),
                x: 10.0,
                y: 20.0,
            }],
            next_id: 2,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let restored: ProjectSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.blocks.len(), 1);
        assert_eq!(restored.blocks[0].id, 1);
        assert_eq!(restored.next_id, 2);
    }

    #[test]
    fn list_projects_returns_empty_on_native() {
        // On non-wasm, localStorage is unavailable, so list returns empty.
        assert!(GraphState::list_projects().is_empty());
    }

    #[test]
    fn find_config_key_matches() {
        let config = serde_json::json!({"input_topic": "add/a", "output_topic": "add/out"});
        assert_eq!(
            find_config_key_for_channel(&config, "add/a"),
            Some("input_topic".to_string())
        );
        assert_eq!(
            find_config_key_for_channel(&config, "add/out"),
            Some("output_topic".to_string())
        );
        assert_eq!(find_config_key_for_channel(&config, "missing"), None);
    }

    #[test]
    fn offset_op_adds_offset() {
        use dag_core::op::Op;
        let op = Op::Add(0, 1);
        let adjusted = offset_op(&op, 5);
        assert_eq!(adjusted, Op::Add(5, 6));
    }

    #[test]
    fn offset_op_const_unchanged() {
        use dag_core::op::Op;
        let op = Op::Const(3.14);
        let adjusted = offset_op(&op, 10);
        assert_eq!(adjusted, Op::Const(3.14));
    }

    #[test]
    fn graph_state_default_matches_new() {
        let gs = GraphState::default();
        assert_eq!(gs.blocks.get_untracked().len(), 0);
        assert_eq!(gs.next_id.get_untracked(), 1);
        assert_eq!(gs.selected_id.get_untracked(), None);
    }

    #[test]
    fn placed_block_reconstruct_unknown_returns_none() {
        let pb = PlacedBlock {
            id: 1,
            block_type: "nonexistent_block_type".to_string(),
            config: serde_json::json!({}),
            x: 0.0,
            y: 0.0,
        };
        assert!(pb.reconstruct().is_none());
    }

    #[test]
    fn graph_state_add_unknown_block_returns_none() {
        let gs = GraphState::new();
        let result = gs.add_block("totally_made_up");
        assert!(result.is_none());
        assert_eq!(gs.blocks.get_untracked().len(), 0);
    }

    #[test]
    fn graph_state_delete_with_no_selection_is_noop() {
        let gs = GraphState::new();
        gs.add_block("constant");
        gs.set_selected_id.set(None);
        let rev_before = gs.revision.get_untracked();
        gs.delete_selected();
        // Nothing deleted, revision should not change
        assert_eq!(gs.blocks.get_untracked().len(), 1);
        assert_eq!(gs.revision.get_untracked(), rev_before);
    }

    #[test]
    fn graph_state_update_config_no_selection_is_noop() {
        let gs = GraphState::new();
        gs.add_block("constant");
        gs.set_selected_id.set(None);
        let rev_before = gs.revision.get_untracked();
        gs.update_config("value".to_string(), serde_json::json!(99.0));
        // Config should not change, revision should not change
        assert_eq!(gs.revision.get_untracked(), rev_before);
    }

    #[test]
    fn graph_state_move_block() {
        let gs = GraphState::new();
        let id = gs.add_block("constant").unwrap();
        let blks = gs.blocks.get_untracked();
        let orig_x = blks[0].x;
        let orig_y = blks[0].y;
        drop(blks);

        gs.move_block(id, 300.0, 400.0);

        let blks = gs.blocks.get_untracked();
        assert!((blks[0].x - 300.0).abs() < 0.01);
        assert!((blks[0].y - 400.0).abs() < 0.01);
        assert!((blks[0].x - orig_x).abs() > 1.0); // actually moved
    }

    #[test]
    fn graph_state_move_nonexistent_block_is_noop() {
        let gs = GraphState::new();
        gs.add_block("constant");
        gs.move_block(999, 100.0, 200.0); // no panic
                                          // Block at id=1 should not have moved
        let blks = gs.blocks.get_untracked();
        assert!(blks[0].x < 100.0); // still at original position
    }

    /// Build a two-block graph (constant -> add) with `constant.publish_topic` set so
    /// that `constant` declares an output port at index 0. Returns `(gs, constant_id, add_id)`.
    fn two_block_graph() -> (GraphState, usize, usize) {
        let gs = GraphState::new();
        let const_id = gs.add_block("constant").expect("constant registered");
        // Select the constant and give it a non-empty publish_topic so it declares an output.
        gs.set_selected_id.set(Some(const_id));
        gs.update_config("publish_topic".to_string(), serde_json::json!("const_out"));
        let add_id = gs.add_block("add").expect("add registered");
        (gs, const_id, add_id)
    }

    #[test]
    fn connect_edge_happy_path() {
        let (gs, from, to) = two_block_graph();
        let rev0 = gs.revision.get_untracked();

        gs.connect_edge(from, 0, to, 0).expect("connects");

        let blks = gs.blocks.get_untracked();
        let src = blks.iter().find(|b| b.id == from).unwrap();
        let dst = blks.iter().find(|b| b.id == to).unwrap();
        let expected = format!("wire_{from}_0");
        assert_eq!(
            src.config.get("publish_topic").and_then(|v| v.as_str()),
            Some(expected.as_str()),
            "source output topic should be auto-wire name"
        );
        assert_eq!(
            dst.config.get("input_a_topic").and_then(|v| v.as_str()),
            Some(expected.as_str()),
            "dest input_a_topic should match auto-wire name"
        );
        assert!(
            gs.revision.get_untracked() > rev0,
            "revision should bump on connect"
        );
    }

    #[test]
    fn connect_edge_self_loop_rejected() {
        let (gs, from, _to) = two_block_graph();
        let err = gs.connect_edge(from, 0, from, 0).unwrap_err();
        assert_eq!(err, "self-loop");
    }

    #[test]
    fn connect_edge_missing_source_block() {
        let (gs, _from, to) = two_block_graph();
        let err = gs.connect_edge(999, 0, to, 0).unwrap_err();
        assert_eq!(err, "source block not found");
    }

    #[test]
    fn connect_edge_missing_dest_block() {
        let (gs, from, _to) = two_block_graph();
        let err = gs.connect_edge(from, 0, 999, 0).unwrap_err();
        assert_eq!(err, "dest block not found");
    }

    #[test]
    fn connect_edge_source_port_out_of_range() {
        let (gs, from, to) = two_block_graph();
        let err = gs.connect_edge(from, 99, to, 0).unwrap_err();
        assert_eq!(err, "source port out of range");
    }

    #[test]
    fn connect_edge_dest_port_out_of_range() {
        let (gs, from, to) = two_block_graph();
        let err = gs.connect_edge(from, 0, to, 99).unwrap_err();
        assert_eq!(err, "dest port out of range");
    }

    #[test]
    fn types_compatible_rule() {
        // Both None: permitted.
        assert!(types_compatible(&None, &None));
        // One side typed, other None: permitted.
        assert!(types_compatible(&Some("f64".into()), &None));
        assert!(types_compatible(&None, &Some("f64".into())));
        // Both typed, same: permitted.
        assert!(types_compatible(&Some("f64".into()), &Some("f64".into())));
        // Both typed, different: rejected.
        assert!(!types_compatible(&Some("f64".into()), &Some("bool".into())));
    }

    #[test]
    fn graph_state_disconnect_edge() {
        let gs = GraphState::new();
        // Add a pubsub_bridge block (has publish_topic as its output channel)
        gs.add_block("pubsub_bridge");
        // Set its publish_topic to "my_wire"
        gs.update_config("publish_topic".to_string(), serde_json::json!("my_wire"));

        // Verify the publish topic is set
        let blks = gs.blocks.get_untracked();
        assert_eq!(
            blks[0].config.get("publish_topic").and_then(|v| v.as_str()),
            Some("my_wire")
        );
        drop(blks);

        // Disconnect output port 0 — should clear the publish topic
        let block_id = 1;
        gs.disconnect_edge(block_id, 0);

        let blks = gs.blocks.get_untracked();
        let output_val = blks[0]
            .config
            .get("publish_topic")
            .and_then(|v| v.as_str())
            .unwrap_or("not_found");
        assert_eq!(output_val, "", "publish topic should be cleared");
    }
}
