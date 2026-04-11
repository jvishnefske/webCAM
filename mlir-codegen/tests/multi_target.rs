//! Multi-target codegen integration tests for the mlir-codegen crate.
//!
//! These tests exercise the MLIR lowering pipeline and the DAG runtime with
//! graphs that represent realistic embedded control patterns:
//!
//! - RP2040: ADC-source -> gain -> PWM-sink (closed-loop control)
//! - STM32F4: constant -> gain -> clamp -> subtract (PID-like datapath)
//! - Runtime with a mock `HardwareBridge` that injects ADC readings
//! - Runtime pub/sub round-trip through `HardwareBridge::subscribe` / `publish`

use std::cell::RefCell;

use mlir_codegen::lower::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot, PortDef, PortKind};
use mlir_codegen::{build_runtime_graph, HwBridge, NullHw};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a block snapshot with no inputs (source-style).
fn source_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id: BlockId(id),
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
        inputs: vec![],
        outputs: vec![PortDef {
            name: "out".to_string(),
            kind: PortKind::Float,
        }],
        config,
        is_delay: false,
    }
}

/// Create a single-input, single-output processing block.
fn process_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id: BlockId(id),
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
        inputs: vec![PortDef {
            name: "in".to_string(),
            kind: PortKind::Float,
        }],
        outputs: vec![PortDef {
            name: "out".to_string(),
            kind: PortKind::Float,
        }],
        config,
        is_delay: false,
    }
}

/// Create a sink block (one input, no outputs).
fn sink_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id: BlockId(id),
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
        inputs: vec![PortDef {
            name: "in".to_string(),
            kind: PortKind::Float,
        }],
        outputs: vec![],
        config,
        is_delay: false,
    }
}

/// Create a two-input, one-output block (add, subtract, multiply).
fn dual_input_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id: BlockId(id),
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
        inputs: vec![
            PortDef {
                name: "a".to_string(),
                kind: PortKind::Float,
            },
            PortDef {
                name: "b".to_string(),
                kind: PortKind::Float,
            },
        ],
        outputs: vec![PortDef {
            name: "out".to_string(),
            kind: PortKind::Float,
        }],
        config,
        is_delay: false,
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
    }
}

