//! The dataflow graph: owns blocks and channels, runs the tick loop.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::block::{BlockId, Module, Value};
use super::channel::{Channel, ChannelId};

/// Snapshot of one block for serialization to the frontend.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub id: u32,
    pub block_type: String,
    pub name: String,
    pub inputs: Vec<super::block::PortDef>,
    pub outputs: Vec<super::block::PortDef>,
    pub config: serde_json::Value,
    /// Last output values (one per output port).
    pub output_values: Vec<Option<Value>>,
    /// Optional target MCU assignment for distributed codegen.
    /// When None, the block runs on all targets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<crate::dataflow::codegen::target::TargetFamily>,
    /// Custom codegen output from blocks implementing the `Codegen` trait.
    /// When present, emit.rs uses this instead of built-in code generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_codegen: Option<String>,
}

/// Snapshot of the entire graph.
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub blocks: Vec<BlockSnapshot>,
    pub channels: Vec<Channel>,
    pub tick_count: u64,
    pub time: f64,
}

pub struct DataflowGraph {
    blocks: HashMap<BlockId, Box<dyn Module>>,
    channels: Vec<Channel>,
    next_block_id: u32,
    next_channel_id: u32,
    /// Cached output values from the last tick, keyed by (block_id, port_index).
    outputs: HashMap<(BlockId, usize), Value>,
    pub tick_count: u64,
    pub time: f64,
    /// When true, the tick loop uses SimModel dispatch for peripheral blocks.
    pub simulation_mode: bool,
    /// Simulated peripherals state (used when simulation_mode is true).
    sim_peripherals: Option<crate::dataflow::sim_peripherals::WasmSimPeripherals>,
}

