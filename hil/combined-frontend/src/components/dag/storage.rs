//! Project persistence: serialisable data types and LocalStorage helpers.
//!
//! Pure-logic types and functions (serialisation, time formatting) are always
//! compiled. Functions that touch the browser (`web_sys::Storage`) are gated
//! behind `#[cfg(target_arch = "wasm32")]`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Serialisable project types
// ---------------------------------------------------------------------------

/// A complete project snapshot that can be stored and restored.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedProject {
    pub name: String,
    pub blocks: Vec<SavedBlock>,
    pub channels: Vec<SavedChannel>,
    pub viewport: Viewport,
    /// Millisecond timestamp from `js_sys::Date::now()`.
    pub last_modified: f64,
}

/// Persisted representation of a single block on the canvas.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedBlock {
    pub id: u32,
    pub block_type: String,
    pub config: serde_json::Value,
    pub x: f64,
    pub y: f64,
}

/// Persisted representation of a channel (edge) between two blocks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedChannel {
    pub from_block: u32,
    pub from_port: usize,
    pub to_block: u32,
    pub to_port: usize,
}

/// Canvas viewport state (pan + zoom).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Viewport {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// LocalStorage key helpers
// ---------------------------------------------------------------------------

#[cfg(any(target_arch = "wasm32", test))]
const STORAGE_PREFIX: &str = "rustcam_project_";

/// Build the LocalStorage key for the given project name.
#[cfg(any(target_arch = "wasm32", test))]
fn storage_key(name: &str) -> String {
    format!("{STORAGE_PREFIX}{name}")
}

// ---------------------------------------------------------------------------
// Web-sys LocalStorage helpers (wasm32 only)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// List all saved project names together with their `last_modified` timestamps.
///
/// Returns an empty `Vec` when LocalStorage is unavailable.
#[cfg(target_arch = "wasm32")]
pub fn list_projects() -> Vec<(String, f64)> {
    let Some(storage) = local_storage() else {
        return Vec::new();
    };
    let len = storage.length().unwrap_or(0);
    let mut projects = Vec::new();
    for i in 0..len {
        let Some(key) = storage.key(i).ok().flatten() else {
            continue;
        };
        if let Some(name) = key.strip_prefix(STORAGE_PREFIX) {
            if let Some(json) = storage.get_item(&key).ok().flatten() {
                if let Ok(proj) = serde_json::from_str::<SavedProject>(&json) {
                    projects.push((name.to_string(), proj.last_modified));
                }
            }
        }
    }
    // Most-recently modified first.
    projects.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    projects
}

/// Save a project to LocalStorage.
///
/// Silently does nothing when LocalStorage is unavailable.
#[cfg(target_arch = "wasm32")]
pub fn save_project(project: &SavedProject) {
    let Some(storage) = local_storage() else {
        return;
    };
    if let Ok(json) = serde_json::to_string(project) {
        let _ = storage.set_item(&storage_key(&project.name), &json);
    }
}

/// Load a project from LocalStorage by name.
#[cfg(target_arch = "wasm32")]
pub fn load_project(name: &str) -> Option<SavedProject> {
    let storage = local_storage()?;
    let json = storage.get_item(&storage_key(name)).ok()??;
    serde_json::from_str(&json).ok()
}

/// Delete a project from LocalStorage by name.
#[cfg(target_arch = "wasm32")]
pub fn delete_project(name: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(&storage_key(name));
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (always compiled)
// ---------------------------------------------------------------------------

/// Format a millisecond timestamp as a human-readable relative-time string.
///
/// `timestamp_ms` is the event time, `now_ms` is the current time (both in
/// milliseconds since the Unix epoch, as returned by `js_sys::Date::now()`).
pub fn format_relative_time(timestamp_ms: f64, now_ms: f64) -> String {
    let diff_s = ((now_ms - timestamp_ms) / 1000.0).max(0.0);

    if diff_s < 60.0 {
        return "just now".to_string();
    }

    let minutes = (diff_s / 60.0).floor() as u64;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }

    let hours = (diff_s / 3600.0).floor() as u64;
    if hours < 24 {
        return format!("{hours}h ago");
    }

    let days = (diff_s / 86400.0).floor() as u64;
    format!("{days}d ago")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_project_roundtrip() {
        let project = SavedProject {
            name: "demo".to_string(),
            blocks: vec![SavedBlock {
                id: 1,
                block_type: "constant".to_string(),
                config: serde_json::json!({"value": 42}),
                x: 100.0,
                y: 200.0,
            }],
            channels: vec![SavedChannel {
                from_block: 1,
                from_port: 0,
                to_block: 2,
                to_port: 0,
            }],
            viewport: Viewport {
                pan_x: 10.0,
                pan_y: 20.0,
                zoom: 1.5,
            },
            last_modified: 1_700_000_000_000.0,
        };

        let json = serde_json::to_string(&project).expect("serialize");
        let restored: SavedProject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(project, restored);
    }

    #[test]
    fn test_format_relative_time_just_now() {
        assert_eq!(format_relative_time(1000.0, 1500.0), "just now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = 1000.0 * 60.0 * 5.0; // 5 minutes in ms
        assert_eq!(format_relative_time(0.0, now), "5m ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let now = 1000.0 * 3600.0 * 3.0; // 3 hours in ms
        assert_eq!(format_relative_time(0.0, now), "3h ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let now = 1000.0 * 86400.0 * 7.0; // 7 days in ms
        assert_eq!(format_relative_time(0.0, now), "7d ago");
    }

    #[test]
    fn test_viewport_default() {
        let vp = Viewport::default();
        assert!((vp.pan_x - 0.0).abs() < f64::EPSILON);
        assert!((vp.pan_y - 0.0).abs() < f64::EPSILON);
        assert!((vp.zoom - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_format_relative_time_boundary_59s() {
        // 59 seconds should still be "just now"
        let now = 59_000.0;
        assert_eq!(format_relative_time(0.0, now), "just now");
    }

    #[test]
    fn test_format_relative_time_boundary_60s() {
        // Exactly 60 seconds should be "1m ago"
        let now = 60_000.0;
        assert_eq!(format_relative_time(0.0, now), "1m ago");
    }

    #[test]
    fn test_storage_key_format() {
        assert_eq!(storage_key("my_project"), "rustcam_project_my_project");
    }
}
