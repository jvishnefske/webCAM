//! Trait definitions for the RustCAM dataflow module system.
//!
//! This crate provides the core trait hierarchy:
//! - [`Module`] — identity, ports, config, capability queries
//! - [`Tick`] — pure computation (browser + sim + codegen)
//! - [`SimModel`] — simulated hardware interaction
//! - [`Codegen`] — custom code emission for embedded targets
//! - [`AnalysisModel`] — placeholder for math model analysis
//!
//! External crate authors depend on this crate (not the full webCAM crate)
//! to implement custom blocks.

#![no_std]

extern crate alloc;

pub mod analysis;
pub mod codegen_trait;
pub mod module;
pub mod sim;
pub mod tick;
pub mod value;

pub use analysis::{AnalysisMetadata, AnalysisModel};
pub use codegen_trait::Codegen;
pub use module::Module;
pub use sim::{SimModel, SimPeripherals};
pub use tick::Tick;
pub use value::{PortDef, PortKind, Value};
