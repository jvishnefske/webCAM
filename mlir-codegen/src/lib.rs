#![forbid(unsafe_code)]
//! MLIR-based code generation backend for the RustCAM dataflow engine.
//!
//! This crate lowers a `GraphSnapshot` to MLIR for analysis and optimization,
//! and generates safe Rust logic crates (no C FFI, no `unsafe`).
//!
//! ```text
//! GraphSnapshot (JSON) → lower.rs → .mlir → mlir-opt → analysis
//! ```
//!
//! # Crate structure
//!
//! - [`dialect`] — MLIR op names, type strings, attribute formatting
//! - [`c_types`] — PortKind → C type mapping
//! - [`lower`] — GraphSnapshot → `.mlir` text generation
//! - [`state_machine`] — FSM blocks → MLIR region-based control flow
//! - [`peripherals`] — Generate safe Rust state modules
//! - [`pipeline`] — Orchestrate mlir-opt → mlir-translate pipeline

pub mod c_types;
pub mod dialect;
pub mod emit_rust;
pub mod ir;
pub mod lower;
pub mod peripherals;
pub mod pipeline;
pub mod printer;
pub mod runtime;
pub mod state_machine;

use lower::GraphSnapshot;

pub use runtime::{BlockFn, DagRuntime, HardwareBridge, NullHardware};
pub use ir::{IrModule, IrBuilder, ValueId, IrType, Attr, IrOp, IrFunc};
pub use printer::print_mlir;
pub use emit_rust::emit_rust;

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

/// Run the full MLIR pipeline: lower → optimize → (optionally) emit C.
///
/// Uses default pipeline configuration. If MLIR tools are not available,
/// produces only the `.mlir` text. The C output is for analysis only;
/// execution uses pure Rust runtime.
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

/// Generate all files for the logic crate in a workspace.
///
/// Produces a safe Rust-only logic crate (no C FFI, no `unsafe`).
/// Returns `(path, content)` pairs for:
/// - `logic/mlir/graph.mlir`
/// - `logic/Cargo.toml`
/// - `logic/src/ffi.rs`
/// - `logic/src/lib.rs`
pub fn generate_logic_files(snap: &GraphSnapshot) -> Result<Vec<(String, String)>, String> {
    let config = pipeline::PipelineConfig::default();
    pipeline::generate_mlir_logic_files(snap, &config)
}

/// Lower a GraphSnapshot to a typed IrModule.
///
/// This is the new typed-IR entry point. The returned module can be:
/// - Printed to MLIR text via [`print_mlir`]
/// - Emitted as safe Rust via [`emit_rust`]
pub fn graph_to_ir(snap: &GraphSnapshot) -> Result<IrModule, String> {
    lower::lower_graph_ir(snap)
}

/// Lower a JSON GraphSnapshot to a typed IrModule.
pub fn graph_json_to_ir(json: &str) -> Result<IrModule, String> {
    let snap: GraphSnapshot =
        serde_json::from_str(json).map_err(|e| format!("failed to parse graph JSON: {e}"))?;
    graph_to_ir(&snap)
}

/// Run the full typed IR pipeline: lower → MLIR text + Rust source.
pub fn run_ir_pipeline(snap: &GraphSnapshot) -> Result<pipeline::IrPipelineOutput, String> {
    pipeline::run_ir_pipeline(snap)
}

