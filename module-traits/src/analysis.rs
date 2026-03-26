//! Placeholder for future analysis model (transfer functions, state-space).

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// Metadata describing a block's mathematical model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    /// Human-readable description of the model.
    pub description: String,
    /// Parameter names for the model.
    pub parameters: Vec<String>,
}

/// Placeholder trait for future math model analysis (transfer functions, state-space).
pub trait AnalysisModel {
    fn analysis_metadata(&self) -> AnalysisMetadata;
}
