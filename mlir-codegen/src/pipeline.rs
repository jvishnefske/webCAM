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
//!   mlir-translate        ← EmitC → C source
//!       │
//!       ▼
//!   (logic.c, peripherals.h)
//! ```

use std::path::PathBuf;
use std::process::Command;

use crate::lower::{self, GraphSnapshot};
use crate::peripherals;

/// Result of the MLIR compilation pipeline.
#[derive(Debug)]
pub struct PipelineOutput {
    /// Generated `.mlir` text (before optimization).
    pub mlir_text: String,
    /// Optimized `.mlir` text (after mlir-opt), if mlir-opt was available.
    pub mlir_optimized: Option<String>,
    /// Generated C source code, if mlir-translate was available.
    pub c_source: Option<String>,
    /// Generated `peripherals.h` header.
    pub peripherals_h: String,
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

    // Step 2: Generate peripherals.h
    let peripherals_h = peripherals::generate_peripherals_h();

    // Step 3: Optimize with mlir-opt (optional)
    let mlir_optimized = run_mlir_opt(&mlir_text, config);

    // Step 4: Translate to C via EmitC (optional)
    let source_mlir = mlir_optimized.as_deref().unwrap_or(&mlir_text);
    let c_source = run_mlir_translate(source_mlir, config);

    Ok(PipelineOutput {
        mlir_text,
        mlir_optimized,
        c_source,
        peripherals_h,
    })
}

/// Lower a graph to MLIR text only (no external tools needed).
pub fn lower_only(snap: &GraphSnapshot) -> Result<PipelineOutput, String> {
    let mlir_text = lower::lower_graph(snap)?;
    let peripherals_h = peripherals::generate_peripherals_h();

    Ok(PipelineOutput {
        mlir_text,
        mlir_optimized: None,
        c_source: None,
        peripherals_h,
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
/// Returns `(path, content)` pairs suitable for inclusion in a
/// `GeneratedWorkspace`.
pub fn generate_mlir_logic_files(
    snap: &GraphSnapshot,
    config: &PipelineConfig,
) -> Result<Vec<(String, String)>, String> {
    let pipeline = run_pipeline(snap, config)?;
    let mut files = Vec::new();

    // Always emit the raw .mlir for debugging
    files.push((
        "logic/csrc/graph.mlir".to_string(),
        pipeline.mlir_text.clone(),
    ));

    // Emit peripherals.h
    files.push((
        "logic/csrc/peripherals.h".to_string(),
        pipeline.peripherals_h,
    ));

    // Emit logic.c — either from mlir-translate or a fallback stub
    let c_source = pipeline
        .c_source
        .unwrap_or_else(|| generate_fallback_c(&pipeline.mlir_text));
    files.push(("logic/csrc/logic.c".to_string(), c_source));

    // Emit build.rs for cc crate
    files.push(("logic/build.rs".to_string(), generate_logic_build_rs()));

    // Emit Cargo.toml with cc build-dependency
    files.push((
        "logic/Cargo.toml".to_string(),
        generate_logic_cargo_toml_mlir(),
    ));

    // Emit ffi.rs with safe State struct (no repr(C), no extern)
    let state_fields = collect_state_fields(snap);
    let ffi_rs = peripherals::generate_ffi_rs(&state_fields);
    files.push(("logic/src/ffi.rs".to_string(), ffi_rs));

    // Emit lib.rs that re-exports ffi
    files.push(("logic/src/lib.rs".to_string(), generate_logic_lib_rs_mlir()));

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
            fields.push((format!("out_{}_p{}", block.id, port_idx), "f64"));
        }
    }
    fields
}

fn generate_fallback_c(_mlir_text: &str) -> String {
    let mut out = String::new();
    out.push_str("// Fallback: MLIR tools not available.\n");
    out.push_str("// The .mlir source is preserved in graph.mlir for manual compilation.\n");
    out.push_str("#include \"peripherals.h\"\n\n");
    out.push_str("// TODO: compile graph.mlir with mlir-opt + mlir-translate\n");
    out.push_str("void tick(void* state) {\n");
    out.push_str("    // stub — replace with mlir-translate output\n");
    out.push_str("    (void)state;\n");
    out.push_str("}\n");
    out
}

fn generate_logic_build_rs() -> String {
    r#"fn main() {
    cc::Build::new()
        .file("csrc/logic.c")
        .include("csrc")
        .opt_level(2)
        .compile("logic");
}
"#
    .to_string()
}

fn generate_logic_cargo_toml_mlir() -> String {
    r#"[package]
name = "logic"
version = "0.1.0"
edition = "2021"

[lib]
name = "logic"

[dependencies]

[build-dependencies]
cc = "1"
"#
    .to_string()
}

fn generate_logic_lib_rs_mlir() -> String {
    r#"//! Logic crate — MLIR-generated C tick function via FFI.

#![no_std]

pub mod ffi;

pub use ffi::{State, tick_safe};
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{BlockSnapshot, PortDef};
    use module_traits::value::PortKind;

    fn simple_graph() -> GraphSnapshot {
        GraphSnapshot {
            blocks: vec![BlockSnapshot {
                id: 1,
                block_type: "constant".to_string(),
                name: "const_42".to_string(),
                inputs: vec![],
                outputs: vec![PortDef {
                    name: "out".to_string(),
                    kind: PortKind::Float,
                }],
                config: serde_json::json!({"value": 42.0}),
                output_values: vec![],
                custom_codegen: None,
            }],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        }
    }

    #[test]
    fn lower_only_produces_mlir() {
        let snap = simple_graph();
        let output = lower_only(&snap).unwrap();
        assert!(output.mlir_text.contains("dataflow.constant"));
        assert!(output.mlir_optimized.is_none());
        assert!(output.c_source.is_none());
        assert!(output.peripherals_h.contains("hw_adc_read"));
    }

    #[test]
    fn generate_files_includes_all() {
        let snap = simple_graph();
        let config = PipelineConfig::default();
        let files = generate_mlir_logic_files(&snap, &config).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"logic/csrc/graph.mlir"));
        assert!(paths.contains(&"logic/csrc/peripherals.h"));
        assert!(paths.contains(&"logic/csrc/logic.c"));
        assert!(paths.contains(&"logic/build.rs"));
        assert!(paths.contains(&"logic/Cargo.toml"));
        assert!(paths.contains(&"logic/src/ffi.rs"));
        assert!(paths.contains(&"logic/src/lib.rs"));
    }

    #[test]
    fn fallback_c_is_valid() {
        let c = generate_fallback_c("// some mlir");
        assert!(c.contains("void tick("));
        assert!(c.contains("#include \"peripherals.h\""));
    }

    #[test]
    fn build_rs_uses_cc() {
        let rs = generate_logic_build_rs();
        assert!(rs.contains("cc::Build::new()"));
        assert!(rs.contains("logic.c"));
    }
}
