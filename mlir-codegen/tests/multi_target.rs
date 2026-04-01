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

use mlir_codegen::lower::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot, PortDef};
use mlir_codegen::{build_runtime, HardwareBridge};
use module_traits::value::PortKind;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a block snapshot with no inputs (source-style).
fn source_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
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

/// Create a single-input, single-output processing block.
fn process_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id,
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
        output_values: vec![],
        custom_codegen: None,
    }
}

/// Create a sink block (one input, no outputs).
fn sink_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id,
        block_type: block_type.to_string(),
        name: format!("{block_type}_{id}"),
        inputs: vec![PortDef {
            name: "in".to_string(),
            kind: PortKind::Float,
        }],
        outputs: vec![],
        config,
        output_values: vec![],
        custom_codegen: None,
    }
}

/// Create a two-input, one-output block (add, subtract, multiply).
fn dual_input_block(id: u32, block_type: &str, config: serde_json::Value) -> BlockSnapshot {
    BlockSnapshot {
        id,
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
        mlir.contains("dataflow.adc_read"),
        "MLIR should contain dataflow.adc_read for the ADC source; got:\n{mlir}"
    );
    assert!(
        mlir.contains("dataflow.pwm_write"),
        "MLIR should contain dataflow.pwm_write for the PWM sink; got:\n{mlir}"
    );
    assert!(
        mlir.contains("dataflow.gain"),
        "MLIR should contain dataflow.gain for the scaling block; got:\n{mlir}"
    );

    // Verify channel configuration appears in the attributes
    assert!(
        mlir.contains("channel = 0 : i32"),
        "adc_read should have channel = 0 attribute; got:\n{mlir}"
    );
    assert!(
        mlir.contains("channel = 1 : i32"),
        "pwm_write should have channel = 1 attribute; got:\n{mlir}"
    );

    // Verify gain factor
    assert!(
        mlir.contains("2.5"),
        "gain op should contain the factor 2.5; got:\n{mlir}"
    );

    // Verify wiring: gain should reference adc output SSA name
    assert!(
        mlir.contains("dataflow.gain(%v1_p0)"),
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
        mlir.contains("dataflow.constant"),
        "MLIR should contain dataflow.constant; got:\n{mlir}"
    );
    assert!(
        mlir.contains("dataflow.gain"),
        "MLIR should contain dataflow.gain; got:\n{mlir}"
    );
    assert!(
        mlir.contains("dataflow.subtract"),
        "MLIR should contain dataflow.subtract; got:\n{mlir}"
    );
    assert!(
        mlir.contains("dataflow.clamp"),
        "MLIR should contain dataflow.clamp; got:\n{mlir}"
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
        mlir.contains("dataflow.subtract(%v1_p0, %v3_p0)"),
        "subtract should be wired to setpoint (%v1_p0) and gain output (%v3_p0); got:\n{mlir}"
    );

    // Verify wiring: clamp should reference subtract output
    assert!(
        mlir.contains("dataflow.clamp(%v4_p0)"),
        "clamp should be wired to subtract output %v4_p0; got:\n{mlir}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Runtime with mock HardwareBridge (ADC -> Gain -> PWM)
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

impl HardwareBridge for MockAdcPwmHardware {
    fn adc_read(&self, _channel: u8) -> f64 {
        self.adc_value
    }

    fn pwm_write(&mut self, channel: u8, duty: f64) {
        self.pwm_writes.borrow_mut().push((channel, duty));
    }
}

/// Build a runtime with adc_source -> gain(3.0) -> pwm_sink, inject a known
/// ADC value via the mock bridge, tick, and verify the PWM output equals
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

    let mut rt = build_runtime(&json).expect("runtime should build from valid graph");
    assert_eq!(rt.node_count(), 3, "graph has 3 blocks");

    let mut hw = MockAdcPwmHardware::new(1.5);
    rt.tick(&mut hw);

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

    // Verify intermediate values via read_output
    let adc_out = rt.read_output(1, 0);
    assert_eq!(
        adc_out,
        Some(1.5),
        "ADC block output should be the hardware reading"
    );

    let gain_out = rt.read_output(2, 0);
    assert_eq!(
        gain_out,
        Some(4.5),
        "gain block output should be adc * factor"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Runtime pub/sub round-trip
// ---------------------------------------------------------------------------

/// Mock hardware bridge that records publish calls and returns a configurable
/// subscribe value.
struct MockPubSubHardware {
    subscribe_values: RefCell<std::collections::HashMap<String, f64>>,
    published: RefCell<Vec<(String, f64)>>,
}

impl MockPubSubHardware {
    fn new() -> Self {
        Self {
            subscribe_values: RefCell::new(std::collections::HashMap::new()),
            published: RefCell::new(Vec::new()),
        }
    }

    fn published_values(&self) -> Vec<(String, f64)> {
        self.published.borrow().clone()
    }
}

impl HardwareBridge for MockPubSubHardware {
    fn subscribe(&self, topic: &str) -> f64 {
        self.subscribe_values
            .borrow()
            .get(topic)
            .copied()
            .unwrap_or(0.0)
    }

    fn publish(&mut self, topic: &str, value: f64) {
        self.published.borrow_mut().push((topic.to_string(), value));
    }
}

/// Build a runtime with pubsub_source -> gain(2.0) -> pubsub_sink.
///
/// The pubsub_source block acts as a mailbox: external values are injected
/// via `DagRuntime::receive()`. After ticking, the pubsub_sink block should
/// call `HardwareBridge::publish()` with the scaled value.
#[test]
fn test_runtime_pubsub_roundtrip() {
    let json = json_graph(
        &[
            json_block(
                1,
                "pubsub_source",
                &[],
                &["out"],
                r#"{"topic": "sensor/temperature"}"#,
            ),
            json_block(2, "gain", &["in"], &["out"], r#"{"param1": 2.0}"#),
            json_block(
                3,
                "pubsub_sink",
                &["in"],
                &[],
                r#"{"topic": "actuator/heater"}"#,
            ),
        ],
        &[
            json_channel(1, 1, 0, 2, 0), // pubsub_source -> gain
            json_channel(2, 2, 0, 3, 0), // gain -> pubsub_sink
        ],
    );

    let mut rt = build_runtime(&json).expect("runtime should build from valid graph");
    assert_eq!(rt.node_count(), 3, "graph has 3 blocks");

    // Verify the topic is registered for receiving
    let topics = rt.topics();
    assert!(
        topics.contains(&"sensor/temperature"),
        "runtime should register the pubsub_source topic; got: {topics:?}"
    );

    // Inject a value via the receive() API (simulating an incoming message)
    rt.receive("sensor/temperature", 25.0);

    let mut hw = MockPubSubHardware::new();
    rt.tick(&mut hw);

    // pubsub_source outputs 25.0, gain multiplies by 2.0 = 50.0,
    // pubsub_sink publishes 50.0 to "actuator/heater"
    let published = hw.published_values();
    assert_eq!(
        published.len(),
        1,
        "exactly one publish call should occur per tick"
    );
    assert_eq!(
        published[0].0, "actuator/heater",
        "published topic should match the sink's configured topic"
    );
    assert!(
        (published[0].1 - 50.0).abs() < f64::EPSILON,
        "published value should be 25.0 * 2.0 = 50.0, got {}",
        published[0].1
    );

    // Verify intermediate state
    let source_out = rt.read_output(1, 0);
    assert_eq!(
        source_out,
        Some(25.0),
        "pubsub_source should output the received value"
    );

    let gain_out = rt.read_output(2, 0);
    assert_eq!(
        gain_out,
        Some(50.0),
        "gain block should output source * factor"
    );

    // Tick again with a new value to verify state updates correctly
    rt.receive("sensor/temperature", 100.0);
    let mut hw2 = MockPubSubHardware::new();
    rt.tick(&mut hw2);

    let published2 = hw2.published_values();
    assert_eq!(published2.len(), 1);
    assert!(
        (published2[0].1 - 200.0).abs() < f64::EPSILON,
        "second tick should publish 100.0 * 2.0 = 200.0, got {}",
        published2[0].1
    );
}
