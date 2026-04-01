//! End-to-end pubsub test: build DAG with publish/subscribe ops,
//! evaluate, verify topic values flow correctly across ticks.

use dag_core::cbor;
use dag_core::eval::{NullChannels, PubSubReader};
use dag_core::op::Dag;
use std::collections::HashMap;

/// In-memory pubsub store — mirrors DagApiHandler's FnvIndexMap.
struct MapPubSub {
    topics: HashMap<String, f64>,
}

impl MapPubSub {
    fn new() -> Self {
        Self {
            topics: HashMap::new(),
        }
    }
}

impl PubSubReader for MapPubSub {
    fn read(&self, topic: &str) -> f64 {
        self.topics.get(topic).copied().unwrap_or(0.0)
    }
}

/// Simulate one tick: evaluate DAG, store published topics.
fn tick(dag: &Dag, values: &mut [f64], pubsub: &mut MapPubSub) {
    let result = dag.evaluate(&NullChannels, pubsub, values);
    for (topic, value) in &result.publishes {
        pubsub.topics.insert(topic.clone(), *value);
    }
}

#[test]
fn const_publish_roundtrip() {
    // DAG: Const(42.0) → Publish("sensor", 0)
    let mut dag = Dag::new();
    let c = dag.constant(42.0).unwrap();
    dag.publish("sensor", c).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    tick(&dag, &mut values, &mut pubsub);

    assert_eq!(pubsub.topics.get("sensor"), Some(&42.0));
}

#[test]
fn subscribe_reads_published_value() {
    // DAG: Const(10) → Publish("alpha", 0), Subscribe("alpha"), Publish("beta", 2)
    let mut dag = Dag::new();
    let c = dag.constant(10.0).unwrap();
    dag.publish("alpha", c).unwrap();
    let sub = dag.subscribe("alpha").unwrap();
    dag.publish("beta", sub).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    // Tick 1: publish alpha=10, subscribe reads alpha (0.0 — not yet stored)
    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["alpha"], 10.0);
    // beta gets the subscribe value which was 0.0 (read before write in same tick)
    assert_eq!(pubsub.topics["beta"], 0.0);

    // Tick 2: subscribe now reads alpha=10 (stored from tick 1)
    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["alpha"], 10.0);
    assert_eq!(pubsub.topics["beta"], 10.0);
}

#[test]
fn multi_topic_isolation() {
    // Two independent publish/subscribe pairs
    let mut dag = Dag::new();
    let a = dag.constant(100.0).unwrap();
    dag.publish("temp", a).unwrap();
    let b = dag.constant(0.5).unwrap();
    dag.publish("duty", b).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    tick(&dag, &mut values, &mut pubsub);

    assert_eq!(pubsub.topics["temp"], 100.0);
    assert_eq!(pubsub.topics["duty"], 0.5);
    assert_eq!(pubsub.topics.len(), 2);
}

#[test]
fn cbor_encode_deploy_tick() {
    // Simulate the full HTTP flow: encode DAG as CBOR, decode it, tick
    let mut dag = Dag::new();
    let c = dag.constant(7.5).unwrap();
    dag.publish("output", c).unwrap();

    // Encode → decode (simulates POST /api/dag)
    let cbor_bytes = cbor::encode_dag(&dag);
    let decoded = cbor::decode_dag(&cbor_bytes).expect("decode should succeed");

    assert_eq!(decoded.len(), dag.len());

    // Tick the decoded DAG
    let mut values = vec![0.0; decoded.len()];
    let mut pubsub = MapPubSub::new();
    tick(&decoded, &mut values, &mut pubsub);

    assert_eq!(pubsub.topics["output"], 7.5);
}

#[test]
fn pubsub_with_math_pipeline() {
    // DAG: Const(3) → node0, Const(4) → node1, Add(0,1) → node2, Publish("sum", 2)
    let mut dag = Dag::new();
    let a = dag.constant(3.0).unwrap();
    let b = dag.constant(4.0).unwrap();
    let sum = dag.add(a, b).unwrap();
    dag.publish("sum", sum).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["sum"], 7.0);
}

#[test]
fn subscribe_unset_topic_returns_zero() {
    // Subscribe to a topic that was never published
    let mut dag = Dag::new();
    let sub = dag.subscribe("nonexistent").unwrap();
    dag.publish("result", sub).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["result"], 0.0);
}

#[test]
fn pubsub_persists_across_ticks() {
    // Verify pubsub state persists: publish once, read on subsequent ticks
    let mut dag = Dag::new();
    let c = dag.constant(99.0).unwrap();
    dag.publish("persistent", c).unwrap();
    let sub = dag.subscribe("persistent").unwrap();
    dag.publish("echo", sub).unwrap();

    let mut values = vec![0.0; dag.len()];
    let mut pubsub = MapPubSub::new();

    // Tick 1
    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["persistent"], 99.0);
    assert_eq!(pubsub.topics["echo"], 0.0); // not yet available

    // Tick 2
    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["echo"], 99.0); // now reads persisted value

    // Tick 3 — still persists
    tick(&dag, &mut values, &mut pubsub);
    assert_eq!(pubsub.topics["echo"], 99.0);
}

#[test]
fn cbor_roundtrip_pubsub_ops() {
    let mut dag = Dag::new();
    let c = dag.constant(1.0).unwrap();
    dag.publish("topic_a", c).unwrap();
    let s = dag.subscribe("topic_b").unwrap();
    dag.publish("topic_c", s).unwrap();

    let bytes = cbor::encode_dag(&dag);
    let decoded = cbor::decode_dag(&bytes).unwrap();

    assert_eq!(dag.len(), decoded.len());
    assert_eq!(dag.nodes(), decoded.nodes());
}
