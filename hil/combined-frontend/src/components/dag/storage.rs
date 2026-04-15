//! Project persistence via browser `localStorage`.
//!
//! Each project is stored as a JSON string under a key prefixed by
//! `"dag_project:"`. The value is a [`SavedProject`] containing the
//! [`GraphSnapshot`] and per-block positions.

use std::collections::HashMap;

use crate::graph_engine::GraphSnapshot;

/// Key prefix for localStorage entries.
const KEY_PREFIX: &str = "dag_project:";

/// A saved project: graph snapshot + block positions + metadata.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SavedProject {
    pub name: String,
    pub snapshot: GraphSnapshot,
    /// Block id -> (x, y) canvas position.
    pub positions: HashMap<u32, (f64, f64)>,
    /// ISO 8601 timestamp of last save (if available).
    #[serde(default)]
    pub saved_at: String,
}

/// Save a project to localStorage.
///
/// Overwrites any existing project with the same name.
pub fn save_project(project: &SavedProject) -> Result<(), String> {
    let storage = local_storage()?;
    let key = format!("{}{}", KEY_PREFIX, project.name);
    let json = serde_json::to_string(project).map_err(|e| format!("Serialize error: {e}"))?;
    storage
        .set_item(&key, &json)
        .map_err(|_| "localStorage.setItem failed".to_string())
}

/// Load a project by name.
pub fn load_project(name: &str) -> Result<SavedProject, String> {
    let storage = local_storage()?;
    let key = format!("{}{}", KEY_PREFIX, name);
    let json = storage
        .get_item(&key)
        .map_err(|_| "localStorage.getItem failed".to_string())?
        .ok_or_else(|| format!("Project not found: {name}"))?;
    serde_json::from_str(&json).map_err(|e| format!("Deserialize error: {e}"))
}

/// Delete a project by name.
pub fn delete_project(name: &str) -> Result<(), String> {
    let storage = local_storage()?;
    let key = format!("{}{}", KEY_PREFIX, name);
    storage
        .remove_item(&key)
        .map_err(|_| "localStorage.removeItem failed".to_string())
}

/// List all saved project names.
pub fn list_projects() -> Result<Vec<String>, String> {
    let storage = local_storage()?;
    let len = storage
        .length()
        .map_err(|_| "localStorage.length failed".to_string())?;
    let mut names = Vec::new();
    for i in 0..len {
        if let Ok(Some(key)) = storage.key(i) {
            if let Some(name) = key.strip_prefix(KEY_PREFIX) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Get the browser's localStorage, if available.
fn local_storage() -> Result<web_sys::Storage, String> {
    let window = web_sys::window().ok_or("No window")?;
    window
        .local_storage()
        .map_err(|_| "localStorage access denied".to_string())?
        .ok_or_else(|| "localStorage not available".to_string())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_engine::GraphEngine;

    /// Verify SavedProject round-trips through JSON (no localStorage in test env).
    #[test]
    fn test_saved_project_json_roundtrip() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block("constant", serde_json::json!({"value": 3.14}))
            .unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        engine.connect(a, 0, b, 0);

        let mut positions = HashMap::new();
        positions.insert(a, (100.0, 200.0));
        positions.insert(b, (300.0, 200.0));

        let project = SavedProject {
            name: "test-project".into(),
            snapshot: engine.snapshot(),
            positions,
            saved_at: "2026-01-01T00:00:00Z".into(),
        };

        let json = serde_json::to_string(&project).expect("serialize");
        let restored: SavedProject = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.name, "test-project");
        assert_eq!(restored.snapshot.blocks.len(), 2);
        assert_eq!(restored.snapshot.channels.len(), 1);
        assert_eq!(restored.positions.len(), 2);
        assert_eq!(restored.positions[&a], (100.0, 200.0));
        assert_eq!(restored.saved_at, "2026-01-01T00:00:00Z");
    }

    /// Verify round-trip through GraphEngine snapshot -> SavedProject -> restore.
    #[test]
    fn test_save_load_roundtrip_through_engine() {
        let mut engine = GraphEngine::new();
        let a = engine
            .add_block(
                "constant",
                serde_json::json!({"value": 42.0, "publish_topic": "out"}),
            )
            .unwrap();
        let b = engine.add_block("gain", serde_json::json!({})).unwrap();
        let ch = engine.connect(a, 0, b, 0).unwrap();

        let mut positions = HashMap::new();
        positions.insert(a, (50.0, 75.0));
        positions.insert(b, (250.0, 75.0));

        let project = SavedProject {
            name: "roundtrip".into(),
            snapshot: engine.snapshot(),
            positions: positions.clone(),
            saved_at: String::new(),
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&project).unwrap();
        let loaded: SavedProject = serde_json::from_str(&json).unwrap();

        // Restore into a new engine
        let mut engine2 = GraphEngine::new();
        engine2.restore(&loaded.snapshot);

        assert_eq!(engine2.block_count(), 2);
        assert_eq!(engine2.channel_count(), 1);
        assert_eq!(engine2.block(a).unwrap().config["value"], 42.0);
        assert!(engine2.channel(ch).is_some());
        assert_eq!(loaded.positions, positions);
    }
}
