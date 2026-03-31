//! Integration tests for the MLIR pipeline entry points.

use mlir_codegen::lower::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot, PortDef};
use mlir_codegen::pipeline::{PipelineConfig, PipelineOutput};
use module_traits::value::PortKind;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: block_type.to_string(),
        name: format!("test_{block_type}_{id}"),
        inputs: vec![],
        outputs: vec![PortDef {
            name: "out".to_string(),
            kind: PortKind::Float,
        }],
        config,
        output_values: vec![],
        custom_codegen: None,
    }
}

fn make_channel(id: u32, from: u32, from_port: usize, to: u32, to_port: usize) -> Channel {
    Channel {
        id: ChannelId(id),
        from_block: BlockId(from),
        from_port,
        to_block: BlockId(to),
        to_port,
    }
}

fn make_snap(blocks: Vec<BlockSnapshot>, channels: Vec<Channel>) -> GraphSnapshot {
    GraphSnapshot {
        blocks,
        channels,
        tick_count: 0,
        time: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_pipeline_default_config() {
    let snap = make_snap(
        vec![make_block(
            1,
            "constant",
            serde_json::json!({"value": 3.25}),
        )],
        vec![],
    );
    let config = PipelineConfig::default();
    let output: PipelineOutput =
        mlir_codegen::pipeline::run_pipeline(&snap, &config).expect("run_pipeline should succeed");

    // The MLIR text should always be produced regardless of external tool availability
    assert!(
        output.mlir_text.contains("dataflow.constant"),
        "pipeline output should contain the lowered constant op"
    );
    assert!(
        output.mlir_text.contains("3.25"),
        "pipeline output should contain the constant value"
    );
    assert!(
        output.peripherals_h.contains("hw_adc_read"),
        "peripherals header should include hardware stubs"
    );
}

#[test]
fn test_pipeline_multi_block_chain() {
    // constant(5.0) -> gain(2.0) -> output chain
    let mut gain_block = make_block(2, "gain", serde_json::json!({"param1": 2.0}));
    gain_block.inputs = vec![PortDef {
        name: "in".to_string(),
        kind: PortKind::Float,
    }];

    let snap = make_snap(
        vec![
            make_block(1, "constant", serde_json::json!({"value": 5.0})),
            gain_block,
        ],
        vec![make_channel(1, 1, 0, 2, 0)],
    );

    let config = PipelineConfig::default();
    let output =
        mlir_codegen::pipeline::run_pipeline(&snap, &config).expect("run_pipeline should succeed");

    assert!(
        output.mlir_text.contains("dataflow.constant"),
        "MLIR should contain the constant op; got:\n{}",
        output.mlir_text
    );
    assert!(
        output.mlir_text.contains("dataflow.gain"),
        "MLIR should contain the gain op; got:\n{}",
        output.mlir_text
    );
    // Verify the wiring: gain should reference the constant's SSA value
    assert!(
        output.mlir_text.contains("%v1_p0"),
        "gain input should reference constant output SSA name %v1_p0; got:\n{}",
        output.mlir_text
    );
    assert!(
        output.mlir_text.contains("dataflow.gain(%v1_p0)"),
        "gain op should be wired to constant output; got:\n{}",
        output.mlir_text
    );
}