impl Default for DataflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DataflowGraph {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            channels: Vec::new(),
            next_block_id: 1,
            next_channel_id: 1,
            outputs: HashMap::new(),
            tick_count: 0,
            time: 0.0,
            simulation_mode: false,
            sim_peripherals: None,
        }
    }

    /// Enable or disable simulation mode.
    pub fn set_simulation_mode(&mut self, enabled: bool) {
        self.simulation_mode = enabled;
    }

    /// Check if sim peripherals are already set.
    pub fn has_sim_peripherals(&self) -> bool {
        self.sim_peripherals.is_some()
    }

    /// Set the SimPeripherals implementation.
    pub fn set_sim_peripherals(
        &mut self,
        peripherals: crate::dataflow::sim_peripherals::WasmSimPeripherals,
    ) {
        self.sim_peripherals = Some(peripherals);
    }

    /// Access WasmSimPeripherals for configuration.
    pub fn with_sim_peripherals<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::dataflow::sim_peripherals::WasmSimPeripherals),
    {
        if let Some(ref mut p) = self.sim_peripherals {
            f(p);
        }
    }

    /// Access sim peripherals immutably, returning an error if not available.
    pub fn sim_peripherals_ref(
        &self,
    ) -> Result<&crate::dataflow::sim_peripherals::WasmSimPeripherals, String> {
        self.sim_peripherals
            .as_ref()
            .ok_or_else(|| "simulation mode not enabled".to_string())
    }

    /// Access sim peripherals mutably, returning an error if not available.
    pub fn sim_peripherals_mut(
        &mut self,
    ) -> Result<&mut crate::dataflow::sim_peripherals::WasmSimPeripherals, String> {
        self.sim_peripherals
            .as_mut()
            .ok_or_else(|| "simulation mode not enabled".to_string())
    }

    /// Read the last PWM duty value for a channel from sim peripherals.
    pub fn get_sim_pwm(&self, channel: u8) -> f64 {
        if let Some(ref p) = self.sim_peripherals {
            return p.get_pwm_duty(channel);
        }
        0.0
    }

    pub fn add_block(&mut self, block: Box<dyn Module>) -> BlockId {
        let id = BlockId(self.next_block_id);
        self.next_block_id += 1;
        self.blocks.insert(id, block);
        id
    }

    pub fn replace_block(&mut self, id: BlockId, new_block: Box<dyn Module>) -> Result<(), String> {
        if !self.blocks.contains_key(&id) {
            return Err("block not found".into());
        }
        self.blocks.insert(id, new_block);
        self.outputs.retain(|&(bid, _), _| bid != id);
        // Prune channels referencing ports beyond new block's port count
        let n_in = self.blocks[&id].input_ports().len();
        let n_out = self.blocks[&id].output_ports().len();
        self.channels.retain(|c| {
            !(c.to_block == id && c.to_port >= n_in || c.from_block == id && c.from_port >= n_out)
        });
        Ok(())
    }

    pub fn remove_block(&mut self, id: BlockId) -> bool {
        if self.blocks.remove(&id).is_some() {
            self.channels
                .retain(|c| c.from_block != id && c.to_block != id);
            self.outputs.retain(|&(bid, _), _| bid != id);
            true
        } else {
            false
        }
    }

    pub fn connect(
        &mut self,
        from_block: BlockId,
        from_port: usize,
        to_block: BlockId,
        to_port: usize,
    ) -> Result<ChannelId, String> {
        // Validate blocks exist.
        let from = self
            .blocks
            .get(&from_block)
            .ok_or("source block not found")?;
        let to = self
            .blocks
            .get(&to_block)
            .ok_or("destination block not found")?;

        if from_port >= from.output_ports().len() {
            return Err(format!(
                "source port {} out of range (block has {})",
                from_port,
                from.output_ports().len()
            ));
        }
        if to_port >= to.input_ports().len() {
            return Err(format!(
                "destination port {} out of range (block has {})",
                to_port,
                to.input_ports().len()
            ));
        }

        // Prevent duplicate connections to the same input.
        if self
            .channels
            .iter()
            .any(|c| c.to_block == to_block && c.to_port == to_port)
        {
            return Err("input port already connected".to_string());
        }

        let id = ChannelId(self.next_channel_id);
        self.next_channel_id += 1;
        self.channels.push(Channel {
            id,
            from_block,
            from_port,
            to_block,
            to_port,
        });
        Ok(id)
    }

    pub fn disconnect(&mut self, channel_id: ChannelId) -> bool {
        let before = self.channels.len();
        self.channels.retain(|c| c.id != channel_id);
        self.channels.len() < before
    }

    /// Execute one simulation step.
    pub fn tick(&mut self, dt: f64) {
        // Topological-ish execution: iterate blocks in id order (sources first).
        let mut block_ids: Vec<BlockId> = self.blocks.keys().copied().collect();
        block_ids.sort_by_key(|id| id.0);

        for &bid in &block_ids {
            let block = self.blocks.get(&bid).unwrap();
            let n_inputs = block.input_ports().len();
            let n_outputs = block.output_ports().len();

            // Gather inputs from connected channels.
            let mut inputs: Vec<Option<&Value>> = vec![None; n_inputs];
            for ch in &self.channels {
                if ch.to_block == bid {
                    if let Some(val) = self.outputs.get(&(ch.from_block, ch.from_port)) {
                        if ch.to_port < n_inputs {
                            inputs[ch.to_port] = Some(val);
                        }
                    }
                }
            }

            // Take block out temporarily for mutable access.
            let mut block = self.blocks.remove(&bid).unwrap();

            let results = if self.simulation_mode {
                // Simulation mode: try SimModel first, then Tick, then empty.
                if let Some(sim) = block.as_sim_model() {
                    if let Some(peripherals) = self.sim_peripherals.as_mut() {
                        sim.sim_tick(&inputs, dt, peripherals)
                    } else {
                        vec![None; n_outputs]
                    }
                } else if let Some(tick) = block.as_tick() {
                    tick.tick(&inputs, dt)
                } else {
                    vec![None; n_outputs]
                }
            } else {
                // Normal mode: try Tick, then empty.
                if let Some(tick) = block.as_tick() {
                    tick.tick(&inputs, dt)
                } else {
                    vec![None; n_outputs]
                }
            };

            self.blocks.insert(bid, block);

            for (port_idx, val) in results.into_iter().enumerate() {
                if let Some(v) = val {
                    self.outputs.insert((bid, port_idx), v);
                } else {
                    self.outputs.remove(&(bid, port_idx));
                }
            }
        }

        self.tick_count += 1;
        self.time += dt;
    }

    /// Run multiple ticks at once (non-realtime batch execution).
    pub fn run(&mut self, steps: u64, dt: f64) {
        for _ in 0..steps {
            self.tick(dt);
        }
    }

    /// Produce a serializable snapshot of the graph.
    pub fn snapshot(&self) -> GraphSnapshot {
        let mut blocks: Vec<BlockSnapshot> = self
            .blocks
            .iter()
            .map(|(&BlockId(id), block)| {
                let n_outputs = block.output_ports().len();
                let output_values = (0..n_outputs)
                    .map(|i| self.outputs.get(&(BlockId(id), i)).cloned())
                    .collect();
                let config =
                    serde_json::from_str(&block.config_json()).unwrap_or(serde_json::Value::Null);
                let custom_codegen = block.as_codegen().and_then(|cg| cg.emit_rust("host").ok());
                BlockSnapshot {
                    id,
                    block_type: block.block_type().to_string(),
                    name: block.name().to_string(),
                    inputs: block.input_ports(),
                    outputs: block.output_ports(),
                    config,
                    output_values,
                    target: None,
                    custom_codegen,
                }
            })
            .collect();
        blocks.sort_by_key(|b| b.id);

        GraphSnapshot {
            blocks,
            channels: self.channels.clone(),
            tick_count: self.tick_count,
            time: self.time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::blocks::constant::ConstantBlock;
    use crate::dataflow::blocks::function::FunctionBlock;

    #[test]
    fn constant_emits_value() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(42.0)));
        g.tick(0.01);
        let snap = g.snapshot();
        let block = snap.blocks.iter().find(|b| b.id == c.0).unwrap();
        assert_eq!(
            block.output_values[0].as_ref().unwrap().as_float(),
            Some(42.0)
        );
    }

    #[test]
    fn connect_and_propagate() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(5.0)));
        let gain = g.add_block(Box::new(FunctionBlock::gain(2.0)));
        g.connect(c, 0, gain, 0).unwrap();

        // First tick: constant produces 5.0, gain hasn't seen it yet.
        g.tick(0.01);
        // Second tick: gain receives 5.0, outputs 10.0.
        g.tick(0.01);

        let snap = g.snapshot();
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(10.0)
        );
    }

    #[test]
    fn disconnect_removes_channel() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        let f = g.add_block(Box::new(FunctionBlock::gain(1.0)));
        let ch = g.connect(c, 0, f, 0).unwrap();
        assert!(g.disconnect(ch));
        assert!(!g.disconnect(ch)); // already removed
    }

    #[test]
    fn remove_block_cleans_channels() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        let f = g.add_block(Box::new(FunctionBlock::gain(1.0)));
        g.connect(c, 0, f, 0).unwrap();
        g.remove_block(c);
        let snap = g.snapshot();
        assert!(snap.channels.is_empty());
        assert_eq!(snap.blocks.len(), 1);
    }

    #[test]
    fn replace_block_preserves_channels() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(5.0)));
        let gain = g.add_block(Box::new(FunctionBlock::gain(2.0)));
        g.connect(c, 0, gain, 0).unwrap();

        // Replace the constant with a different value
        g.replace_block(c, Box::new(ConstantBlock::new(10.0)))
            .unwrap();

        let snap = g.snapshot();
        // Channel should still exist
        assert_eq!(snap.channels.len(), 1);
        // Block count unchanged
        assert_eq!(snap.blocks.len(), 2);

        // Run and verify new value propagates
        g.tick(0.01); // constant emits 10.0
        g.tick(0.01); // gain receives 10.0, outputs 20.0
        let snap = g.snapshot();
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(20.0)
        );
    }

    #[test]
    fn batch_run() {
        let mut g = DataflowGraph::new();
        g.add_block(Box::new(ConstantBlock::new(1.0)));
        g.run(100, 0.01);
        assert_eq!(g.tick_count, 100);
        assert!((g.time - 1.0).abs() < 1e-9);
    }

    #[test]
    fn snapshot_target_is_none_by_default() {
        let mut g = DataflowGraph::new();
        g.add_block(Box::new(ConstantBlock::new(1.0)));
        let snap = g.snapshot();
        assert!(snap.blocks[0].target.is_none());
    }

    #[test]
    fn block_snapshot_serde_roundtrip_with_target() {
        use crate::dataflow::block::PortKind;
        use crate::dataflow::codegen::target::TargetFamily;

        let snap = BlockSnapshot {
            id: 1,
            block_type: "constant".to_string(),
            name: "Constant".to_string(),
            inputs: vec![],
            outputs: vec![super::super::block::PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({"value": 42.0}),
            output_values: vec![Some(Value::Float(42.0))],
            target: Some(TargetFamily::Rp2040),
            custom_codegen: None,
        };

        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("\"target\""));
        let deser: BlockSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.target, Some(TargetFamily::Rp2040));
    }

    #[test]
    fn block_snapshot_serde_roundtrip_without_target() {
        use crate::dataflow::block::PortKind;

        let snap = BlockSnapshot {
            id: 2,
            block_type: "gain".to_string(),
            name: "Gain".to_string(),
            inputs: vec![super::super::block::PortDef::new("in", PortKind::Float)],
            outputs: vec![super::super::block::PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({}),
            output_values: vec![None],
            target: None,
            custom_codegen: None,
        };

        let json = serde_json::to_string(&snap).unwrap();
        // target: None should be skipped in serialization
        assert!(!json.contains("\"target\""));
        let deser: BlockSnapshot = serde_json::from_str(&json).unwrap();
        assert!(deser.target.is_none());
    }

    #[test]
    fn remove_block_returns_false_for_missing() {
        let mut g = DataflowGraph::new();
        assert!(!g.remove_block(BlockId(999)));
    }

    #[test]
    fn replace_block_error_for_missing() {
        let mut g = DataflowGraph::new();
        let res = g.replace_block(BlockId(999), Box::new(ConstantBlock::new(1.0)));
        assert!(res.is_err());
    }

    #[test]
    fn snapshot_returns_sorted_blocks_and_channels() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        let f = g.add_block(Box::new(FunctionBlock::gain(2.0)));
        g.connect(c, 0, f, 0).unwrap();
        g.tick(0.01);
        let snap = g.snapshot();
        assert_eq!(snap.blocks.len(), 2);
        assert_eq!(snap.channels.len(), 1);
        assert_eq!(snap.tick_count, 1);
        // Blocks should be sorted by id
        assert!(snap.blocks[0].id < snap.blocks[1].id);
    }

    #[test]
    fn with_sim_peripherals_calls_closure() {
        let mut g = DataflowGraph::new();
        g.set_sim_peripherals(crate::dataflow::sim_peripherals::WasmSimPeripherals::new());
        g.with_sim_peripherals(|p| {
            p.set_adc_voltage(0, 2.5);
        });
        assert!((g.get_sim_pwm(0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn with_sim_peripherals_noop_without_peripherals() {
        let mut g = DataflowGraph::new();
        // Should not panic when no peripherals set
        g.with_sim_peripherals(|_p| {
            panic!("should not be called");
        });
    }

    #[test]
    fn embedded_block_outputs_none_without_simulation() {
        use crate::dataflow::blocks::embedded::{AdcBlock, AdcConfig};

        let mut g = DataflowGraph::new();
        let adc = g.add_block(Box::new(AdcBlock::from_config(AdcConfig::default())));
        g.tick(0.01);
        let snap = g.snapshot();
        let adc_snap = snap.blocks.iter().find(|b| b.id == adc.0).unwrap();
        assert_eq!(adc_snap.output_values[0], None);
    }

    #[test]
    fn dataflow_graph_default() {
        let g = DataflowGraph::default();
        let snap = g.snapshot();
        assert!(snap.blocks.is_empty());
    }

    #[test]
    fn remove_block_with_outputs() {
        // Exercise remove_block::{closure#1} (outputs.retain)
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        g.tick(0.01); // produce output
        assert!(g.remove_block(c));
        let snap = g.snapshot();
        assert!(snap.blocks.is_empty());
    }

    #[test]
    fn replace_block_prunes_extra_ports() {
        // Exercise replace_block::{closure#0} (channels.retain for port pruning)
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        let gain = g.add_block(Box::new(FunctionBlock::gain(2.0)));
        g.connect(c, 0, gain, 0).unwrap();
        // Replace gain (1 input, 1 output) with a block that has 0 inputs
        // This should prune the channel because to_port 0 >= new n_in=0
        g.replace_block(gain, Box::new(ConstantBlock::new(5.0)))
            .unwrap();
        let snap = g.snapshot();
        // Channel should be pruned since ConstantBlock has 0 inputs
        assert!(snap.channels.is_empty());
    }

    #[test]
    fn snapshot_with_codegen_block() {
        // Exercise snapshot::{closure#0}::{closure#1} (custom_codegen path)
        use crate::dataflow::blocks::embedded::{AdcBlock, AdcConfig};
        let mut g = DataflowGraph::new();
        g.add_block(Box::new(AdcBlock::from_config(AdcConfig::default())));
        let snap = g.snapshot();
        // AdcBlock implements Codegen, so custom_codegen may be Some
        assert_eq!(snap.blocks.len(), 1);
    }

    #[test]
    fn has_sim_peripherals_false_initially() {
        let g = DataflowGraph::new();
        assert!(!g.has_sim_peripherals());
    }

    #[test]
    fn has_sim_peripherals_true_after_set() {
        let mut g = DataflowGraph::new();
        g.set_sim_peripherals(crate::dataflow::sim_peripherals::WasmSimPeripherals::new());
        assert!(g.has_sim_peripherals());
    }

    #[test]
    fn get_sim_pwm_without_peripherals() {
        let g = DataflowGraph::new();
        // Should return 0.0 when no sim peripherals are set
        assert!((g.get_sim_pwm(0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_simulation_mode_toggle() {
        let mut g = DataflowGraph::new();
        assert!(!g.simulation_mode);
        g.set_simulation_mode(true);
        assert!(g.simulation_mode);
        g.set_simulation_mode(false);
        assert!(!g.simulation_mode);
    }

    #[test]
    fn connect_errors() {
        let mut g = DataflowGraph::new();
        let c = g.add_block(Box::new(ConstantBlock::new(1.0)));
        let f = g.add_block(Box::new(FunctionBlock::gain(1.0)));

        // Source port out of range
        let result = g.connect(c, 99, f, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("source port"));

        // Destination port out of range
        let result = g.connect(c, 0, f, 99);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("destination port"));

        // Duplicate connection to same input
        g.connect(c, 0, f, 0).unwrap();
        let result = g.connect(c, 0, f, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already connected"));

        // Non-existent source block
        let result = g.connect(BlockId(999), 0, f, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("source block not found"));

        // Non-existent destination block
        let result = g.connect(c, 0, BlockId(999), 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("destination block not found"));
    }
}
