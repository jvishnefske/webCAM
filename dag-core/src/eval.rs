use alloc::string::String;
use alloc::vec::Vec;

use crate::op::{Dag, Op};

pub trait ChannelReader {
    fn read(&self, name: &str) -> f64;
}

pub trait ChannelWriter {
    fn write(&mut self, name: &str, value: f64);
}

pub trait PubSubReader {
    fn read(&self, topic: &str) -> f64;
}

pub trait PubSubWriter {
    fn write(&mut self, topic: &str, value: f64);
}

pub struct NullChannels;

impl ChannelReader for NullChannels {
    fn read(&self, _: &str) -> f64 {
        0.0
    }
}

impl ChannelWriter for NullChannels {
    fn write(&mut self, _: &str, _: f64) {}
}

pub struct NullPubSub;

impl PubSubReader for NullPubSub {
    fn read(&self, _: &str) -> f64 {
        0.0
    }
}

impl PubSubWriter for NullPubSub {
    fn write(&mut self, _: &str, _: f64) {}
}

pub struct EvalResult {
    pub outputs: Vec<(String, f64)>,
    pub publishes: Vec<(String, f64)>,
}

impl Dag {
    pub fn evaluate(
        &self,
        channels: &dyn ChannelReader,
        pubsub: &dyn PubSubReader,
        values: &mut [f64],
    ) -> EvalResult {
        let mut result = EvalResult {
            outputs: Vec::new(),
            publishes: Vec::new(),
        };

        for (i, op) in self.nodes().iter().enumerate() {
            values[i] = match op {
                Op::Const(v) => *v,
                Op::Input(name) => channels.read(name),
                Op::Output(name, src) => {
                    let v = values[*src as usize];
                    result.outputs.push((name.clone(), v));
                    v
                }
                Op::Add(a, b) => values[*a as usize] + values[*b as usize],
                Op::Mul(a, b) => values[*a as usize] * values[*b as usize],
                Op::Sub(a, b) => values[*a as usize] - values[*b as usize],
                Op::Div(a, b) => values[*a as usize] / values[*b as usize],
                Op::Pow(a, b) => libm::pow(values[*a as usize], values[*b as usize]),
                Op::Neg(a) => -values[*a as usize],
                Op::Relu(a) => {
                    let v = values[*a as usize];
                    if v > 0.0 {
                        v
                    } else {
                        0.0
                    }
                }
                Op::Subscribe(topic) => pubsub.read(topic),
                Op::Publish(topic, src) => {
                    let v = values[*src as usize];
                    result.publishes.push((topic.clone(), v));
                    v
                }
            };
        }

        result
    }
}

// ---------------------------------------------------------------------------
// SimState: persistent simulation state with pubsub feedback
// ---------------------------------------------------------------------------

/// Simulation state that persists pubsub values across ticks.
///
/// On each tick, `Publish` outputs are stored in the internal map.
/// On the next tick, `Subscribe` reads from this map, enabling
/// cross-tick feedback loops (e.g., accumulator, low-pass filter).
pub struct SimState {
    tick: u64,
    values: Vec<f64>,
    pubsub: alloc::collections::BTreeMap<String, f64>,
}

impl SimState {
    /// Create a new simulation state for a DAG with `node_count` nodes.
    pub fn new(node_count: usize) -> Self {
        Self {
            tick: 0,
            values: alloc::vec![0.0; node_count],
            pubsub: alloc::collections::BTreeMap::new(),
        }
    }

    /// Evaluate one tick of the DAG, storing pubsub outputs for the next tick.
    pub fn tick(&mut self, dag: &Dag) {
        let result = dag.evaluate(&NullChannels, &SimPubSub(&self.pubsub), &mut self.values);
        for (topic, value) in &result.publishes {
            self.pubsub.insert(topic.clone(), *value);
        }
        self.tick += 1;
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Get a pubsub topic's current value.
    pub fn pubsub_value(&self, topic: &str) -> Option<f64> {
        self.pubsub.get(topic).copied()
    }

    /// Get all pubsub topics and values.
    pub fn topics(&self) -> &alloc::collections::BTreeMap<String, f64> {
        &self.pubsub
    }

    /// Externally set (inject) a pubsub topic value.
    ///
    /// The value is immediately visible to `Subscribe` ops on the next
    /// `tick()` call, just as if a `Publish` op had written it.
    pub fn set_topic(&mut self, topic: &str, value: f64) {
        self.pubsub.insert(topic.into(), value);
    }

    /// Reset simulation state: clear tick counter, pubsub store, and values.
    pub fn reset(&mut self) {
        self.tick = 0;
        self.pubsub.clear();
        for v in &mut self.values {
            *v = 0.0;
        }
    }
}

/// PubSub reader backed by a BTreeMap (used internally by SimState).
struct SimPubSub<'a>(&'a alloc::collections::BTreeMap<String, f64>);

