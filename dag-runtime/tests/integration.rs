//! Integration tests exercising the dag-core builder + CBOR encoding
//! round-tripped through the dag-runtime executor.

use dag_core::cbor;
use dag_core::op::Dag;
use dag_runtime::channels::MapChannels;
use dag_runtime::executor::DagExecutor;
use dag_runtime::pubsub::SimplePubSub;

/// Build constant(5.0) -> output("result"), encode to CBOR, decode in executor,
/// tick once, and verify the output is 5.0.
#[test]
fn test_builder_cbor_executor_roundtrip() {
    let mut dag = Dag::new();
    let c = dag.constant(5.0).unwrap();
    dag.output("result", c).unwrap();

    let bytes = cbor::encode_dag(&dag);

    let mut exec = DagExecutor::new();
    exec.load_cbor(&bytes).unwrap();

    let channels = MapChannels::new();
    let mut ch_writer = MapChannels::new();
    let pubsub = SimplePubSub::new();
    let mut ps_writer = SimplePubSub::new();

    let result = exec
        .tick(&channels, &mut ch_writer, &pubsub, &mut ps_writer)
        .expect("tick should return Some when a DAG is loaded");

    assert_eq!(result.outputs.len(), 1);
    assert_eq!(result.outputs[0].0, "result");
    assert_eq!(result.outputs[0].1, 5.0);

    // The channel writer should also have the output value
    assert_eq!(ch_writer.get("result"), 5.0);
}

/// Build subscribe("sensor/temp") -> mul(_, constant(2.0)) -> publish("actuator/fan").
/// Set pubsub "sensor/temp" = 25.0, tick, verify "actuator/fan" == 50.0.
#[test]
fn test_pubsub_propagation() {
    let mut dag = Dag::new();
    let sub = dag.subscribe("sensor/temp").unwrap();
    let gain = dag.constant(2.0).unwrap();
    let product = dag.mul(sub, gain).unwrap();
    dag.publish("actuator/fan", product).unwrap();

    let bytes = cbor::encode_dag(&dag);

    let mut exec = DagExecutor::new();
    exec.load_cbor(&bytes).unwrap();

    let channels = MapChannels::new();
    let mut ch_writer = MapChannels::new();
    let mut ps_reader = SimplePubSub::new();
    ps_reader.set("sensor/temp", 25.0);
    let mut ps_writer = SimplePubSub::new();

    let result = exec
        .tick(&channels, &mut ch_writer, &ps_reader, &mut ps_writer)
        .expect("tick should return Some when a DAG is loaded");

    assert_eq!(result.publishes.len(), 1);
    assert_eq!(result.publishes[0].0, "actuator/fan");
    assert_eq!(result.publishes[0].1, 50.0);

    // The pubsub writer should also have the published value
    assert_eq!(ps_writer.get("actuator/fan"), 50.0);
}

/// Load one DAG (constant 1.0 -> output "x"), tick and verify x=1.0.
/// Then load a different DAG (constant 99.0 -> output "y"), tick and verify
/// y=99.0 and tick_count resets to 1 after the single new tick.
#[test]
fn test_executor_reload_replaces_graph() {
    let mut exec = DagExecutor::new();

    // --- First DAG: constant(1.0) -> output("x") ---
    let mut dag1 = Dag::new();
    let c1 = dag1.constant(1.0).unwrap();
    dag1.output("x", c1).unwrap();

    exec.load_dag(dag1);

    let channels = MapChannels::new();
    let mut ch_writer = MapChannels::new();
    let pubsub = SimplePubSub::new();
    let mut ps_writer = SimplePubSub::new();

    let result1 = exec
        .tick(&channels, &mut ch_writer, &pubsub, &mut ps_writer)
        .unwrap();

    assert_eq!(result1.outputs.len(), 1);
    assert_eq!(result1.outputs[0].0, "x");
    assert_eq!(result1.outputs[0].1, 1.0);
    assert_eq!(exec.tick_count(), 1);

    // --- Second DAG: constant(99.0) -> output("y") ---
    let mut dag2 = Dag::new();
    let c2 = dag2.constant(99.0).unwrap();
    dag2.output("y", c2).unwrap();

    exec.load_dag(dag2);
    // tick_count should have been reset to 0 by load_dag
    assert_eq!(exec.tick_count(), 0);

    let result2 = exec
        .tick(&channels, &mut ch_writer, &pubsub, &mut ps_writer)
        .unwrap();

    assert_eq!(result2.outputs.len(), 1);
    assert_eq!(result2.outputs[0].0, "y");
    assert_eq!(result2.outputs[0].1, 99.0);
    assert_eq!(exec.tick_count(), 1);

    // The old output "x" should not appear in the new result
    assert!(
        result2.outputs.iter().all(|(name, _)| name != "x"),
        "old DAG output 'x' must not appear after reload"
    );
}

/// Build input("adc0") -> output("result"), set channel "adc0" = 3.25,
/// tick, verify output "result" == 3.25.
#[test]
fn test_input_channel_to_output() {
    let mut dag = Dag::new();
    let inp = dag.input("adc0").unwrap();
    dag.output("result", inp).unwrap();

    let bytes = cbor::encode_dag(&dag);

    let mut exec = DagExecutor::new();
    exec.load_cbor(&bytes).unwrap();

    let mut channels = MapChannels::new();
    channels.set("adc0", 3.25);
    let mut ch_writer = MapChannels::new();
    let pubsub = SimplePubSub::new();
    let mut ps_writer = SimplePubSub::new();

    let result = exec
        .tick(&channels, &mut ch_writer, &pubsub, &mut ps_writer)
        .expect("tick should return Some when a DAG is loaded");

    assert_eq!(result.outputs.len(), 1);
    assert_eq!(result.outputs[0].0, "result");
    assert_eq!(result.outputs[0].1, 3.25);

    // Verify the value was also written to the channel writer
    assert_eq!(ch_writer.get("result"), 3.25);
}
