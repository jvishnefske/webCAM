//! Core block trait and value types for the dataflow graph.
//!
//! Types are re-exported from the `module-traits` crate so that external
//! crate authors can depend on `module-traits` alone.

use serde::{Deserialize, Serialize};

// Re-export core types from module-traits.
pub use module_traits::{
    AnalysisMetadata, AnalysisModel, Codegen, FieldType, MessageData, MessageField, MessageSchema,
    Module, PortDef, PortKind, SimModel, SimPeripherals, Tick, Value,
};

/// Opaque block identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u32);
