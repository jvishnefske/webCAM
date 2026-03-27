//! MLIR-based code generation backend for the RustCAM dataflow engine.
//!
//! This crate replaces the string-interpolation Rust codegen in `emit.rs`
//! with an MLIR pipeline:
//!
//! ```text
//! GraphSnapshot (JSON) → lower.rs → .mlir → mlir-opt → mlir-translate → C
//! ```
//!
//! The generated C code is compiled into a static library via the `cc` crate
//! and linked into the firmware binary via FFI.
//!
//! # Crate structure
//!
//! - [`dialect`] — MLIR op names, type strings, attribute formatting
//! - [`c_types`] — PortKind → C type mapping
//! - [`lower`] — GraphSnapshot → `.mlir` text generation
//! - [`state_machine`] — FSM blocks → MLIR region-based control flow
//! - [`peripherals`] — Generate `peripherals.h` C header
//! - [`pipeline`] — Orchestrate mlir-opt → mlir-translate → .c/.h

pub mod c_types;
pub mod dialect;
pub mod lower;
pub mod peripherals;
pub mod pipeline;
pub mod runtime;
pub mod state_machine;

use lower::GraphSnapshot;

pub use runtime::{BlockFn, DagRuntime, HardwareBridge, NullHardware};

/// Lower a `GraphSnapshot` (deserialized from JSON) to textual `.mlir`.
///
/// This is the primary entry point for Phase 1 (textual MLIR generation).
/// The output can be piped to `mlir-opt --verify-diagnostics` for validation.
pub fn graph_to_mlir(snap: &GraphSnapshot) -> Result<String, String> {
    lower::lower_graph(snap)
}

/// Lower a JSON string containing a `GraphSnapshot` to `.mlir` text.
///
/// Convenience wrapper that deserializes first.
pub fn graph_json_to_mlir(json: &str) -> Result<String, String> {
    let snap: GraphSnapshot =
        serde_json::from_str(json).map_err(|e| format!("failed to parse graph JSON: {e}"))?;
    graph_to_mlir(&snap)
}

/// Run the full MLIR pipeline: lower → optimize → emit C.
///
/// Uses default pipeline configuration. If MLIR tools are not available,
/// produces the `.mlir` text and a fallback C stub.
pub fn compile_to_c(snap: &GraphSnapshot) -> Result<pipeline::PipelineOutput, String> {
    let config = pipeline::PipelineConfig::default();
    pipeline::run_pipeline(snap, &config)
}

/// Build a [`DagRuntime`] from a JSON `GraphSnapshot`.
///
/// The runtime deserializes the DAG, curries each block's config into a
/// [`BlockFn`] enum variant (partial application), and stores all state in
/// a flat `f64` buffer (typeless container). The returned object can
/// [`receive`](DagRuntime::receive) channel calls and [`tick`](DagRuntime::tick).
pub fn build_runtime(json: &str) -> Result<DagRuntime, String> {
    DagRuntime::from_json(json)
}

/// Generate all files for the MLIR-backed logic crate in a workspace.
///
/// Returns `(path, content)` pairs for:
/// - `logic/csrc/graph.mlir`
/// - `logic/csrc/peripherals.h`
/// - `logic/csrc/logic.c`
/// - `logic/build.rs`
/// - `logic/Cargo.toml`
/// - `logic/src/ffi.rs`
/// - `logic/src/lib.rs`
pub fn generate_logic_files(snap: &GraphSnapshot) -> Result<Vec<(String, String)>, String> {
    let config = pipeline::PipelineConfig::default();
    pipeline::generate_mlir_logic_files(snap, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_json_to_mlir_basic() {
        let json = r#"{
            "blocks": [{
                "id": 1,
                "block_type": "constant",
                "name": "const_1",
                "inputs": [],
                "outputs": [{"name": "out", "kind": "Float"}],
                "config": {"value": 99.0},
                "output_values": []
            }],
            "channels": [],
            "tick_count": 0,
            "time": 0.0
        }"#;
        let mlir = graph_json_to_mlir(json).unwrap();
        assert!(mlir.contains("dataflow.constant"));
        assert!(mlir.contains("99"));
        assert!(mlir.contains("func.func @tick"));
    }

    #[test]
    fn graph_json_to_mlir_invalid_json() {
        let result = graph_json_to_mlir("not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[test]
    fn compile_to_c_produces_output() {
        let snap = GraphSnapshot {
            blocks: vec![lower::BlockSnapshot {
                id: 1,
                block_type: "constant".to_string(),
                name: "c".to_string(),
                inputs: vec![],
                outputs: vec![lower::PortDef {
                    name: "out".to_string(),
                    kind: module_traits::value::PortKind::Float,
                }],
                config: serde_json::json!({"value": 1.0}),
                output_values: vec![],
                custom_codegen: None,
            }],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let output = compile_to_c(&snap).unwrap();
        assert!(output.mlir_text.contains("dataflow.constant"));
        assert!(output.peripherals_h.contains("hw_adc_read"));
    }
}
