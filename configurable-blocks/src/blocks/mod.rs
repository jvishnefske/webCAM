//! Built-in configurable blocks.

pub mod pid;

use crate::lower::ConfigurableBlock;
use crate::schema::BlockCategory;

/// Metadata for a block type shown in the palette.
pub struct BlockEntry {
    pub block_type: &'static str,
    pub display_name: &'static str,
    pub category: BlockCategory,
    pub description: &'static str,
    /// Factory function to create a default instance.
    pub create: fn() -> Box<dyn ConfigurableBlock>,
}

/// All registered configurable block types.
pub fn registry() -> Vec<BlockEntry> {
    vec![
        BlockEntry {
            block_type: "pid",
            display_name: "PID Controller",
            category: BlockCategory::Control,
            description: "Proportional-Integral-Derivative controller with configurable gains, pubsub I/O, and output clamping",
            create: || Box::new(pid::PidBlock::default()),
        },
        BlockEntry {
            block_type: "gain",
            display_name: "Gain",
            category: BlockCategory::Math,
            description: "Multiply input by a constant factor",
            create: || Box::new(pid::SimpleGainBlock::default()),
        },
        BlockEntry {
            block_type: "pubsub_bridge",
            display_name: "PubSub Bridge",
            category: BlockCategory::PubSub,
            description: "Subscribe to a topic, apply gain, publish to another topic",
            create: || Box::new(pid::PubSubBridgeBlock::default()),
        },
    ]
}

/// Return block entries grouped by category.
pub fn registry_by_category() -> Vec<(BlockCategory, Vec<BlockEntry>)> {
    let mut groups: Vec<(BlockCategory, Vec<BlockEntry>)> = BlockCategory::all()
        .iter()
        .map(|cat| (cat.clone(), Vec::new()))
        .collect();

    for entry in registry() {
        if let Some(group) = groups.iter_mut().find(|(cat, _)| *cat == entry.category) {
            group.1.push(entry);
        }
    }

    // Remove empty categories
    groups.retain(|(_, entries)| !entries.is_empty());
    groups
}
