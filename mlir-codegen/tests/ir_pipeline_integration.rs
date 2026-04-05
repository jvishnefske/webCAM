//! Integration tests for the typed IR pipeline.
//!
//! Tests the complete flow: GraphSnapshot → typed IR → MLIR text → Rust source.

use mlir_codegen::lower::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot, PortDef};
use module_traits::value::PortKind;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_block(
    id: u32,
    block_type: &str,
    config: serde_json::Value,
    inputs: Vec<PortDef>,
    outputs: Vec<PortDef>,
) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: block_type.to_string(),
        name: format!("{}_{}", block_type, id),
        inputs,
        outputs,
        config,
        output_values: vec![],
        custom_codegen: None,
    }
}

fn float_port(name: &str) -> PortDef {
    PortDef {
        name: name.to_string(),
        kind: PortKind::Float,
    }
}

fn wire(id: u32, from_block: u32, from_port: usize, to_block: u32, to_port: usize) -> Channel {
    Channel {
        id: ChannelId(id),
        from_block: BlockId(from_block),
        from_port,
        to_block: BlockId(to_block),
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
// Test 1: Single constant → IR → MLIR text
// ---------------------------------------------------------------------------

#[test]
fn test_ir_constant_to_mlir() {
    let snap = make_snap(
        vec![make_block(
            1,
            "constant",
            serde_json::json!({"value": 42.0}),
            vec![],
            vec![float_port("out")],
        )],
        vec![],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let mlir = mlir_codegen::print_mlir(&ir);

    assert!(
        mlir.contains("arith.constant"),
        "MLIR should contain arith.constant; got:\n{mlir}"
    );
    assert!(
        mlir.contains("42"),
        "MLIR should contain the value 42; got:\n{mlir}"
    );
    assert!(
        mlir.contains("module"),
        "MLIR should contain module wrapper; got:\n{mlir}"
    );
    assert!(
        mlir.contains("func.func @tick"),
        "MLIR should contain func.func @tick; got:\n{mlir}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Constant → Gain chain → MLIR text
// ---------------------------------------------------------------------------

#[test]
fn test_ir_chain_to_mlir() {
    let snap = make_snap(
        vec![
            make_block(
                1,
                "constant",
                serde_json::json!({"value": 5.0}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                2,
                "gain",
                serde_json::json!({"param1": 2.0}),
                vec![float_port("in")],
                vec![float_port("out")],
            ),
        ],
        vec![wire(1, 1, 0, 2, 0)],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let mlir = mlir_codegen::print_mlir(&ir);

    // constant appears at least twice: the value 5.0 and the gain factor 2.0
    let constant_count = mlir.matches("arith.constant").count();
    assert!(
        constant_count >= 2,
        "MLIR should contain at least 2 arith.constant ops (value + gain factor); found {constant_count} in:\n{mlir}"
    );
    assert!(
        mlir.contains("arith.mulf"),
        "MLIR should contain arith.mulf for the gain; got:\n{mlir}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Single constant → IR → Rust source
// ---------------------------------------------------------------------------

#[test]
fn test_ir_to_rust_constant() {
    let snap = make_snap(
        vec![make_block(
            1,
            "constant",
            serde_json::json!({"value": 42.0}),
            vec![],
            vec![float_port("out")],
        )],
        vec![],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let rust = mlir_codegen::emit_rust(&ir);

    assert!(
        rust.contains("#![forbid(unsafe_code)]"),
        "Rust should contain #![forbid(unsafe_code)]; got:\n{rust}"
    );
    assert!(
        rust.contains("pub trait HardwareBridge"),
        "Rust should contain pub trait HardwareBridge; got:\n{rust}"
    );
    assert!(
        rust.contains("pub struct State"),
        "Rust should contain pub struct State; got:\n{rust}"
    );
    assert!(
        rust.contains("pub fn tick"),
        "Rust should contain pub fn tick; got:\n{rust}"
    );
    assert!(
        rust.contains("42.0"),
        "Rust should contain the constant value 42.0; got:\n{rust}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Constant → Gain chain → Rust source
// ---------------------------------------------------------------------------

#[test]
fn test_ir_to_rust_chain() {
    let snap = make_snap(
        vec![
            make_block(
                1,
                "constant",
                serde_json::json!({"value": 5.0}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                2,
                "gain",
                serde_json::json!({"param1": 2.0}),
                vec![float_port("in")],
                vec![float_port("out")],
            ),
        ],
        vec![wire(1, 1, 0, 2, 0)],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let rust = mlir_codegen::emit_rust(&ir);

    assert!(
        rust.contains("5.0"),
        "Rust should contain the constant value 5.0; got:\n{rust}"
    );
    assert!(
        rust.contains("2.0"),
        "Rust should contain the gain factor 2.0; got:\n{rust}"
    );
    assert!(
        rust.contains("*"),
        "Rust should contain multiplication operator *; got:\n{rust}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: ADC → Gain → PWM hardware chain → Rust source
// ---------------------------------------------------------------------------

#[test]
fn test_ir_to_rust_hardware() {
    let snap = make_snap(
        vec![
            make_block(
                1,
                "adc_source",
                serde_json::json!({"channel": 3}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                2,
                "gain",
                serde_json::json!({"param1": 2.0}),
                vec![float_port("in")],
                vec![float_port("out")],
            ),
            make_block(
                3,
                "pwm_sink",
                serde_json::json!({"channel": 1}),
                vec![float_port("in")],
                vec![],
            ),
        ],
        vec![wire(1, 1, 0, 2, 0), wire(2, 2, 0, 3, 0)],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let rust = mlir_codegen::emit_rust(&ir);

    assert!(
        rust.contains("hw.adc_read(3)"),
        "Rust should contain hw.adc_read(3); got:\n{rust}"
    );
    assert!(
        rust.contains("hw.pwm_write(1,"),
        "Rust should contain hw.pwm_write(1,; got:\n{rust}"
    );
}

// ---------------------------------------------------------------------------
// Test 6: PubSub source → PubSub sink → Rust source
// ---------------------------------------------------------------------------

#[test]
fn test_ir_to_rust_pubsub() {
    let snap = make_snap(
        vec![
            make_block(
                1,
                "pubsub_source",
                serde_json::json!({"topic": "sensor/temp"}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                2,
                "pubsub_sink",
                serde_json::json!({"topic": "out/value"}),
                vec![float_port("in")],
                vec![],
            ),
        ],
        vec![wire(1, 1, 0, 2, 0)],
    );

    let ir = mlir_codegen::graph_to_ir(&snap).unwrap();
    let rust = mlir_codegen::emit_rust(&ir);

    assert!(
        rust.contains("hw.subscribe(\"sensor/temp\")"),
        "Rust should contain hw.subscribe(\"sensor/temp\"); got:\n{rust}"
    );
    assert!(
        rust.contains("hw.publish(\"out/value\""),
        "Rust should contain hw.publish(\"out/value\"; got:\n{rust}"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Full IR pipeline round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_ir_pipeline_round_trip() {
    let snap = make_snap(
        vec![
            make_block(
                1,
                "constant",
                serde_json::json!({"value": 10.0}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                2,
                "gain",
                serde_json::json!({"param1": 3.0}),
                vec![float_port("in")],
                vec![float_port("out")],
            ),
            make_block(
                3,
                "adc_source",
                serde_json::json!({"channel": 0}),
                vec![],
                vec![float_port("out")],
            ),
            make_block(
                4,
                "add",
                serde_json::json!({}),
                vec![float_port("a"), float_port("b")],
                vec![float_port("out")],
            ),
        ],
        vec![wire(1, 1, 0, 2, 0), wire(2, 2, 0, 4, 0), wire(3, 3, 0, 4, 1)],
    );

    let output = mlir_codegen::run_ir_pipeline(&snap).unwrap();

    // IR module has exactly one function
    assert_eq!(
        output.ir_module.funcs.len(),
        1,
        "IR module should have exactly 1 function"
    );

    // MLIR text is non-empty and contains expected ops
    assert!(
        !output.mlir_text.is_empty(),
        "MLIR text should be non-empty"
    );
    assert!(
        output.mlir_text.contains("arith.constant"),
        "MLIR text should contain arith.constant; got:\n{}",
        output.mlir_text
    );
    assert!(
        output.mlir_text.contains("arith.mulf"),
        "MLIR text should contain arith.mulf (from gain); got:\n{}",
        output.mlir_text
    );
    assert!(
        output.mlir_text.contains("arith.addf"),
        "MLIR text should contain arith.addf (from add); got:\n{}",
        output.mlir_text
    );
    assert!(
        output.mlir_text.contains("dataflow.adc_read"),
        "MLIR text should contain dataflow.adc_read; got:\n{}",
        output.mlir_text
    );

    // Rust source is non-empty and contains tick function
    assert!(
        !output.rust_source.is_empty(),
        "Rust source should be non-empty"
    );
    assert!(
        output.rust_source.contains("fn tick"),
        "Rust source should contain fn tick; got:\n{}",
        output.rust_source
    );
}

// ---------------------------------------------------------------------------
// Test 8: generate_ir_logic_files produces correct files
// ---------------------------------------------------------------------------

#[test]
fn test_ir_generate_logic_files() {
    let snap = make_snap(
        vec![make_block(
            1,
            "constant",
            serde_json::json!({"value": 7.0}),
            vec![],
            vec![float_port("out")],
        )],
        vec![],
    );

    let files = mlir_codegen::generate_ir_logic_files(&snap).unwrap();
    let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

    // Must include the 3 expected files
    assert!(
        paths.contains(&"logic/mlir/graph.mlir"),
        "Should contain logic/mlir/graph.mlir; got: {paths:?}"
    );
    assert!(
        paths.contains(&"logic/Cargo.toml"),
        "Should contain logic/Cargo.toml; got: {paths:?}"
    );
    assert!(
        paths.contains(&"logic/src/lib.rs"),
        "Should contain logic/src/lib.rs; got: {paths:?}"
    );

    // Cargo.toml contains crate name
    let cargo_toml = files
        .iter()
        .find(|(p, _)| p == "logic/Cargo.toml")
        .map(|(_, c)| c.as_str())
        .unwrap();
    assert!(
        cargo_toml.contains("name = \"logic\""),
        "Cargo.toml should contain name = \"logic\"; got:\n{cargo_toml}"
    );

    // lib.rs contains forbid(unsafe_code)
    let lib_rs = files
        .iter()
        .find(|(p, _)| p == "logic/src/lib.rs")
        .map(|(_, c)| c.as_str())
        .unwrap();
    assert!(
        lib_rs.contains("#![forbid(unsafe_code)]"),
        "lib.rs should contain #![forbid(unsafe_code)]; got:\n{lib_rs}"
    );

    // No file should contain unsafe code
    for (path, content) in &files {
        assert!(
            !content.contains("unsafe {") && !content.contains("unsafe fn"),
            "File {path} should not contain unsafe code"
        );
    }
}
