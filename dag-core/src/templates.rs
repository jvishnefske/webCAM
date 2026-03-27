use alloc::string::String;
use alloc::vec::Vec;

use crate::op::{Dag, DagError, NodeId};

/// Result of expanding a block template into a DAG subgraph.
pub struct BlockPorts {
    /// Named input nodes (each is the NodeId where external input connects).
    pub inputs: Vec<(String, NodeId)>,
    /// Named output nodes.
    pub outputs: Vec<(String, NodeId)>,
}

/// Constant: no inputs, one output.
pub fn constant_template(dag: &mut Dag, value: f64) -> Result<BlockPorts, DagError> {
    let out = dag.constant(value)?;
    Ok(BlockPorts {
        inputs: Vec::new(),
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// Gain: one input, one output (input * factor).
pub fn gain_template(dag: &mut Dag, input: NodeId, factor: f64) -> Result<BlockPorts, DagError> {
    let k = dag.constant(factor)?;
    let out = dag.mul(input, k)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("in".into(), input)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// Add: two inputs, one output.
pub fn add_template(dag: &mut Dag, a: NodeId, b: NodeId) -> Result<BlockPorts, DagError> {
    let out = dag.add(a, b)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("a".into(), a), ("b".into(), b)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// Multiply: two inputs, one output.
pub fn multiply_template(dag: &mut Dag, a: NodeId, b: NodeId) -> Result<BlockPorts, DagError> {
    let out = dag.mul(a, b)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("a".into(), a), ("b".into(), b)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// Clamp: one input, one output (clamp between min and max).
///
/// Uses the identity: clamp(x, min, max) = relu(x - min) + min - relu(relu(x - min) + min - max)
pub fn clamp_template(
    dag: &mut Dag,
    input: NodeId,
    min: f64,
    max: f64,
) -> Result<BlockPorts, DagError> {
    let min_node = dag.constant(min)?;
    let max_node = dag.constant(max)?;
    // x - min
    let shifted_low = dag.sub(input, min_node)?;
    // relu(x - min) — clamps negative to 0
    let clamped_low = dag.relu(shifted_low)?;
    // relu(x - min) + min — result >= min
    let above_min = dag.add(clamped_low, min_node)?;
    // above_min - max
    let shifted_high = dag.sub(above_min, max_node)?;
    // relu(above_min - max) — excess above max
    let excess = dag.relu(shifted_high)?;
    // above_min - excess — clamp to <= max
    let out = dag.sub(above_min, excess)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("in".into(), input)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// ADC Source: named input channel.
pub fn adc_source_template(dag: &mut Dag, channel_name: &str) -> Result<BlockPorts, DagError> {
    let out = dag.input(channel_name)?;
    Ok(BlockPorts {
        inputs: Vec::new(),
        outputs: alloc::vec![("value".into(), out)],
    })
}

/// PWM Sink: named output channel.
pub fn pwm_sink_template(
    dag: &mut Dag,
    channel_name: &str,
    input: NodeId,
) -> Result<BlockPorts, DagError> {
    let out = dag.output(channel_name, input)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("duty".into(), input)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

/// PubSub Source: subscribe to a topic.
pub fn pubsub_source_template(dag: &mut Dag, topic: &str) -> Result<BlockPorts, DagError> {
    let out = dag.subscribe(topic)?;
    Ok(BlockPorts {
        inputs: Vec::new(),
        outputs: alloc::vec![("value".into(), out)],
    })
}

/// PubSub Sink: publish to a topic.
pub fn pubsub_sink_template(
    dag: &mut Dag,
    topic: &str,
    input: NodeId,
) -> Result<BlockPorts, DagError> {
    let out = dag.publish(topic, input)?;
    Ok(BlockPorts {
        inputs: alloc::vec![("value".into(), input)],
        outputs: alloc::vec![("out".into(), out)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use crate::eval::{ChannelReader, NullChannels, NullPubSub, PubSubReader};
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
    fn test_constant_template() {
        let mut dag = Dag::new();
        let ports = constant_template(&mut dag, 42.0).unwrap();

        assert!(ports.inputs.is_empty());
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "out");

        let values = eval_simple(&dag);
        assert_eq!(values[ports.outputs[0].1 as usize], 42.0);
    }

    #[test]
    fn test_gain_template() {
        let mut dag = Dag::new();
        let input = dag.constant(3.0).unwrap();
        let ports = gain_template(&mut dag, input, 2.5).unwrap();

        assert_eq!(ports.inputs.len(), 1);
        assert_eq!(ports.inputs[0].0, "in");
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "out");

        let values = eval_simple(&dag);
        assert_eq!(values[ports.outputs[0].1 as usize], 7.5);
    }

    #[test]
    fn test_add_template() {
        let mut dag = Dag::new();
        let a = dag.constant(10.0).unwrap();
        let b = dag.constant(20.0).unwrap();
        let ports = add_template(&mut dag, a, b).unwrap();

        assert_eq!(ports.inputs.len(), 2);
        assert_eq!(ports.inputs[0].0, "a");
        assert_eq!(ports.inputs[1].0, "b");
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "out");

        let values = eval_simple(&dag);
        assert_eq!(values[ports.outputs[0].1 as usize], 30.0);
    }

    #[test]
    fn test_multiply_template() {
        let mut dag = Dag::new();
        let a = dag.constant(6.0).unwrap();
        let b = dag.constant(7.0).unwrap();
        let ports = multiply_template(&mut dag, a, b).unwrap();

        assert_eq!(ports.inputs.len(), 2);
        assert_eq!(ports.inputs[0].0, "a");
        assert_eq!(ports.inputs[1].0, "b");
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "out");

        let values = eval_simple(&dag);
        assert_eq!(values[ports.outputs[0].1 as usize], 42.0);
    }

    #[test]
    fn test_clamp_template() {
        // Test clamping above max: clamp(7.0, 2.0, 5.0) = 5.0
        {
            let mut dag = Dag::new();
            let input = dag.constant(7.0).unwrap();
            let ports = clamp_template(&mut dag, input, 2.0, 5.0).unwrap();
            let values = eval_simple(&dag);
            assert_eq!(values[ports.outputs[0].1 as usize], 5.0);
        }

        // Test clamping below min: clamp(-1.0, 2.0, 5.0) = 2.0
        {
            let mut dag = Dag::new();
            let input = dag.constant(-1.0).unwrap();
            let ports = clamp_template(&mut dag, input, 2.0, 5.0).unwrap();
            let values = eval_simple(&dag);
            assert_eq!(values[ports.outputs[0].1 as usize], 2.0);
        }

        // Test value within range: clamp(3.0, 2.0, 5.0) = 3.0
        {
            let mut dag = Dag::new();
            let input = dag.constant(3.0).unwrap();
            let ports = clamp_template(&mut dag, input, 2.0, 5.0).unwrap();
            let values = eval_simple(&dag);
            assert_eq!(values[ports.outputs[0].1 as usize], 3.0);
        }
    }

    #[test]
    fn test_adc_source_template() {
        let mut dag = Dag::new();
        let ports = adc_source_template(&mut dag, "adc0").unwrap();

        assert!(ports.inputs.is_empty());
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "value");

        // Verify the node is an Input with the correct channel name
        use crate::op::Op;
        assert_eq!(dag.nodes()[ports.outputs[0].1 as usize], Op::Input("adc0".into()));

        // Evaluate with mock channels
        let mut channels = MockChannels {
            values: BTreeMap::new(),
        };
        channels.values.insert("adc0".into(), 3.25);
        let mut values = alloc::vec![0.0; dag.len()];
        dag.evaluate(&channels, &NullPubSub, &mut values);
        assert_eq!(values[ports.outputs[0].1 as usize], 3.25);
    }

    #[test]
    fn test_pwm_sink_template() {
        let mut dag = Dag::new();
        let input = dag.constant(0.75).unwrap();
        let ports = pwm_sink_template(&mut dag, "pwm0", input).unwrap();

        assert_eq!(ports.inputs.len(), 1);
        assert_eq!(ports.inputs[0].0, "duty");
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.outputs[0].0, "out");

        let mut values = alloc::vec![0.0; dag.len()];
        let result = dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].0, "pwm0");
        assert_eq!(result.outputs[0].1, 0.75);
    }

    #[test]
    fn test_pubsub_templates() {
        let mut dag = Dag::new();
        let source_ports = pubsub_source_template(&mut dag, "sensor/temp").unwrap();
        let source_out = source_ports.outputs[0].1;
        let sink_ports = pubsub_sink_template(&mut dag, "actuator/fan", source_out).unwrap();

        assert!(source_ports.inputs.is_empty());
        assert_eq!(source_ports.outputs[0].0, "value");
        assert_eq!(sink_ports.inputs[0].0, "value");
        assert_eq!(sink_ports.outputs[0].0, "out");

        let mut pubsub = MockPubSub {
            values: BTreeMap::new(),
        };
        pubsub.values.insert("sensor/temp".into(), 25.0);

        let mut values = alloc::vec![0.0; dag.len()];
        let result = dag.evaluate(&NullChannels, &pubsub, &mut values);
        assert_eq!(values[source_out as usize], 25.0);
        assert_eq!(result.publishes.len(), 1);
        assert_eq!(result.publishes[0].0, "actuator/fan");
        assert_eq!(result.publishes[0].1, 25.0);
    }

    #[test]
    fn test_composite_template() {
        // Chain: adc_source("adc0") -> gain(2.5) -> clamp(0, 100) -> pwm_sink("pwm0")
        let mut dag = Dag::new();

        let adc = adc_source_template(&mut dag, "adc0").unwrap();
        let adc_out = adc.outputs[0].1;

        let gain = gain_template(&mut dag, adc_out, 2.5).unwrap();
        let gain_out = gain.outputs[0].1;

        let clamp = clamp_template(&mut dag, gain_out, 0.0, 100.0).unwrap();
        let clamp_out = clamp.outputs[0].1;

        let _pwm = pwm_sink_template(&mut dag, "pwm0", clamp_out).unwrap();

        // Test with input = 30.0 -> gain = 75.0 -> clamp = 75.0
        {
            let mut channels = MockChannels {
                values: BTreeMap::new(),
            };
            channels.values.insert("adc0".into(), 30.0);
            let mut values = alloc::vec![0.0; dag.len()];
            let result = dag.evaluate(&channels, &NullPubSub, &mut values);
            assert_eq!(result.outputs.len(), 1);
            assert_eq!(result.outputs[0].0, "pwm0");
            assert_eq!(result.outputs[0].1, 75.0);
        }

        // Test with input = 50.0 -> gain = 125.0 -> clamp = 100.0
        {
            let mut channels = MockChannels {
                values: BTreeMap::new(),
            };
            channels.values.insert("adc0".into(), 50.0);
            let mut values = alloc::vec![0.0; dag.len()];
            let result = dag.evaluate(&channels, &NullPubSub, &mut values);
            assert_eq!(result.outputs[0].1, 100.0);
        }

        // Test with input = -5.0 -> gain = -12.5 -> clamp = 0.0
        {
            let mut channels = MockChannels {
                values: BTreeMap::new(),
            };
            channels.values.insert("adc0".into(), -5.0);
            let mut values = alloc::vec![0.0; dag.len()];
            let result = dag.evaluate(&channels, &NullPubSub, &mut values);
            assert_eq!(result.outputs[0].1, 0.0);
        }
    }
}