/// Generate logic crate files using the typed IR pipeline.
pub fn generate_ir_logic_files(snap: &GraphSnapshot) -> Result<Vec<(String, String)>, String> {
    pipeline::generate_ir_logic_files(snap)
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
    }

    #[test]
    fn test_build_runtime_valid() {
        let json = r#"{
            "blocks": [{
                "id": 1,
                "block_type": "constant",
                "name": "const_1",
                "inputs": [],
                "outputs": [{"name": "out", "kind": "Float"}],
                "config": {"value": 7.0},
                "output_values": []
            }],
            "channels": [],
            "tick_count": 0,
            "time": 0.0
        }"#;
        let rt = build_runtime(json);
        assert!(rt.is_ok(), "build_runtime should succeed for valid JSON");
        let rt = rt.unwrap();
        assert_eq!(rt.node_count(), 1);
    }

    #[test]
    fn test_build_runtime_invalid_json() {
        let result = build_runtime("not json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("JSON parse error"),
            "expected JSON parse error, got: {err}"
        );
    }

    #[test]
    fn test_build_runtime_tick() {
        let json = r#"{
            "blocks": [{
                "id": 1,
                "block_type": "constant",
                "name": "const_1",
                "inputs": [],
                "outputs": [{"name": "out", "kind": "Float"}],
                "config": {"value": 42.0},
                "output_values": []
            }],
            "channels": [],
            "tick_count": 0,
            "time": 0.0
        }"#;
        let mut rt = build_runtime(json).unwrap();
        rt.tick(&mut NullHardware);
        let val = rt.read_output(1, 0);
        assert_eq!(
            val,
            Some(42.0),
            "constant block should produce its configured value after tick"
        );
    }

    #[test]
    fn test_generate_logic_files_has_expected_paths() {
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
        let files = generate_logic_files(&snap).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

        // Must include safe Rust files + MLIR for debugging
        assert!(paths.contains(&"logic/mlir/graph.mlir"));
        assert!(paths.contains(&"logic/Cargo.toml"));
        assert!(paths.contains(&"logic/src/ffi.rs"));
        assert!(paths.contains(&"logic/src/lib.rs"));

        // Must NOT include C files, C headers, or cc build scripts
        for (path, _) in &files {
            assert!(!path.ends_with(".h"), "no C headers: {path}");
            assert!(!path.ends_with(".c"), "no C sources: {path}");
        }

        // All .rs files must be safe (allow `forbid(unsafe_code)` lint)
        for (path, content) in &files {
            if path.ends_with(".rs") {
                assert!(
                    !content.contains("unsafe {") && !content.contains("unsafe fn"),
                    "{path} contains unsafe code"
                );
            }
        }
    }

    #[test]
    fn test_graph_to_mlir_empty_graph() {
        let snap = GraphSnapshot {
            blocks: vec![],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let mlir = graph_to_mlir(&snap).unwrap();
        assert!(
            mlir.contains("func.func @tick"),
            "empty graph should still produce a tick function"
        );
        assert!(
            mlir.contains("module"),
            "empty graph should still produce a module wrapper"
        );
        assert!(
            mlir.contains("return"),
            "empty graph tick function should have a return"
        );
    }

    #[test]
    fn test_compile_multi_block() {
        // constant(5.0) -> gain(2.0) chain
        let snap = GraphSnapshot {
            blocks: vec![
                lower::BlockSnapshot {
                    id: 1,
                    block_type: "constant".to_string(),
                    name: "src".to_string(),
                    inputs: vec![],
                    outputs: vec![lower::PortDef {
                        name: "out".to_string(),
                        kind: module_traits::value::PortKind::Float,
                    }],
                    config: serde_json::json!({"value": 5.0}),
                    output_values: vec![],
                    custom_codegen: None,
                },
                lower::BlockSnapshot {
                    id: 2,
                    block_type: "gain".to_string(),
                    name: "amp".to_string(),
                    inputs: vec![lower::PortDef {
                        name: "in".to_string(),
                        kind: module_traits::value::PortKind::Float,
                    }],
                    outputs: vec![lower::PortDef {
                        name: "out".to_string(),
                        kind: module_traits::value::PortKind::Float,
                    }],
                    config: serde_json::json!({"param1": 2.0}),
                    output_values: vec![],
                    custom_codegen: None,
                },
            ],
            channels: vec![lower::Channel {
                id: lower::ChannelId(1),
                from_block: lower::BlockId(1),
                from_port: 0,
                to_block: lower::BlockId(2),
                to_port: 0,
            }],
            tick_count: 0,
            time: 0.0,
        };
        let output = compile_to_c(&snap).unwrap();
        assert!(
            output.mlir_text.contains("dataflow.constant"),
            "MLIR should contain constant op"
        );
        assert!(
            output.mlir_text.contains("dataflow.gain"),
            "MLIR should contain gain op"
        );
        assert!(
            output.mlir_text.contains("5"),
            "MLIR should contain the constant value 5"
        );
        assert!(
            output.mlir_text.contains("2"),
            "MLIR should contain the gain factor 2"
        );
    }
}
