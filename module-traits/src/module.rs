//! The core `Module` trait — identity and capability queries.

use alloc::string::String;
use alloc::vec::Vec;

use crate::analysis::AnalysisModel;
use crate::codegen_trait::Codegen;
use crate::sim::SimModel;
use crate::tick::Tick;
use crate::value::PortDef;

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

    /// Whether this block is a delay element (z⁻¹).
    ///
    /// Delay blocks break feedback cycles in the dataflow graph. Their output
    /// at tick N equals their input at tick N-1. The topological sort excludes
    /// incoming edges to delay blocks so that cycles through them are allowed.
    fn is_delay(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::Module;
    use crate::tick::Tick;
    use crate::value::{PortDef, PortKind, Value};

    // ── Minimal mock: no optional traits ──────────────────────────

    struct MinimalBlock;

    impl super::Module for MinimalBlock {
        fn name(&self) -> &str {
            "Minimal"
        }
        fn block_type(&self) -> &str {
            "minimal"
        }
        fn input_ports(&self) -> Vec<PortDef> {
            vec![]
        }
        fn output_ports(&self) -> Vec<PortDef> {
            vec![]
        }
    }

    // ── Gain mock: implements Tick too ────────────────────────────

    struct GainBlock {
        factor: f64,
    }

    impl super::Module for GainBlock {
        fn name(&self) -> &str {
            "Gain"
        }
        fn block_type(&self) -> &str {
            "gain"
        }
        fn input_ports(&self) -> Vec<PortDef> {
            vec![PortDef::new("in", PortKind::Float)]
        }
        fn output_ports(&self) -> Vec<PortDef> {
            vec![PortDef::new("out", PortKind::Float)]
        }
        fn config_json(&self) -> String {
            alloc::format!("{{\"factor\":{}}}", self.factor)
        }
        fn as_tick(&mut self) -> Option<&mut dyn Tick> {
            Some(self)
        }
    }

    impl Tick for GainBlock {
        fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
            let v = inputs
                .first()
                .and_then(|i| i.and_then(|v| v.as_float()))
                .unwrap_or(0.0);
            vec![Some(Value::Float(v * self.factor))]
        }
    }

    // ── Multi-port mock ──────────────────────────────────────────

    struct MixerBlock;

    impl super::Module for MixerBlock {
        fn name(&self) -> &str {
            "Mixer"
        }
        fn block_type(&self) -> &str {
            "mixer"
        }
        fn input_ports(&self) -> Vec<PortDef> {
            vec![
                PortDef::new("a", PortKind::Float),
                PortDef::new("b", PortKind::Float),
            ]
        }
        fn output_ports(&self) -> Vec<PortDef> {
            vec![
                PortDef::new("sum", PortKind::Float),
                PortDef::new("diff", PortKind::Float),
            ]
        }
    }

    // 1. name() and block_type()
    #[test]
    fn test_module_name_and_type() {
        let block = GainBlock { factor: 2.0 };
        assert_eq!(block.name(), "Gain");
        assert_eq!(block.block_type(), "gain");

        let minimal = MinimalBlock;
        assert_eq!(minimal.name(), "Minimal");
        assert_eq!(minimal.block_type(), "minimal");
    }

    // 2. input_ports() returns expected ports
    #[test]
    fn test_module_input_ports() {
        let block = GainBlock { factor: 1.0 };
        let ports = block.input_ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "in");
        assert_eq!(ports[0].kind, PortKind::Float);
    }

    // 3. output_ports() returns expected ports
    #[test]
    fn test_module_output_ports() {
        let block = GainBlock { factor: 1.0 };
        let ports = block.output_ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "out");
        assert_eq!(ports[0].kind, PortKind::Float);
    }

    // 4. Default config_json() returns "{}"
    #[test]
    fn test_module_default_config_json() {
        let block = MinimalBlock;
        assert_eq!(block.config_json(), "{}");
    }

    // 5. Overridden config_json() returns custom value
    #[test]
    fn test_module_custom_config_json() {
        let block = GainBlock { factor: 3.5 };
        let json = block.config_json();
        assert_eq!(json, "{\"factor\":3.5}");
    }

    // 6. Default as_tick() returns None
    #[test]
    fn test_module_default_as_tick_none() {
        let mut block = MinimalBlock;
        assert!(block.as_tick().is_none());
    }

    // 7. Default as_sim_model() returns None
    #[test]
    fn test_module_default_as_sim_model_none() {
        let mut block = MinimalBlock;
        assert!(block.as_sim_model().is_none());
    }

    // 8. Module that also implements Tick — as_tick() returns Some and is callable
    #[test]
    fn test_module_with_tick_impl() {
        let mut block = GainBlock { factor: 2.5 };
        assert!(block.as_tick().is_some());

        // Use the returned Tick reference to run a tick
        let tick_ref = block.as_tick().unwrap();
        let input = Value::Float(4.0);
        let result = tick_ref.tick(&[Some(&input)], 0.01);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Some(Value::Float(10.0)));
    }

    // 9. Default as_codegen() returns None
    #[test]
    fn test_module_default_as_codegen_none() {
        let block = MinimalBlock;
        assert!(block.as_codegen().is_none());
    }

    // 10. Default as_analysis() returns None
    #[test]
    fn test_module_default_as_analysis_none() {
        let block = MinimalBlock;
        assert!(block.as_analysis().is_none());
    }

    // 11. Multi-port block returns correct port counts and names
    #[test]
    fn test_module_multi_port() {
        let block = MixerBlock;
        let ins = block.input_ports();
        let outs = block.output_ports();

        assert_eq!(ins.len(), 2);
        assert_eq!(ins[0].name, "a");
        assert_eq!(ins[1].name, "b");

        assert_eq!(outs.len(), 2);
        assert_eq!(outs[0].name, "sum");
        assert_eq!(outs[1].name, "diff");
    }

    // 12. Tick with missing input defaults to zero
    #[test]
    fn test_module_tick_missing_input() {
        let mut block = GainBlock { factor: 5.0 };
        let tick_ref = block.as_tick().unwrap();
        let result = tick_ref.tick(&[None], 0.01);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Some(Value::Float(0.0)));
    }

    // 13. Tick with empty inputs slice defaults to zero
    #[test]
    fn test_module_tick_empty_inputs() {
        let mut block = GainBlock { factor: 7.0 };
        let tick_ref = block.as_tick().unwrap();
        let result = tick_ref.tick(&[], 0.01);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Some(Value::Float(0.0)));
    }

    // 14. Minimal block has no ports
    #[test]
    fn test_module_minimal_no_ports() {
        let block = MinimalBlock;
        assert!(block.input_ports().is_empty());
        assert!(block.output_ports().is_empty());
    }
}
