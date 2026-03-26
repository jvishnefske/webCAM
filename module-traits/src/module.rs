//! The core `Module` trait — identity and capability queries.

use alloc::string::String;
use alloc::vec::Vec;

use crate::codegen_trait::Codegen;
use crate::sim::SimModel;
use crate::tick::Tick;
use crate::value::PortDef;
use crate::analysis::AnalysisModel;

/// Identity and metadata. Every block implements this.
/// The registry stores `Box<dyn Module>`.
pub trait Module {
    /// Human-readable block name (e.g. "Constant", "Gain").
    fn name(&self) -> &str;

    /// Block type identifier used for serialization.
    fn block_type(&self) -> &str;

    /// Input port definitions.
    fn input_ports(&self) -> Vec<PortDef>;

    /// Output port definitions.
    fn output_ports(&self) -> Vec<PortDef>;

    /// Serialise block-specific config to JSON.
    fn config_json(&self) -> String {
        String::from("{}")
    }

    /// Downcast to `Tick` for pure computation blocks.
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        None
    }

    /// Downcast to `SimModel` for simulated peripheral blocks.
    fn as_sim_model(&mut self) -> Option<&mut dyn SimModel> {
        None
    }

    /// Downcast to `Codegen` for custom code emission.
    fn as_codegen(&self) -> Option<&dyn Codegen> {
        None
    }

    /// Downcast to `AnalysisModel` for math model analysis.
    fn as_analysis(&self) -> Option<&dyn AnalysisModel> {
        None
    }
}
