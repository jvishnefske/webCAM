//! Decentralized block registration.
//!
//! Each block module provides a `register()` function that pushes
//! `BlockRegistration` entries into a registry vec.  The entries are
//! collected once in `all_registrations()` and drive both
//! `create_block()` and `available_block_types()`.

use module_traits::Module;

/// A single block type registration — enough to create an instance
/// from JSON config and to advertise the type in the palette.
pub struct BlockRegistration {
    pub block_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub create_from_json: fn(&str) -> Result<Box<dyn Module>, String>,
}
