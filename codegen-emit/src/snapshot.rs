//! Codegen-specific snapshot types.
//!
//! These extend the pure-logical [`graph_model::BlockSnapshot`] with fields
//! needed for code generation: `output_values`, `target`, and `custom_codegen`.
//! The upstream crate (e.g. rustsim) converts its own rich snapshots into these
//! types before invoking code generation.

use graph_model::PortDef;
use module_traits::value::Value;

/// Block snapshot with codegen-specific extensions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodegenBlockSnapshot {
    pub id: u32,
    pub block_type: String,
    pub name: String,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    #[serde(default)]
    pub config: serde_json::Value,
    /// Last output values (one per output port). Used by legacy codegen path.
    #[serde(default)]
    pub output_values: Vec<Option<Value>>,
    /// Optional target MCU assignment for distributed codegen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<target_registry::target::TargetFamily>,
    /// Custom codegen output from blocks implementing the `Codegen` trait.
    /// When present, emit.rs uses this instead of built-in code generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_codegen: Option<String>,
    /// Whether this block is a delay element (z^-1) that breaks feedback cycles.
    #[serde(default)]
    pub is_delay: bool,
}

/// Graph snapshot with codegen-specific extensions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodegenGraphSnapshot {
    pub blocks: Vec<CodegenBlockSnapshot>,
    pub channels: Vec<graph_model::Channel>,
    #[serde(default)]
    pub tick_count: u64,
    #[serde(default)]
    pub time: f64,
}