/// Build a JSON block object string for use in runtime tests.
fn json_block(
    id: u32,
    block_type: &str,
    inputs: &[&str],
    outputs: &[&str],
    config: &str,
) -> String {
    let inputs_json: Vec<String> = inputs
        .iter()
        .map(|name| format!(r#"{{"name": "{name}", "kind": "Float"}}"#))
        .collect();
    let outputs_json: Vec<String> = outputs
        .iter()
        .map(|name| format!(r#"{{"name": "{name}", "kind": "Float"}}"#))
        .collect();
    format!(
        r#"{{"id": {id}, "block_type": "{block_type}", "name": "{block_type}_{id}", "inputs": [{inputs}], "outputs": [{outputs}], "config": {config}, "output_values": []}}"#,
        inputs = inputs_json.join(", "),
        outputs = outputs_json.join(", "),
    )
}

/// Build a JSON channel object string.
fn json_channel(id: u32, from: u32, from_port: usize, to: u32, to_port: usize) -> String {
    format!(
        r#"{{"id": {id}, "from_block": {from}, "from_port": {from_port}, "to_block": {to}, "to_port": {to_port}}}"#
    )
}

/// Build a full JSON graph snapshot string.
fn json_graph(blocks: &[String], channels: &[String]) -> String {
    format!(
        r#"{{"blocks": [{blocks}], "channels": [{channels}], "tick_count": 0, "time": 0.0}}"#,
        blocks = blocks.join(", "),
        channels = channels.join(", "),
    )
}

// ---------------------------------------------------------------------------
// Test 1: RP2040 ADC -> Gain -> PWM graph (MLIR lowering)
// ---------------------------------------------------------------------------

/// Graph: adc_source(ch=0) -> gain(factor=2.5) -> pwm_sink(ch=1)
///
/// This represents a typical RP2040 closed-loop control pattern where an
/// ADC reading is scaled and fed to a PWM output.
#[test]
fn test_rp2040_adc_pwm_graph() {
    let snap = make_snap(
        vec![
            source_block(1, "adc_source", serde_json::json!({"channel": 0})),
            process_block(2, "gain", serde_json::json!({"param1": 2.5})),
            sink_block(3, "pwm_sink", serde_json::json!({"channel": 1})),
        ],
        vec![
            make_channel(1, 1, 0, 2, 0), // adc -> gain
            make_channel(2, 2, 0, 3, 0), // gain -> pwm
        ],
    );

    let mlir = mlir_codegen::graph_to_mlir(&snap).expect("lowering should succeed");

    // Verify the MLIR contains the expected hardware ops
    assert!(
        mlir.contains("func.call @adc_read"),
        "MLIR should contain func.call @adc_read for the ADC source; got:\n{mlir}"
    );
    assert!(
        mlir.contains("func.call @pwm_write"),
        "MLIR should contain func.call @pwm_write for the PWM sink; got:\n{mlir}"
    );
    assert!(
        mlir.contains("arith.mulf"),
        "MLIR should contain arith.mulf for the scaling block; got:\n{mlir}"
    );

    // Verify channel configuration appears in the function names
    assert!(
        mlir.contains("@adc_read_0"),
        "adc_read should have channel 0 in function name; got:\n{mlir}"
    );
    assert!(
        mlir.contains("@pwm_write_1"),
        "pwm_write should have channel 1 in function name; got:\n{mlir}"
    );

    // Verify gain factor
    assert!(
        mlir.contains("2.5"),
        "gain op should contain the factor 2.5; got:\n{mlir}"
    );

    // Verify wiring: gain should reference adc output SSA name
    assert!(
        mlir.contains("arith.mulf %v1_p0"),
        "gain input should be wired to adc output %v1_p0; got:\n{mlir}"
    );

    // Verify structural MLIR elements
    assert!(
        mlir.contains("func.func @tick"),
        "lowered MLIR must contain a tick function; got:\n{mlir}"
    );
    assert!(
        mlir.contains("module"),
        "lowered MLIR must be wrapped in a module; got:\n{mlir}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: STM32 multi-block PID-like graph (MLIR lowering)
// ---------------------------------------------------------------------------

/// Graph:
///   constant(setpoint=100.0) ─────────────┐
///   constant(measured=60.0) -> gain(0.5) -> subtract -> clamp(0, 255)
///
/// This represents a PID-like error computation:
///   error = setpoint - (measured * gain)
///   output = clamp(error, 0, 255)
#[test]
fn test_stm32_multi_block_graph() {
    let snap = make_snap(
        vec![
            source_block(1, "constant", serde_json::json!({"value": 100.0})),
            source_block(2, "constant", serde_json::json!({"value": 60.0})),
            process_block(3, "gain", serde_json::json!({"param1": 0.5})),
            dual_input_block(4, "subtract", serde_json::json!({})),
            process_block(
                5,
                "clamp",
                serde_json::json!({"param1": 0.0, "param2": 255.0}),
            ),
        ],
        vec![
            make_channel(1, 2, 0, 3, 0), // measured -> gain
            make_channel(2, 1, 0, 4, 0), // setpoint -> subtract.a
            make_channel(3, 3, 0, 4, 1), // gain.out -> subtract.b
            make_channel(4, 4, 0, 5, 0), // subtract -> clamp
        ],
    );

    let mlir = mlir_codegen::graph_to_mlir(&snap).expect("lowering should succeed");

    // Verify all four op types are present
    assert!(
        mlir.contains("arith.constant"),
        "MLIR should contain arith.constant; got:\n{mlir}"
    );
    assert!(
        mlir.contains("arith.mulf"),
        "MLIR should contain arith.mulf for gain; got:\n{mlir}"
    );
    assert!(
        mlir.contains("arith.subf"),
        "MLIR should contain arith.subf for subtract; got:\n{mlir}"
    );
    assert!(
        mlir.contains("arith.minimumf") || mlir.contains("arith.maximumf"),
        "MLIR should contain arith.minimumf/maximumf for clamp; got:\n{mlir}"
    );

    // Verify the constant values appear
    assert!(
        mlir.contains("100"),
        "MLIR should contain setpoint value 100; got:\n{mlir}"
    );
    assert!(
        mlir.contains("60"),
        "MLIR should contain measured value 60; got:\n{mlir}"
    );

    // Verify clamp bounds
    assert!(
        mlir.contains("255"),
        "MLIR should contain clamp max 255; got:\n{mlir}"
    );

    // Verify wiring: subtract should reference both setpoint and gain outputs
    assert!(
        mlir.contains("arith.subf %v1_p0, %v3_p0"),
        "subtract should be wired to setpoint (%v1_p0) and gain output (%v3_p0); got:\n{mlir}"
    );

    // Verify wiring: clamp should reference subtract output
    assert!(
        mlir.contains("arith.minimumf %v4_p0"),
        "clamp should be wired to subtract output %v4_p0; got:\n{mlir}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Runtime with mock HwBridge (ADC -> Gain -> PWM)
// ---------------------------------------------------------------------------

/// Mock hardware bridge that returns a fixed ADC value and records PWM writes.
struct MockAdcPwmHardware {
    adc_value: f64,
    pwm_writes: RefCell<Vec<(u8, f64)>>,
}

impl MockAdcPwmHardware {
    fn new(adc_value: f64) -> Self {
        Self {
            adc_value,
            pwm_writes: RefCell::new(Vec::new()),
        }
    }

    fn pwm_writes(&self) -> Vec<(u8, f64)> {
        self.pwm_writes.borrow().clone()
    }
}

impl HwBridge for MockAdcPwmHardware {
    fn adc_read(&self, _channel: u8) -> f64 {
        self.adc_value
    }

    fn pwm_write(&mut self, channel: u8, duty: f64) {
        self.pwm_writes.borrow_mut().push((channel, duty));
    }
}

/// Build a compiled graph from adc_source -> gain(3.0) -> pwm_sink, inject a
/// known ADC value via the mock bridge, tick, and verify the PWM output equals
/// adc_value * gain_factor.
#[test]
fn test_runtime_with_hardware_bridge() {
    let json = json_graph(
        &[
            json_block(1, "adc_source", &[], &["out"], r#"{"channel": 0}"#),
            json_block(2, "gain", &["in"], &["out"], r#"{"param1": 3.0}"#),
            json_block(3, "pwm_sink", &["in"], &[], r#"{"channel": 1}"#),
        ],
        &[
            json_channel(1, 1, 0, 2, 0), // adc -> gain
            json_channel(2, 2, 0, 3, 0), // gain -> pwm
        ],
    );

    let mut graph =
        build_runtime_graph(&json).expect("build_runtime_graph should build from valid graph");
    assert!(
        graph.block_count() >= 3,
        "graph should have at least 3 blocks (gain expands to mulf + constant)"
    );

    let mut hw = MockAdcPwmHardware::new(1.5);
    graph.tick(1.0, &mut hw);

    // ADC reads 1.5, gain multiplies by 3.0, PWM should receive 4.5
    let writes = hw.pwm_writes();
    assert_eq!(
        writes.len(),
        1,
        "exactly one PWM write should occur per tick"
    );
    assert_eq!(writes[0].0, 1, "PWM channel should be 1");
    assert!(
        (writes[0].1 - 4.5).abs() < f64::EPSILON,
        "PWM duty should be 1.5 * 3.0 = 4.5, got {}",
        writes[0].1
    );
}

// ---------------------------------------------------------------------------
// Test 4: Runtime compiled graph from multi-block snapshot
// ---------------------------------------------------------------------------

/// Compile a constant -> gain chain and verify the result via the slot-based
/// compiled graph interface.
#[test]
fn test_runtime_constant_gain_chain() {
    let json = json_graph(
        &[
            json_block(1, "constant", &[], &["out"], r#"{"value": 25.0}"#),
            json_block(2, "gain", &["in"], &["out"], r#"{"param1": 2.0}"#),
        ],
        &[json_channel(1, 1, 0, 2, 0)],
    );

    let mut graph =
        build_runtime_graph(&json).expect("build_runtime_graph should build from valid graph");
    graph.tick(1.0, &mut NullHw);

    // Find the last allocated slot (the mulf output) — it should hold 25*2=50.
    // Scan slots for the expected value.
    let slot_count = graph.slot_count();
    let mut found_50 = false;
    for i in 0..slot_count as u16 {
        if (graph.read_slot(i) - 50.0).abs() < f64::EPSILON {
            found_50 = true;
            break;
        }
    }
    assert!(
        found_50,
        "should find 25.0 * 2.0 = 50.0 in one of the slots"
    );
}
