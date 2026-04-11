//! Target definitions, MCU metadata, and per-target code generators.
//!
//! This crate is the single source of truth for:
//! - [`TargetFamily`] enum and [`TargetDef`] catalog
//! - [`Binding`] / [`PinBinding`] codegen-level pin models
//! - Per-target firmware generators (host, rp2040, stm32f4, esp32c3, stm32g0b1)
//!
//! The [`TargetCodegen`] trait lives here (not in `module-traits`) because it
//! depends on `graph_model::GraphSnapshot`, which requires `std`.

pub mod binding;
pub mod generators;
pub mod target;

use crate::binding::Binding;
use graph_model::GraphSnapshot;

/// Per-target code generation trait.
///
/// Implementors produce a set of files (path, content) that form a deployable
/// firmware crate for a specific target.
pub trait TargetCodegen {
    fn generate(
        &self,
        snap: &GraphSnapshot,
        binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String>;
}