impl<'a> PubSubReader for SimPubSub<'a> {
    fn read(&self, topic: &str) -> f64 {
        self.0.get(topic).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Dag;
    use alloc::collections::BTreeMap;

    struct MockChannels {
        values: BTreeMap<String, f64>,
    }

    impl ChannelReader for MockChannels {
        fn read(&self, name: &str) -> f64 {
            self.values.get(name).copied().unwrap_or(0.0)
        }
    }

    struct MockPubSub {
        values: BTreeMap<String, f64>,
    }

    impl PubSubReader for MockPubSub {
        fn read(&self, topic: &str) -> f64 {
            self.values.get(topic).copied().unwrap_or(0.0)
        }
    }

    fn eval_simple(dag: &Dag) -> Vec<f64> {
        let mut values = alloc::vec![0.0; dag.len()];
        dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        values
    }

    #[test]
    fn test_null_channels_writer() {
        let mut nc = NullChannels;
        // NullChannels::write should be a no-op and not panic
        ChannelWriter::write(&mut nc, "anything", 42.0);
        assert_eq!(ChannelReader::read(&nc, "anything"), 0.0);
    }

    #[test]
    fn test_null_pubsub_writer() {
        let mut ps = NullPubSub;
        PubSubWriter::write(&mut ps, "topic", 99.0);
        assert_eq!(PubSubReader::read(&ps, "topic"), 0.0);
    }

    #[test]
    fn test_eval_const() {
        let mut dag = Dag::new();
        dag.constant(42.0).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[0], 42.0);
    }

