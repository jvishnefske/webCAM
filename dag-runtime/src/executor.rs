use alloc::vec;
use alloc::vec::Vec;

use dag_core::cbor::DecodeError;
use dag_core::eval::{ChannelReader, ChannelWriter, EvalResult, PubSubReader, PubSubWriter};
use dag_core::op::Dag;

/// Owns a DAG and its evaluation buffer, providing a tick-based execution loop.
pub struct DagExecutor {
    dag: Option<Dag>,
    values: Vec<f64>,
    tick_count: u64,
}

impl DagExecutor {
    /// Create a new executor with no DAG loaded.
    pub fn new() -> Self {
        DagExecutor {
            dag: None,
            values: Vec::new(),
            tick_count: 0,
        }
    }

    /// Load a new DAG from CBOR bytes. Returns error if decode fails.
    pub fn load_cbor(&mut self, bytes: &[u8]) -> Result<(), DecodeError> {
        let dag = dag_core::cbor::decode_dag(bytes)?;
        self.values = vec![0.0; dag.len()];
        self.dag = Some(dag);
        self.tick_count = 0;
        Ok(())
    }

    /// Load a pre-built DAG directly.
    pub fn load_dag(&mut self, dag: Dag) {
        self.values = vec![0.0; dag.len()];
        self.dag = Some(dag);
        self.tick_count = 0;
    }

    /// Execute one tick of the DAG. Returns `None` if no DAG is loaded.
    pub fn tick(
        &mut self,
        channels: &dyn ChannelReader,
        channel_writer: &mut dyn ChannelWriter,
        pubsub_reader: &dyn PubSubReader,
        pubsub_writer: &mut dyn PubSubWriter,
    ) -> Option<EvalResult> {
        let dag = self.dag.as_ref()?;
        let result = dag.evaluate(channels, pubsub_reader, &mut self.values);
        for (name, value) in &result.outputs {
            channel_writer.write(name, *value);
        }
        for (topic, value) in &result.publishes {
            pubsub_writer.write(topic, *value);
        }
        self.tick_count += 1;
        Some(result)
    }

    /// Number of ticks executed since last load.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Whether a DAG is currently loaded.
    pub fn is_loaded(&self) -> bool {
        self.dag.is_some()
    }

    /// Number of nodes in the loaded DAG, or 0 if none.
    pub fn node_count(&self) -> usize {
        self.dag.as_ref().map_or(0, |d| d.len())
    }

    /// Current evaluation buffer.
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

impl Default for DagExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::MapChannels;
    use crate::pubsub::SimplePubSub;
    use dag_core::cbor::encode_dag;
    use dag_core::eval::{NullChannels, NullPubSub};

    #[test]
    fn test_executor_new() {
        let exec = DagExecutor::new();
        assert!(!exec.is_loaded());
        assert_eq!(exec.tick_count(), 0);
        assert_eq!(exec.node_count(), 0);
        assert!(exec.values().is_empty());
    }

    #[test]
    fn test_executor_load_dag() {
        let mut exec = DagExecutor::new();
        let mut dag = Dag::new();
        dag.constant(1.0).unwrap();
        dag.constant(2.0).unwrap();
        dag.add(0, 1).unwrap();
        exec.load_dag(dag);
        assert!(exec.is_loaded());
        assert_eq!(exec.node_count(), 3);
        assert_eq!(exec.values().len(), 3);
    }

    #[test]
    fn test_executor_load_cbor() {
        let mut dag = Dag::new();
        dag.constant(1.0).unwrap();
        dag.constant(2.0).unwrap();
        dag.add(0, 1).unwrap();
        let bytes = encode_dag(&dag);

        let mut exec = DagExecutor::new();
        exec.load_cbor(&bytes).unwrap();
        assert!(exec.is_loaded());
        assert_eq!(exec.node_count(), 3);
    }

