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

#[cfg(test)]
mod tests {
    use super::{AnalysisMetadata, AnalysisModel};
    use alloc::string::String;
    use alloc::vec;

    struct MockAnalysis;

    impl AnalysisModel for MockAnalysis {
        fn analysis_metadata(&self) -> AnalysisMetadata {
            AnalysisMetadata {
                description: String::from("First-order low-pass filter"),
                parameters: vec![String::from("cutoff_hz"), String::from("sample_rate")],
            }
        }
    }

    #[test]
    fn test_analysis_metadata_construction() {
        let meta = AnalysisMetadata {
            description: String::from("PID controller"),
            parameters: vec![String::from("kp"), String::from("ki"), String::from("kd")],
        };
        assert_eq!(meta.description, "PID controller");
        assert_eq!(meta.parameters.len(), 3);
        assert_eq!(meta.parameters[0], "kp");
        assert_eq!(meta.parameters[1], "ki");
        assert_eq!(meta.parameters[2], "kd");
    }

    #[test]
    fn test_analysis_metadata_serde_roundtrip() {
        let original = AnalysisMetadata {
            description: String::from("Second-order system"),
            parameters: vec![String::from("omega_n"), String::from("zeta")],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: AnalysisMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.description, original.description);
        assert_eq!(restored.parameters, original.parameters);
    }

    #[test]
    fn test_analysis_model_trait() {
        let model = MockAnalysis;
        let meta = model.analysis_metadata();
        assert_eq!(meta.description, "First-order low-pass filter");
        assert_eq!(meta.parameters.len(), 2);
        assert_eq!(meta.parameters[0], "cutoff_hz");
        assert_eq!(meta.parameters[1], "sample_rate");
    }
}
