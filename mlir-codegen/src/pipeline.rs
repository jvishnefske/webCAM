//! Orchestrate the MLIR compilation pipeline.
//!
//! ```text
//! GraphSnapshot (JSON)
//!       │
//!       ▼
//!   lower.rs              ← GraphSnapshot → .mlir text
//!       │
//!       ▼
//!   mlir-opt (external)   ← canonicalize, constant fold, DCE
//!       │
//!       ▼
//!   mlir-translate        ← EmitC → C source (reference only)
//! ```

use std::path::PathBuf;
use std::process::Command;

use crate::ir::IrModule;
use crate::lower::{self, GraphSnapshot};
use crate::peripherals;
use crate::{emit_rust, printer};

// Note: the MLIR pipeline preserves .mlir for debugging and optimization
// analysis but does NOT generate C-FFI logic crates. The execution path
// uses pure-Rust dag-core evaluation (see configurable-blocks codegen).

/// Result of the MLIR compilation pipeline.
#[derive(Debug)]
pub struct PipelineOutput {
    /// Generated `.mlir` text (before optimization).
    pub mlir_text: String,
    /// Optimized `.mlir` text (after mlir-opt), if mlir-opt was available.
    pub mlir_optimized: Option<String>,
    /// Generated C source code, if mlir-translate was available.
    pub c_source: Option<String>,
}

/// Configuration for the MLIR pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Path to the `mlir-opt` binary. Default: "mlir-opt" (on PATH).
    pub mlir_opt: String,
    /// Path to the `mlir-translate` binary. Default: "mlir-translate" (on PATH).
    pub mlir_translate: String,
    /// MLIR optimization passes to run. Default: canonicalize, cse.
    pub opt_passes: Vec<String>,
    /// Working directory for temporary files.
    pub work_dir: PathBuf,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            mlir_opt: "mlir-opt".to_string(),
            mlir_translate: "mlir-translate".to_string(),
            opt_passes: vec!["--canonicalize".to_string(), "--cse".to_string()],
            work_dir: std::env::temp_dir().join("mlir-codegen"),
        }
    }
}

/// Run the full MLIR pipeline: lower → optimize → emit C.
///
/// If `mlir-opt` or `mlir-translate` are not available, the pipeline
/// degrades gracefully — the `.mlir` text is always produced, and
/// optional stages return `None`.
pub fn run_pipeline(
    snap: &GraphSnapshot,
    config: &PipelineConfig,
) -> Result<PipelineOutput, String> {
    // Step 1: Lower to MLIR
    let mlir_text = lower::lower_graph(snap)?;

    // Step 2: Optimize with mlir-opt (optional)
    let mlir_optimized = run_mlir_opt(&mlir_text, config);

    // Step 3: Translate to C via EmitC (optional, for analysis only)
    let source_mlir = mlir_optimized.as_deref().unwrap_or(&mlir_text);
    let c_source = run_mlir_translate(source_mlir, config);

    Ok(PipelineOutput {
        mlir_text,
        mlir_optimized,
        c_source,
    })
}

/// Lower a graph to MLIR text only (no external tools needed).
pub fn lower_only(snap: &GraphSnapshot) -> Result<PipelineOutput, String> {
    let mlir_text = lower::lower_graph(snap)?;

    Ok(PipelineOutput {
        mlir_text,
        mlir_optimized: None,
        c_source: None,
    })
}

/// Run `mlir-opt` on the given MLIR text. Returns `None` if the tool is
/// unavailable or fails.
fn run_mlir_opt(mlir_text: &str, config: &PipelineConfig) -> Option<String> {
    std::fs::create_dir_all(&config.work_dir).ok()?;
    let input_path = config.work_dir.join("input.mlir");
    let output_path = config.work_dir.join("optimized.mlir");

    std::fs::write(&input_path, mlir_text).ok()?;

    let mut cmd = Command::new(&config.mlir_opt);
    for pass in &config.opt_passes {
        cmd.arg(pass);
    }
    cmd.arg(&input_path);
    cmd.arg("-o");
    cmd.arg(&output_path);

    let output = cmd.output().ok()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("mlir-opt failed: {stderr}");
        return None;
    }

    std::fs::read_to_string(&output_path).ok()
}