    #[test]
    fn test_executor_tick() {
        let mut exec = DagExecutor::new();
        let mut dag = Dag::new();
        let inp = dag.input("adc0").unwrap();
        let gain = dag.constant(2.0).unwrap();
        let product = dag.mul(inp, gain).unwrap();
        dag.output("pwm0", product).unwrap();
        exec.load_dag(dag);

        let mut reader = MapChannels::new();
        reader.set("adc0", 3.0);
        let mut writer = MapChannels::new();
        let pubsub = SimplePubSub::new();
        let mut pubsub_w = SimplePubSub::new();

        let result = exec
            .tick(&reader, &mut writer, &pubsub, &mut pubsub_w)
            .unwrap();
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].0, "pwm0");
        assert_eq!(result.outputs[0].1, 6.0);
        // Verify channel was written
        assert_eq!(writer.get("pwm0"), 6.0);
    }

    #[test]
    fn test_executor_tick_pubsub() {
        let mut exec = DagExecutor::new();
        let mut dag = Dag::new();
        let sub = dag.subscribe("sensor/temp").unwrap();
        dag.publish("actuator/fan", sub).unwrap();
        exec.load_dag(dag);

        let channels = MapChannels::new();
        let mut ch_writer = MapChannels::new();
        // Use the writer (which has the value) as reader too — need separate objects
        let mut ps_reader = SimplePubSub::new();
        ps_reader.set("sensor/temp", 25.0);
        let mut ps_writer = SimplePubSub::new();

        let result = exec
            .tick(&channels, &mut ch_writer, &ps_reader, &mut ps_writer)
            .unwrap();
        assert_eq!(result.publishes.len(), 1);
        assert_eq!(result.publishes[0].0, "actuator/fan");
        assert_eq!(result.publishes[0].1, 25.0);
        // Verify pubsub was written
        assert_eq!(ps_writer.get("actuator/fan"), 25.0);
    }

    #[test]
    fn test_executor_tick_count() {
        let mut exec = DagExecutor::new();
        let mut dag = Dag::new();
        dag.constant(1.0).unwrap();
        exec.load_dag(dag);

        let channels = MapChannels::new();
        let mut ch_writer = MapChannels::new();
        let pubsub = SimplePubSub::new();
        let mut pubsub_w = SimplePubSub::new();

        for _ in 0..5 {
            exec.tick(&channels, &mut ch_writer, &pubsub, &mut pubsub_w);
        }
        assert_eq!(exec.tick_count(), 5);
    }

    #[test]
    fn test_executor_reload() {
        let mut exec = DagExecutor::new();

        // Load first dag
        let mut dag1 = Dag::new();
        dag1.constant(1.0).unwrap();
        dag1.constant(2.0).unwrap();
        exec.load_dag(dag1);

        let channels = MapChannels::new();
        let mut ch_writer = MapChannels::new();
        let pubsub = SimplePubSub::new();
        let mut pubsub_w = SimplePubSub::new();
        exec.tick(&channels, &mut ch_writer, &pubsub, &mut pubsub_w);
        exec.tick(&channels, &mut ch_writer, &pubsub, &mut pubsub_w);
        assert_eq!(exec.tick_count(), 2);
        assert_eq!(exec.node_count(), 2);

        // Load second dag — should reset
        let mut dag2 = Dag::new();
        dag2.constant(42.0).unwrap();
        exec.load_dag(dag2);
        assert_eq!(exec.tick_count(), 0);
        assert_eq!(exec.node_count(), 1);
        assert_eq!(exec.values(), &[0.0]);
    }

    #[test]
    fn test_executor_micrograd() {
        let mut exec = DagExecutor::new();
        let mut dag = Dag::new();

        let a = dag.constant(-4.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let c0 = dag.add(a, b).unwrap();
        let ab = dag.mul(a, b).unwrap();
        let three = dag.constant(3.0).unwrap();
        let b3 = dag.pow(b, three).unwrap();
        let d0 = dag.add(ab, b3).unwrap();
        let one = dag.constant(1.0).unwrap();
        let c1 = dag.add(c0, one).unwrap();
        let c1 = dag.add(c0, c1).unwrap();
        let neg_a = dag.neg(a).unwrap();
        let c2 = dag.add(one, c1).unwrap();
        let c2 = dag.add(c2, neg_a).unwrap();
        let c2 = dag.add(c1, c2).unwrap();
        let two = dag.constant(2.0).unwrap();
        let d1 = dag.mul(d0, two).unwrap();
        let ba = dag.add(b, a).unwrap();
        let ba_relu = dag.relu(ba).unwrap();
        let d1 = dag.add(d1, ba_relu).unwrap();
        let d1 = dag.add(d0, d1).unwrap();
        let d2 = dag.mul(three, d1).unwrap();
        let bsa = dag.sub(b, a).unwrap();
        let bsa_relu = dag.relu(bsa).unwrap();
        let d2 = dag.add(d2, bsa_relu).unwrap();
        let d2 = dag.add(d1, d2).unwrap();
        let e = dag.sub(c2, d2).unwrap();
        let f = dag.pow(e, two).unwrap();
        let half = dag.constant(0.5).unwrap();
        let g0 = dag.mul(f, half).unwrap();
        let ten = dag.constant(10.0).unwrap();
        let g1 = dag.div(ten, f).unwrap();
        let g = dag.add(g0, g1).unwrap();

        exec.load_dag(dag);

        let null_ch = NullChannels;
        let mut null_ch_w = NullChannels;
        let null_ps = NullPubSub;
        let mut null_ps_w = NullPubSub;
        exec.tick(&null_ch, &mut null_ch_w, &null_ps, &mut null_ps_w);

        let result = exec.values()[g as usize];
        assert!(
            (result - 24.7041).abs() < 1e-4,
            "Expected ~24.7041, got {}",
            result
        );
    }
}
