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
pub mod async_block;
pub mod codegen_trait;
pub mod deployment;
pub mod function_def;
pub mod hardware;
pub mod inventory;
pub mod module;
pub mod registry;
pub mod sim;
pub mod tick;
pub mod value;

pub use analysis::{AnalysisMetadata, AnalysisModel};
pub use async_block::{AsyncBlock, BlockContext};
pub use codegen_trait::Codegen;
pub use function_def::{
    builtin_function_defs, FuncPortDef, FunctionDef, FunctionOp, ParamDef, ParamKind,
};
pub use module::Module;
pub use registry::{BlockRegistry, BlockTypeInfo, TargetInfo, TargetRegistry};
pub use sim::{PeripheralError, SimModel, SimPeripherals};
pub use tick::Tick;
pub use value::{FieldType, MessageData, MessageField, MessageSchema, PortDef, PortKind, Value};
