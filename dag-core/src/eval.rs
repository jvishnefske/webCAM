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
                    if v > 0.0 { v } else { 0.0 }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use crate::op::Dag;

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
}
