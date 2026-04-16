//! Block registry: catalog of all configurable block types with categories.

use serde::{Deserialize, Serialize};

use crate::blocks;
use crate::lower::ConfigurableBlock;
use crate::schema::BlockCategory;

/// Serializable block descriptor for the frontend palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDescriptor {
    pub block_type: String,
    pub display_name: String,
    pub category: BlockCategory,
    pub description: String,
    pub config_schema: Vec<crate::schema::ConfigField>,
}

/// Build a list of block descriptors for all registered block types.
///
/// This is the data sent to the frontend to populate the palette sub-menus.
pub fn block_descriptors() -> Vec<BlockDescriptor> {
    blocks::registry()
        .into_iter()
        .map(|entry| {
            let instance = (entry.create)();
            BlockDescriptor {
                block_type: entry.block_type.into(),
                display_name: entry.display_name.into(),
                category: entry.category,
                description: entry.description.into(),
                config_schema: instance.config_schema(),
            }
        })
        .collect()
}

/// Group block descriptors by category for sub-menu rendering.
pub fn descriptors_by_category() -> Vec<(BlockCategory, Vec<BlockDescriptor>)> {
    let mut groups: Vec<(BlockCategory, Vec<BlockDescriptor>)> = BlockCategory::all()
        .iter()
        .map(|cat| (cat.clone(), Vec::new()))
        .collect();

    for desc in block_descriptors() {
        if let Some(group) = groups.iter_mut().find(|(cat, _)| *cat == desc.category) {
            group.1.push(desc);
        }
    }

    groups.retain(|(_, entries)| !entries.is_empty());
    groups
}

/// Create a configurable block instance by type name.
pub fn create_block(block_type: &str) -> Option<Box<dyn ConfigurableBlock>> {
    blocks::registry()
        .into_iter()
        .find(|entry| entry.block_type == block_type)
        .map(|entry| (entry.create)())
}

/// Serialize the full palette (for JSON transport to WASM frontend).
pub fn palette_json() -> String {
    let groups = descriptors_by_category();
    serde_json::to_string(&groups).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_descriptors() {
        let descs = block_descriptors();
        assert!(descs.len() >= 3);
        let pid = descs.iter().find(|d| d.block_type == "pid").unwrap();
        assert_eq!(pid.display_name, "PID Controller");
        assert_eq!(pid.category, BlockCategory::Control);
        assert!(!pid.config_schema.is_empty());
    }

    #[test]
    fn test_create_block() {
        let block = create_block("pid").expect("should create pid");
        assert_eq!(block.block_type(), "pid");

        let block = create_block("gain").expect("should create gain");
        assert_eq!(block.block_type(), "gain");

        assert!(create_block("nonexistent").is_none());
    }

    #[test]
    fn test_descriptors_by_category() {
        let groups = descriptors_by_category();
        assert!(!groups.is_empty());
        // Control category should have PID
        let control = groups
            .iter()
            .find(|(cat, _)| *cat == BlockCategory::Control);
        assert!(control.is_some());
        assert!(control.unwrap().1.iter().any(|d| d.block_type == "pid"));
    }

    #[test]
    fn test_palette_json() {
        let json = palette_json();
        assert!(json.contains("pid"));
        assert!(json.contains("PID Controller"));
        // Should be valid JSON
        let _: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
    }

    #[test]
    fn test_palette_json_is_nonempty_array() {
        let json = palette_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
        assert!(parsed.is_array(), "palette_json should return a JSON array");
        assert!(
            !parsed.as_array().unwrap().is_empty(),
            "palette should not be empty"
        );
    }
}