/// Run `mlir-translate --mlir-to-cpp` on MLIR text. Returns `None` if the tool
/// is unavailable or fails.
fn run_mlir_translate(mlir_text: &str, config: &PipelineConfig) -> Option<String> {
    std::fs::create_dir_all(&config.work_dir).ok()?;
    let input_path = config.work_dir.join("for_translate.mlir");

    std::fs::write(&input_path, mlir_text).ok()?;

    let output = Command::new(&config.mlir_translate)
        .arg("--mlir-to-cpp")
        .arg(&input_path)
        .output()
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("mlir-translate failed: {stderr}");
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Generate the complete set of files for the MLIR-backed logic crate.
///
/// Produces a safe Rust-only logic crate (no C FFI, no `unsafe`).
/// The `.mlir` source is preserved for debugging / optimization analysis.
///
/// Returns `(path, content)` pairs suitable for inclusion in a
/// `GeneratedWorkspace`.
pub fn generate_mlir_logic_files(
    snap: &GraphSnapshot,
    config: &PipelineConfig,
) -> Result<Vec<(String, String)>, String> {
    let pipeline = run_pipeline(snap, config)?;
    let mut files = Vec::new();

    // Preserve the raw .mlir for debugging
    files.push((
        "logic/mlir/graph.mlir".to_string(),
        pipeline.mlir_text.clone(),
    ));

    // Optionally preserve C translation for reference (not compiled)
    if let Some(c_src) = pipeline.c_source {
        files.push(("logic/mlir/logic.c.reference".to_string(), c_src));
    }

    // Emit Cargo.toml (pure Rust, no cc dependency)
    files.push(("logic/Cargo.toml".to_string(), generate_logic_cargo_toml()));

    // Emit ffi.rs with safe State struct
    let state_fields = collect_state_fields(snap);
    let ffi_rs = peripherals::generate_ffi_rs(&state_fields);
    files.push(("logic/src/ffi.rs".to_string(), ffi_rs));

    // Emit lib.rs — safe tick function using dataflow-rt Peripherals trait
    files.push((
        "logic/src/lib.rs".to_string(),
        generate_logic_lib_rs(&state_fields),
    ));

    Ok(files)
}

/// Collect state field names and types from the graph snapshot.
fn collect_state_fields(snap: &GraphSnapshot) -> Vec<(String, &'static str)> {
    let mut fields = Vec::new();
    for block in &snap.blocks {
        let bt = block.block_type.as_str();
        if matches!(bt, "plot" | "json_encode" | "json_decode") {
            continue;
        }
        for (port_idx, _port) in block.outputs.iter().enumerate() {
            fields.push((format!("out_{}_p{}", block.id.0, port_idx), "f64"));
        }
    }
    fields
}

fn generate_logic_cargo_toml() -> String {
    r#"[package]
name = "logic"
version = "0.1.0"
edition = "2021"

[lib]
name = "logic"

[dependencies]
dataflow-rt = { path = "../dataflow-rt", default-features = false }
"#
    .to_string()
}

fn generate_logic_lib_rs(state_fields: &[(String, &str)]) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    writeln!(out, "//! Logic crate — safe Rust tick function.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![no_std]").unwrap();
    writeln!(out, "#![forbid(unsafe_code)]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "pub mod ffi;").unwrap();
    writeln!(out, "pub use ffi::State;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use dataflow_rt::Peripherals;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Evaluate one tick of the dataflow graph.").unwrap();
    writeln!(
        out,
        "pub fn tick<P: Peripherals>(hw: &mut P, state: &mut State) {{"
    )
    .unwrap();

    // Generate stub tick body — each output field is set to its default
    for (name, _ty) in state_fields {
        writeln!(out, "    let _ = &state.{name};").unwrap();
    }
    if state_fields.is_empty() {
        writeln!(out, "    let _ = hw;").unwrap();
        writeln!(out, "    let _ = state;").unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

// ── Typed IR pipeline ────────────────────────────────────────────

/// Result of the typed IR pipeline.
#[derive(Debug)]
pub struct IrPipelineOutput {
    /// The typed IR module.
    pub ir_module: IrModule,
    /// MLIR text serialized from the typed IR.
    pub mlir_text: String,
    /// Generated safe Rust source code with callable objects.
    pub rust_source: String,
}

/// Run the typed IR pipeline: lower → IR → MLIR text + Rust source.
///
/// This is the new pipeline that replaces string-based MLIR generation.
/// The IR can be printed to MLIR text for debugging and emitted as
/// safe Rust code with callable objects for runtime interpretation.
pub fn run_ir_pipeline(snap: &GraphSnapshot) -> Result<IrPipelineOutput, String> {
    // Step 1: Lower to typed IR
    let ir_module = lower::lower_graph_ir(snap)?;

    // Step 2: Print to MLIR text (for debugging)
    let mlir_text = printer::print_mlir(&ir_module);

    // Step 3: Emit safe Rust code
    let rust_source = emit_rust::emit_rust(&ir_module);

    Ok(IrPipelineOutput {
        ir_module,
        mlir_text,
        rust_source,
    })
}

/// Generate logic crate files using the typed IR pipeline.
///
/// Produces:
/// - `logic/mlir/graph.mlir` — MLIR text for debugging
/// - `logic/Cargo.toml` — Pure Rust crate manifest
/// - `logic/src/lib.rs` — Generated Rust with callable objects
pub fn generate_ir_logic_files(snap: &GraphSnapshot) -> Result<Vec<(String, String)>, String> {
    let output = run_ir_pipeline(snap)?;

    let files = vec![
        // MLIR for debugging
        ("logic/mlir/graph.mlir".to_string(), output.mlir_text),
        // Cargo.toml (simpler — no dataflow-rt dep needed since code is self-contained)
        ("logic/Cargo.toml".to_string(), generate_ir_cargo_toml()),
        // Generated Rust source with HardwareBridge trait + State + tick()
        ("logic/src/lib.rs".to_string(), output.rust_source),
    ];

    Ok(files)
}

fn generate_ir_cargo_toml() -> String {
    r#"[package]
name = "logic"
version = "0.1.0"
edition = "2021"

[lib]
name = "logic"
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{BlockId, BlockSnapshot, PortDef, PortKind};

    fn simple_graph() -> GraphSnapshot {
        GraphSnapshot {
            blocks: vec![BlockSnapshot {
                id: BlockId(1),
                block_type: "constant".to_string(),
                name: "const_42".to_string(),
                inputs: vec![],
                outputs: vec![PortDef {
                    name: "out".to_string(),
                    kind: PortKind::Float,
                }],
                config: serde_json::json!({"value": 42.0}),
                is_delay: false,
            }],
            channels: vec![],
        }
    }

    #[test]
    fn lower_only_produces_mlir() {
        let snap = simple_graph();
        let output = lower_only(&snap).unwrap();
        assert!(output.mlir_text.contains("arith.constant"));
        assert!(output.mlir_optimized.is_none());
        assert!(output.c_source.is_none());
    }

    #[test]
    fn generate_files_includes_all() {
        let snap = simple_graph();
        let config = PipelineConfig::default();
        let files = generate_mlir_logic_files(&snap, &config).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"logic/mlir/graph.mlir"));
        assert!(paths.contains(&"logic/Cargo.toml"));
        assert!(paths.contains(&"logic/src/ffi.rs"));
        assert!(paths.contains(&"logic/src/lib.rs"));
    }

    #[test]
    fn generated_logic_crate_is_safe() {
        let snap = simple_graph();
        let config = PipelineConfig::default();
        let files = generate_mlir_logic_files(&snap, &config).unwrap();
        for (path, content) in &files {
            if path.ends_with(".rs") {
                assert!(
                    !content.contains("unsafe {") && !content.contains("unsafe fn"),
                    "file {path} contains unsafe code"
                );
                assert!(
                    !content.contains("#[no_mangle]"),
                    "file {path} contains #[no_mangle]"
                );
                assert!(
                    !content.contains("extern \"C\""),
                    "file {path} contains extern \"C\""
                );
            }
        }
        // No C files or headers in the output
        assert!(
            !files
                .iter()
                .any(|(p, _)| p.ends_with(".h") || p.ends_with(".c")),
            "logic crate must not contain C files"
        );
    }

    #[test]
    fn lib_rs_forbids_unsafe() {
        let snap = simple_graph();
        let config = PipelineConfig::default();
        let files = generate_mlir_logic_files(&snap, &config).unwrap();
        let lib = files.iter().find(|(p, _)| p.ends_with("lib.rs")).unwrap();
        assert!(lib.1.contains("forbid(unsafe_code)"));
    }
}
