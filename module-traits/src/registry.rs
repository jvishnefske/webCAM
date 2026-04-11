//! Block registry trait: catalog of available block types.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use crate::Module;

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
