//! Block registry trait: catalog of available block types.
//! Target registry trait: catalog of supported deployment targets.

use crate::Module;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Metadata about a block type for UI palette display.
pub struct BlockTypeInfo {
    pub block_type: String,
    pub display_name: String,
    pub category: String,
}

/// Catalog of available block types. Implement to provide blocks.
pub trait BlockRegistry {
    fn block_types(&self) -> Vec<BlockTypeInfo>;
    fn create(&self, type_id: &str, config: &str) -> Result<Box<dyn Module>, String>;
}

/// Information about a supported deployment target.
pub struct TargetInfo {
    pub id: String,
    pub display_name: String,
    pub rust_target: String,
}

/// Catalog of available deployment targets (MCU families / host).
pub trait TargetRegistry {
    /// List all supported targets.
    fn targets(&self) -> Vec<TargetInfo>;
    /// Look up the MCU definition for a target id.
    fn mcu_def(&self, id: &str) -> Option<&crate::inventory::McuDef>;
}
