//! Integration tests for complete block workflows.
//!
//! These tests verify end-to-end dataflow through logic, embedded, and I/O blocks.

use rustsim::dataflow::block::{Module, Value};
use rustsim::dataflow::blocks;
use rustsim::dataflow::graph::DataflowGraph;
use rustsim::dataflow::sim_peripherals::WasmSimPeripherals;

// ── Helpers ───────────────────────────────────────────────────────

fn add(graph: &mut DataflowGraph, block_type: &str, config: &str) -> rustsim::dataflow::block::BlockId {
    let block = blocks::create_block(block_type, config).unwrap();
    graph.add_block(block)
}

fn output_value_by_id(graph: &DataflowGraph, id: u32, port: usize) -> Option<Value> {
    let snap = graph.snapshot();
    snap.blocks
        .iter()
        .find(|b| b.id == id)
        .and_then(|b| b.output_values.get(port).cloned().flatten())
}

// ── task-002: ADC → Gain → PWM workflow ──────────────────────────

#[test]
fn adc_gain_pwm_normal_mode() {
    let mut graph = DataflowGraph::new();

    let adc = add(&mut graph, "adc_source", r#"{"channel":0,"resolution_bits":12}"#);
    let gain = add(&mut graph, "gain", r#"{"op":"Gain","param1":2.0,"param2":0.0}"#);
    let pwm = add(&mut graph, "pwm_sink", r#"{"channel":0,"frequency_hz":1000}"#);

    graph.connect(adc, 0, gain, 0).unwrap();
    graph.connect(gain, 0, pwm, 0).unwrap();

    // Normal mode: ADC outputs 0.0, gain * 2 = 0.0, PWM consumes
    graph.tick(0.01);

    let snap = graph.snapshot();
    assert_eq!(snap.blocks.len(), 3);
    assert_eq!(snap.tick_count, 1);

    // Gain output should be 0.0 (ADC default * 2)
    let gain_out = output_value_by_id(&graph, gain.0, 0);
    assert_eq!(gain_out, Some(Value::Float(0.0)));
}

#[test]
fn adc_gain_pwm_simulation_mode() {
    let mut graph = DataflowGraph::new();
    graph.set_simulation_mode(true);
    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_adc_voltage(0, 1.5);
    graph.set_sim_peripherals(peripherals);

    let adc = add(&mut graph, "adc_source", r#"{"channel":0,"resolution_bits":12}"#);
    let gain = add(&mut graph, "gain", r#"{"op":"Gain","param1":2.0,"param2":0.0}"#);
    let _pwm = add(&mut graph, "pwm_sink", r#"{"channel":0,"frequency_hz":1000}"#);

    graph.connect(adc, 0, gain, 0).unwrap();
    graph.connect(gain, 0, _pwm, 0).unwrap();

    graph.tick(0.01);

    // ADC reads 1.5 from sim, gain * 2 = 3.0
    let gain_out = output_value_by_id(&graph, gain.0, 0);
    assert_eq!(gain_out, Some(Value::Float(3.0)));
}

// ── task-003: State machine transitions ──────────────────────────

#[test]
fn state_machine_transitions_in_graph() {
    let mut graph = DataflowGraph::new();

    let trigger = add(&mut graph, "constant", r#"{"value":1.0}"#);
    let state_init = add(&mut graph, "constant", r#"{"value":0.0}"#);
    let sm = add(
        &mut graph,
        "state_machine",
        r#"{"states":["idle","running","stopped"],"initial":"idle","transitions":[{"from":"idle","to":"running","guard":{"type":"GuardPort","port":0}},{"from":"running","to":"stopped","guard":{"type":"GuardPort","port":1}}]}"#,
    );

    // Connect state_init(0.0) to state_in (port 0)
    graph.connect(state_init, 0, sm, 0).unwrap();
    // Connect constant(1.0) to guard_0 (port 1) → idle→running
    graph.connect(trigger, 0, sm, 1).unwrap();

    graph.tick(0.01);

    let snap = graph.snapshot();
    let sm_block = snap.blocks.iter().find(|b| b.block_type == "state_machine").unwrap();
    // State machine should have outputs
    assert!(!sm_block.output_values.is_empty());

    // First output is next_state index: idle=0, running=1
    // With state_in=0 (idle) and guard_0=1.0, should transition to running (index 1)
    assert_eq!(sm_block.output_values[0], Some(Value::Float(1.0)));

    // Second tick: state_in is still 0.0 (constant), so it transitions idle→running again
    graph.tick(0.01);
    let snap2 = graph.snapshot();
    let sm_block2 = snap2.blocks.iter().find(|b| b.block_type == "state_machine").unwrap();
    assert_eq!(sm_block2.output_values[0], Some(Value::Float(1.0)));
}

// ── task-004: I/O blocks ─────────────────────────────────────────

#[test]
fn pubsub_source_sink_workflow() {
    let mut graph = DataflowGraph::new();

    let source = add(&mut graph, "pubsub_source", r#"{"topic":"test/val","port_kind":"Float"}"#);
    let sink = add(&mut graph, "pubsub_sink", r#"{"topic":"test/out","port_kind":"Float"}"#);

    graph.connect(source, 0, sink, 0).unwrap();
    graph.tick(0.01);

    let snap = graph.snapshot();
    assert_eq!(snap.blocks.len(), 2);
    // PubSub source with no value set outputs None
    let src_block = snap.blocks.iter().find(|b| b.id == source.0).unwrap();
    assert_eq!(src_block.output_values[0], None);
}

#[test]
fn gpio_passthrough_ticks() {
    let mut graph = DataflowGraph::new();

    let gpio_in = add(&mut graph, "gpio_in", r#"{"pin":2}"#);
    let gpio_out = add(&mut graph, "gpio_out", r#"{"pin":13}"#);

    graph.connect(gpio_in, 0, gpio_out, 0).unwrap();

    graph.tick(0.01);
    graph.tick(0.01);
    graph.tick(0.01);

    let snap = graph.snapshot();
    assert_eq!(snap.blocks.len(), 2);
    assert_eq!(snap.tick_count, 3);
}

#[test]
fn encoder_to_gain_workflow() {
    let mut graph = DataflowGraph::new();

    let encoder = add(&mut graph, "encoder", r#"{"channel":0}"#);
    let gain = add(&mut graph, "gain", r#"{"op":"Gain","param1":0.5,"param2":0.0}"#);

    // Encoder position (port 0) → gain
    graph.connect(encoder, 0, gain, 0).unwrap();

    graph.tick(0.01);

    // Encoder outputs 0.0 in normal mode, gain * 0.5 = 0.0
    let gain_out = output_value_by_id(&graph, gain.0, 0);
    assert_eq!(gain_out, Some(Value::Float(0.0)));
}

#[test]
fn encoder_simulation_velocity() {
    let mut graph = DataflowGraph::new();
    graph.set_simulation_mode(true);
    let peripherals = WasmSimPeripherals::new();
    graph.set_sim_peripherals(peripherals);

    let encoder = add(&mut graph, "encoder", r#"{"channel":0}"#);

    // Tick 1: position moves from 0 → 100, dt=0.01
    graph.with_sim_peripherals(|p| p.set_encoder_position(0, 100));
    graph.tick(0.01);

    let pos = output_value_by_id(&graph, encoder.0, 0);
    let vel = output_value_by_id(&graph, encoder.0, 1);
    assert_eq!(pos, Some(Value::Float(100.0)));
    // velocity = delta(100-0) / 0.01 = 10000.0
    assert_eq!(vel, Some(Value::Float(10000.0)));

    // Tick 2: position stays at 100, velocity should be 0
    graph.tick(0.01);
    let vel2 = output_value_by_id(&graph, encoder.0, 1);
    assert_eq!(vel2, Some(Value::Float(0.0)));
}

// ── task-005: Mixed workflows ────────────────────────────────────

#[test]
fn math_pipeline_add_clamp() {
    let mut graph = DataflowGraph::new();

    let c1 = add(&mut graph, "constant", r#"{"value":3.0}"#);
    let c2 = add(&mut graph, "constant", r#"{"value":4.0}"#);
    let sum = add(&mut graph, "add", "{}");
    let clamp = add(&mut graph, "clamp", r#"{"op":"Clamp","param1":0.0,"param2":5.0}"#);

    graph.connect(c1, 0, sum, 0).unwrap();
    graph.connect(c2, 0, sum, 1).unwrap();
    graph.connect(sum, 0, clamp, 0).unwrap();

    graph.tick(0.01);

    // 3 + 4 = 7, clamped to [0, 5] = 5.0
    let clamp_out = output_value_by_id(&graph, clamp.0, 0);
    assert_eq!(clamp_out, Some(Value::Float(5.0)));
}

#[test]
fn mixed_embedded_math_simulation() {
    let mut graph = DataflowGraph::new();
    graph.set_simulation_mode(true);
    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_adc_voltage(0, 0.75);
    graph.set_sim_peripherals(peripherals);

    let adc = add(&mut graph, "adc_source", r#"{"channel":0,"resolution_bits":12}"#);
    let gain = add(&mut graph, "gain", r#"{"op":"Gain","param1":2.0,"param2":0.0}"#);
    let clamp = add(&mut graph, "clamp", r#"{"op":"Clamp","param1":0.0,"param2":100.0}"#);
    let _pwm = add(&mut graph, "pwm_sink", r#"{"channel":0,"frequency_hz":1000}"#);

    graph.connect(adc, 0, gain, 0).unwrap();
    graph.connect(gain, 0, clamp, 0).unwrap();
    graph.connect(clamp, 0, _pwm, 0).unwrap();

    graph.tick(0.01);

    // ADC=0.75, gain*2=1.5, clamp[0,100]=1.5
    let gain_out = output_value_by_id(&graph, gain.0, 0);
    assert_eq!(gain_out, Some(Value::Float(1.5)));
    let clamp_out = output_value_by_id(&graph, clamp.0, 0);
    assert_eq!(clamp_out, Some(Value::Float(1.5)));
}

// ── task-006: Simulation mode toggle ─────────────────────────────

#[test]
fn simulation_mode_produces_real_values() {
    let mut graph = DataflowGraph::new();
    graph.set_simulation_mode(true);
    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_adc_voltage(0, 3.3);
    peripherals.set_gpio_state(5, true);
    graph.set_sim_peripherals(peripherals);

    let adc = add(&mut graph, "adc_source", r#"{"channel":0,"resolution_bits":12}"#);
    let gpio_in = add(&mut graph, "gpio_in", r#"{"pin":5}"#);

    graph.tick(0.01);

    // Simulation mode: ADC reads 3.3, GPIO reads 1.0 (true)
    let adc_out = output_value_by_id(&graph, adc.0, 0);
    assert_eq!(adc_out, Some(Value::Float(3.3)));
    let gpio_out = output_value_by_id(&graph, gpio_in.0, 0);
    assert_eq!(gpio_out, Some(Value::Float(1.0)));

    // Switch back to normal mode
    graph.set_simulation_mode(false);
    graph.tick(0.01);

    // Normal mode: ADC outputs 0.0, GPIO outputs 0.0
    let adc_out2 = output_value_by_id(&graph, adc.0, 0);
    assert_eq!(adc_out2, Some(Value::Float(0.0)));
    let gpio_out2 = output_value_by_id(&graph, gpio_in.0, 0);
    assert_eq!(gpio_out2, Some(Value::Float(0.0)));
}

// ── task-007: Register + StateMachine feedback loop ──────────────

/// End-to-end test: Register(z⁻¹) feeds state into StateMachine, which
/// outputs next_state back into Register, forming a feedback loop.
///
/// Topology:
///   trigger(1.0) ──→ sm.guard_0  (port 1, after state_in at port 0)
///   register.out ──→ sm.state_in (port 0)
///   sm.next_state ──→ register.in
///
/// StateMachine config:
///   states: ["idle", "running"]
///   idle → running when guard_0 > 0.5 (unconditionally true here)
///
/// Expected tick-by-tick behavior:
///   Tick 1: register outputs 0.0 (initial=idle). SM receives state_in=0 (idle),
///           guard_0=1.0 → transitions to running (next_state=1.0).
///           Register stores 1.0 (SM output from this tick).
///   Tick 2: register outputs 1.0 (stored from tick 1 = running).
///           SM receives state_in=1 (running). No transition from running defined.
///           next_state stays 1.0 (running).
///   Tick 3: Same — stays in running state.
#[test]
fn register_state_machine_feedback_loop() {
    let mut graph = DataflowGraph::new();

    // State machine: idle → running when guard_0 is high
    let sm_config = r#"{
        "states": ["idle", "running"],
        "initial": "idle",
        "transitions": [
            { "from": "idle", "to": "running", "guard": { "type": "GuardPort", "port": 0 } }
        ]
    }"#;
    let sm = add(&mut graph, "state_machine", sm_config);

    // Constant trigger: guard_0 always high (port 1 of SM after migration)
    let trigger = add(&mut graph, "constant", r#"{"value": 1.0}"#);

    // Register: holds the SM next_state output and feeds it back as state_in
    let register = add(&mut graph, "register", r#"{"initial_value": 0.0}"#);

    // Wire: register.out → sm.state_in (port 0)
    graph.connect(register, 0, sm, 0).unwrap();
    // Wire: trigger → sm.guard_0 (port 1 — the new layout with state_in at 0)
    graph.connect(trigger, 0, sm, 1).unwrap();
    // Wire: sm.next_state (port 0) → register.in (feedback)
    graph.connect(sm, 0, register, 0).unwrap();

    // Tick 1:
    // Register(init=0) outputs 0.0 → SM state_in=0 (idle).
    // trigger outputs 1.0 → guard_0 = 1.0 > 0.5 → transition idle→running.
    // SM next_state = 1.0. Register stores 1.0.
    graph.tick(0.01);
    let sm_next_state = output_value_by_id(&graph, sm.0, 0);
    assert_eq!(
        sm_next_state,
        Some(Value::Float(1.0)),
        "Tick 1: SM should transition idle→running (next_state=1.0)"
    );

    // active_idle (port 1) should be 0.0, active_running (port 2) should be 1.0
    let active_idle = output_value_by_id(&graph, sm.0, 1);
    let active_running = output_value_by_id(&graph, sm.0, 2);
    assert_eq!(active_idle, Some(Value::Float(0.0)), "Tick 1: active_idle=0");
    assert_eq!(active_running, Some(Value::Float(1.0)), "Tick 1: active_running=1");

    // Tick 2:
    // Register outputs 1.0 (stored from tick 1) → SM state_in=1 (running).
    // No transition from running defined → stays in running.
    // SM next_state = 1.0.
    graph.tick(0.01);
    let sm_next_state2 = output_value_by_id(&graph, sm.0, 0);
    assert_eq!(
        sm_next_state2,
        Some(Value::Float(1.0)),
        "Tick 2: SM should stay in running (next_state=1.0)"
    );

    let active_running2 = output_value_by_id(&graph, sm.0, 2);
    assert_eq!(active_running2, Some(Value::Float(1.0)), "Tick 2: active_running=1");

    // Tick 3: still running
    graph.tick(0.01);
    let sm_next_state3 = output_value_by_id(&graph, sm.0, 0);
    assert_eq!(
        sm_next_state3,
        Some(Value::Float(1.0)),
        "Tick 3: SM should remain in running"
    );
    assert_eq!(graph.snapshot().tick_count, 3);
}

// ── SimModel tests (moved from embedded.rs — require WasmSimPeripherals) ──

#[test]
fn adc_sim_reads_configured_voltage() {
    use blocks::embedded::{AdcBlock, AdcConfig};

    let mut block = AdcBlock::from_config(AdcConfig {
        channel: 2,
        resolution_bits: 12,
    });
    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_adc_voltage(2, 3.3);

    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[], 0.01, &mut peripherals);
    assert_eq!(out[0], Some(Value::Float(3.3)));
}

#[test]
fn pwm_sim_writes_duty() {
    use blocks::embedded::{PwmBlock, PwmConfig};

    let mut block = PwmBlock::from_config(PwmConfig {
        channel: 1,
        frequency_hz: 1000,
    });
    let mut peripherals = WasmSimPeripherals::new();
    let duty = Value::Float(0.75);

    let sim = block.as_sim_model().unwrap();
    sim.sim_tick(&[Some(&duty)], 0.01, &mut peripherals);
    assert_eq!(peripherals.get_pwm_duty(1), 0.75);
}

#[test]
fn gpio_sim_roundtrip() {
    use blocks::embedded::{GpioInBlock, GpioInConfig, GpioOutBlock, GpioOutConfig};

    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_gpio_state(5, true);

    let mut in_block = GpioInBlock::from_config(GpioInConfig { pin: 5 });
    let sim = in_block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[], 0.01, &mut peripherals);
    assert_eq!(out[0], Some(Value::Float(1.0)));

    let mut out_block = GpioOutBlock::from_config(GpioOutConfig { pin: 7 });
    let val = Value::Float(1.0);
    let sim = out_block.as_sim_model().unwrap();
    sim.sim_tick(&[Some(&val)], 0.01, &mut peripherals);
    assert!(peripherals.get_gpio_state(7));
}

#[test]
fn uart_tx_sim_tick() {
    use blocks::embedded::{UartTxBlock, UartTxConfig};

    let mut block = UartTxBlock::from_config(UartTxConfig::default());
    let mut peripherals = WasmSimPeripherals::new();
    let data = Value::Bytes(vec![0x41, 0x42]);

    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[Some(&data)], 0.01, &mut peripherals);
    assert!(out.is_empty());
}

#[test]
fn uart_rx_sim_tick() {
    use blocks::embedded::{UartRxBlock, UartRxConfig};

    let mut block = UartRxBlock::from_config(UartRxConfig::default());
    let mut peripherals = WasmSimPeripherals::new();

    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[], 0.01, &mut peripherals);
    assert_eq!(out.len(), 1);
    assert!(out[0].is_none());
}

#[test]
fn encoder_sim_tick() {
    use blocks::embedded::{EncoderBlock, EncoderConfig};

    let mut block = EncoderBlock::from_config(EncoderConfig::default());
    let mut peripherals = WasmSimPeripherals::new();

    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[], 0.01, &mut peripherals);
    assert_eq!(out.len(), 2);
    assert!(out[0].is_some());
    assert!(out[1].is_some());
}

#[test]
fn ssd1306_sim_tick() {
    use blocks::embedded::{Ssd1306DisplayBlock, Ssd1306DisplayConfig};

    let mut block = Ssd1306DisplayBlock::from_config(Ssd1306DisplayConfig::default());
    let mut peripherals = WasmSimPeripherals::new();

    let line1 = Value::Text("hello".into());
    let line2 = Value::Text("world".into());
    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[Some(&line1), Some(&line2)], 0.01, &mut peripherals);
    assert!(out.is_empty());
}

#[test]
fn tmc2209_stepper_sim_tick() {
    use blocks::embedded::{Tmc2209StepperBlock, Tmc2209StepperConfig};

    let mut block = Tmc2209StepperBlock::from_config(Tmc2209StepperConfig::default());
    let mut peripherals = WasmSimPeripherals::new();

    let target = Value::Float(100.0);
    let enable = Value::Float(1.0);
    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[Some(&target), Some(&enable)], 0.01, &mut peripherals);
    assert_eq!(out.len(), 1);
    assert!(out[0].is_some());
}

#[test]
fn tmc2209_stallguard_sim_tick() {
    use blocks::embedded::{Tmc2209StallGuardBlock, Tmc2209StallGuardConfig};

    let mut block = Tmc2209StallGuardBlock::from_config(Tmc2209StallGuardConfig::default());
    let mut peripherals = WasmSimPeripherals::new();

    let sim = block.as_sim_model().unwrap();
    let out = sim.sim_tick(&[], 0.01, &mut peripherals);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Some(Value::Float(0.0)));
    assert_eq!(out[1], Some(Value::Float(0.0)));
}

#[test]
fn sim_mode_graph_adc_to_gain_to_pwm() {
    use blocks::embedded::{AdcBlock, AdcConfig, PwmBlock, PwmConfig};
    use blocks::function::FunctionBlock;

    let mut g = DataflowGraph::new();
    g.set_simulation_mode(true);
    let mut peripherals = WasmSimPeripherals::new();
    peripherals.set_adc_voltage(0, 2.5);
    g.set_sim_peripherals(peripherals);

    let adc = g.add_block(Box::new(AdcBlock::from_config(AdcConfig::default())));
    let gain = g.add_block(Box::new(FunctionBlock::gain(0.4)));
    let pwm = g.add_block(Box::new(PwmBlock::from_config(PwmConfig::default())));

    g.connect(adc, 0, gain, 0).unwrap();
    g.connect(gain, 0, pwm, 0).unwrap();

    // Tick 1: ADC reads 2.5
    g.tick(0.01);
    // Tick 2: Gain receives 2.5, outputs 1.0
    g.tick(0.01);
    // Tick 3: PWM receives 1.0
    g.tick(0.01);

    assert_eq!(g.get_sim_pwm(0), 1.0);
}