    #[test]
    fn test_eval_add() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let c = dag.add(a, b).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[c as usize], 3.0);
    }

    #[test]
    fn test_eval_mul() {
        let mut dag = Dag::new();
        let a = dag.constant(3.0).unwrap();
        let b = dag.constant(4.0).unwrap();
        let c = dag.mul(a, b).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[c as usize], 12.0);
    }

    #[test]
    fn test_eval_sub() {
        let mut dag = Dag::new();
        let a = dag.constant(5.0).unwrap();
        let b = dag.constant(3.0).unwrap();
        let c = dag.sub(a, b).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[c as usize], 2.0);
    }

    #[test]
    fn test_eval_div() {
        let mut dag = Dag::new();
        let a = dag.constant(10.0).unwrap();
        let b = dag.constant(4.0).unwrap();
        let c = dag.div(a, b).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[c as usize], 2.5);
    }

    #[test]
    fn test_eval_pow() {
        let mut dag = Dag::new();
        let a = dag.constant(2.0).unwrap();
        let b = dag.constant(3.0).unwrap();
        let c = dag.pow(a, b).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[c as usize], 8.0);
    }

    #[test]
    fn test_eval_neg() {
        let mut dag = Dag::new();
        let a = dag.constant(5.0).unwrap();
        let n = dag.neg(a).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[n as usize], -5.0);
    }

    #[test]
    fn test_eval_relu_positive() {
        let mut dag = Dag::new();
        let a = dag.constant(3.0).unwrap();
        let r = dag.relu(a).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[r as usize], 3.0);
    }

    #[test]
    fn test_eval_relu_negative() {
        let mut dag = Dag::new();
        let a = dag.constant(-3.0).unwrap();
        let r = dag.relu(a).unwrap();
        let values = eval_simple(&dag);
        assert_eq!(values[r as usize], 0.0);
    }

    #[test]
    fn test_eval_input() {
        let mut dag = Dag::new();
        dag.input("adc0").unwrap();

        let mut channels = MockChannels {
            values: BTreeMap::new(),
        };
        channels.values.insert("adc0".into(), 3.25);

        let mut values = alloc::vec![0.0; dag.len()];
        dag.evaluate(&channels, &NullPubSub, &mut values);
        assert_eq!(values[0], 3.25);
    }

    #[test]
    fn test_eval_output() {
        let mut dag = Dag::new();
        let a = dag.constant(7.5).unwrap();
        dag.output("pwm0", a).unwrap();

        let mut values = alloc::vec![0.0; dag.len()];
        let result = dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].0, "pwm0");
        assert_eq!(result.outputs[0].1, 7.5);
    }

    #[test]
    fn test_eval_subscribe_publish() {
        let mut dag = Dag::new();
        let sub = dag.subscribe("sensor/temp").unwrap();
        dag.publish("actuator/fan", sub).unwrap();

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("sensor/temp".into(), 25.0);

        let mut values = alloc::vec![0.0; dag.len()];
        let result = dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(values[0], 25.0);
        assert_eq!(result.publishes.len(), 1);
        assert_eq!(result.publishes[0].0, "actuator/fan");
        assert_eq!(result.publishes[0].1, 25.0);
    }

    #[test]
    fn test_eval_micrograd_example() {
        let mut dag = Dag::new();
        let a = dag.constant(-4.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let c0 = dag.add(a, b).unwrap(); // c = a + b
        let ab = dag.mul(a, b).unwrap(); // a * b
        let three = dag.constant(3.0).unwrap();
        let b3 = dag.pow(b, three).unwrap(); // b**3
        let d0 = dag.add(ab, b3).unwrap(); // d = a*b + b**3
        let one = dag.constant(1.0).unwrap();
        let c1 = dag.add(c0, one).unwrap(); // c + 1
        let c1 = dag.add(c0, c1).unwrap(); // c += c + 1  =>  c = c + (c + 1)
        let neg_a = dag.neg(a).unwrap(); // -a
        let c2 = dag.add(one, c1).unwrap(); // 1 + c
        let c2 = dag.add(c2, neg_a).unwrap(); // 1 + c + (-a)
        let c2 = dag.add(c1, c2).unwrap(); // c += 1 + c + (-a)
        let two = dag.constant(2.0).unwrap();
        let d1 = dag.mul(d0, two).unwrap(); // d * 2
        let ba = dag.add(b, a).unwrap(); // b + a
        let ba_relu = dag.relu(ba).unwrap(); // (b+a).relu()
        let d1 = dag.add(d1, ba_relu).unwrap(); // d*2 + (b+a).relu()
        let d1 = dag.add(d0, d1).unwrap(); // d += d*2 + (b+a).relu()
        let d2 = dag.mul(three, d1).unwrap(); // 3 * d
        let bsa = dag.sub(b, a).unwrap(); // b - a
        let bsa_relu = dag.relu(bsa).unwrap(); // (b-a).relu()
        let d2 = dag.add(d2, bsa_relu).unwrap(); // 3*d + (b-a).relu()
        let d2 = dag.add(d1, d2).unwrap(); // d += 3*d + (b-a).relu()
        let e = dag.sub(c2, d2).unwrap(); // e = c - d
        let f = dag.pow(e, two).unwrap(); // f = e**2
        let half = dag.constant(0.5).unwrap();
        let g0 = dag.mul(f, half).unwrap(); // f / 2.0  (as f * 0.5)
        let ten = dag.constant(10.0).unwrap();
        let g1 = dag.div(ten, f).unwrap(); // 10.0 / f
        let g = dag.add(g0, g1).unwrap(); // g = f/2 + 10/f

        let values = eval_simple(&dag);
        let result = values[g as usize];

        assert!(
            (result - 24.7041).abs() < 1e-4,
            "Expected ~24.7041, got {}",
            result
        );
    }

    // =================================================================
    // SimState tests (task-001)
    // =================================================================

    #[test]
    fn test_req_001_simstate_tick_increments() {
        let mut dag = Dag::new();
        dag.constant(1.0).unwrap();
        let mut state = SimState::new(dag.len());
        assert_eq!(state.tick_count(), 0);
        state.tick(&dag);
        assert_eq!(state.tick_count(), 1);
        state.tick(&dag);
        assert_eq!(state.tick_count(), 2);
    }

    #[test]
    fn test_req_001_simstate_pubsub_persistence() {
        // Const(10) → Publish("x"), Subscribe("x") → Publish("y")
        let mut dag = Dag::new();
        let c = dag.constant(10.0).unwrap();
        dag.publish("x", c).unwrap();
        let s = dag.subscribe("x").unwrap();
        dag.publish("y", s).unwrap();

        let mut state = SimState::new(dag.len());

        // Tick 1: publish x=10, subscribe reads x=0 (not yet stored)
        state.tick(&dag);
        assert_eq!(state.pubsub_value("x"), Some(10.0));
        assert_eq!(state.pubsub_value("y"), Some(0.0));

        // Tick 2: subscribe reads x=10 (from tick 1), publishes y=10
        state.tick(&dag);
        assert_eq!(state.pubsub_value("y"), Some(10.0));
    }

    #[test]
    fn test_req_001_simstate_reset() {
        let mut dag = Dag::new();
        let c = dag.constant(42.0).unwrap();
        dag.publish("val", c).unwrap();

        let mut state = SimState::new(dag.len());
        state.tick(&dag);
        assert_eq!(state.tick_count(), 1);
        assert_eq!(state.pubsub_value("val"), Some(42.0));

        state.reset();
        assert_eq!(state.tick_count(), 0);
        assert_eq!(state.pubsub_value("val"), None);
    }

    #[test]
    fn test_req_001_simstate_multi_tick_accumulation() {
        // Subscribe("acc") → Add with Const(1) → Publish("acc")
        // Each tick: acc = acc + 1 (accumulator via pubsub feedback)
        let mut dag = Dag::new();
        let prev = dag.subscribe("acc").unwrap();
        let one = dag.constant(1.0).unwrap();
        let sum = dag.add(prev, one).unwrap();
        dag.publish("acc", sum).unwrap();

        let mut state = SimState::new(dag.len());

        state.tick(&dag); // acc = 0 + 1 = 1
        assert_eq!(state.pubsub_value("acc"), Some(1.0));

        state.tick(&dag); // acc = 1 + 1 = 2
        assert_eq!(state.pubsub_value("acc"), Some(2.0));

        state.tick(&dag); // acc = 2 + 1 = 3
        assert_eq!(state.pubsub_value("acc"), Some(3.0));
    }

    #[test]
    fn test_req_001_simstate_topics_list() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        dag.publish("alpha", a).unwrap();
        dag.publish("beta", b).unwrap();

        let mut state = SimState::new(dag.len());
        state.tick(&dag);

        let topics = state.topics();
        assert_eq!(topics.len(), 2);
        assert!(topics.contains_key("alpha"));
        assert!(topics.contains_key("beta"));
    }
}
