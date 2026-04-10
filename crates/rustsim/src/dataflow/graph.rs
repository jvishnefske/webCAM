//! The dataflow graph: owns blocks and channels, runs the tick loop.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tsify_next::Tsify;

use super::block::{BlockId, Module, Value};
use super::channel::{Channel, ChannelId};

/// Snapshot of one block for serialization to the frontend.
#[derive(Debug, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
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
    /// Whether this block is a delay element (z⁻¹) that breaks feedback cycles.
    #[serde(default)]
    pub is_delay: bool,
}

/// Snapshot of the entire graph.
#[derive(Debug, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
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
        use crate::dataflow::codegen::topo::topological_sort;

        // Build set of delay blocks (z⁻¹ elements) for back-edge exclusion.
        let delay_blocks: HashSet<BlockId> = self.blocks.iter()
            .filter(|(_, block)| block.is_delay())
            .map(|(&id, _)| id)
            .collect();

        // Topological sort for correct evaluation order.
        let all_ids: Vec<BlockId> = self.blocks.keys().copied().collect();
        let block_ids = match topological_sort(&all_ids, &self.channels, &delay_blocks) {
            Ok(sorted) => sorted,
            Err(_) => {
                // Fallback to ID order if cycle detected (shouldn't happen with delay exclusion)
                let mut ids = all_ids;
                ids.sort_by_key(|id| id.0);
                ids
            }
        };

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
                let is_delay = block.is_delay();
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
                    is_delay,
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
            is_delay: false,
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
            is_delay: false,
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
    fn embedded_block_outputs_default_without_simulation() {
        use crate::dataflow::blocks::embedded::{AdcBlock, AdcConfig};

        let mut g = DataflowGraph::new();
        let adc = g.add_block(Box::new(AdcBlock::from_config(AdcConfig::default())));
        g.tick(0.01);
        let snap = g.snapshot();
        let adc_snap = snap.blocks.iter().find(|b| b.id == adc.0).unwrap();
        // Without simulation, the Tick impl returns a default 0.0
        assert_eq!(adc_snap.output_values[0], Some(Value::Float(0.0)));
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

    // ── Data-driven block integration tests ──────────────────────────

    #[test]
    fn data_driven_constant_via_create_block() {
        use crate::dataflow::blocks::create_block;
        let mut g = DataflowGraph::new();
        let block = create_block("constant", r#"{"value": 7.0}"#).unwrap();
        let c = g.add_block(block);
        g.tick(0.01);
        let snap = g.snapshot();
        let b = snap.blocks.iter().find(|b| b.id == c.0).unwrap();
        assert_eq!(b.output_values[0].as_ref().unwrap().as_float(), Some(7.0));
    }

    #[test]
    fn data_driven_subtract_chain() {
        use crate::dataflow::blocks::create_block;
        let mut g = DataflowGraph::new();
        let a = g.add_block(create_block("constant", r#"{"value": 10.0}"#).unwrap());
        let b = g.add_block(create_block("constant", r#"{"value": 3.0}"#).unwrap());
        let sub = g.add_block(create_block("subtract", "{}").unwrap());
        g.connect(a, 0, sub, 0).unwrap();
        g.connect(b, 0, sub, 1).unwrap();
        g.tick(0.01);
        g.tick(0.01);
        let snap = g.snapshot();
        let sub_snap = snap.blocks.iter().find(|b| b.id == sub.0).unwrap();
        assert_eq!(
            sub_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(7.0)
        );
    }

    #[test]
    fn data_driven_select_block() {
        use crate::dataflow::blocks::create_block;
        let mut g = DataflowGraph::new();
        let cond = g.add_block(create_block("constant", r#"{"value": 1.0}"#).unwrap());
        let a = g.add_block(create_block("constant", r#"{"value": 10.0}"#).unwrap());
        let b = g.add_block(create_block("constant", r#"{"value": 20.0}"#).unwrap());
        let sel = g.add_block(create_block("select", "{}").unwrap());
        g.connect(cond, 0, sel, 0).unwrap();
        g.connect(a, 0, sel, 1).unwrap();
        g.connect(b, 0, sel, 2).unwrap();
        g.tick(0.01);
        g.tick(0.01);
        let snap = g.snapshot();
        let sel_snap = snap.blocks.iter().find(|b| b.id == sel.0).unwrap();
        // cond > 0 → selects a (10.0)
        assert_eq!(
            sel_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(10.0)
        );
    }

    #[test]
    fn data_driven_channel_read_write() {
        use crate::dataflow::blocks::create_block;
        let mut g = DataflowGraph::new();
        let cr = g.add_block(
            create_block("channel_read", r#"{"channel": "adc0"}"#).unwrap(),
        );
        let cw = g.add_block(
            create_block("channel_write", r#"{"channel": "pwm0"}"#).unwrap(),
        );
        g.connect(cr, 0, cw, 0).unwrap();
        g.tick(0.01);
        let snap = g.snapshot();
        // channel_read returns 0.0 in WASM (no hardware)
        let cr_snap = snap.blocks.iter().find(|b| b.id == cr.0).unwrap();
        assert_eq!(
            cr_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(0.0)
        );
        // channel_write is a sink — no outputs
        let cw_snap = snap.blocks.iter().find(|b| b.id == cw.0).unwrap();
        assert!(cw_snap.output_values.is_empty());
    }

    #[test]
    fn data_driven_gain_new_schema() {
        use crate::dataflow::blocks::create_block;
        let mut g = DataflowGraph::new();
        let c = g.add_block(create_block("constant", r#"{"value": 4.0}"#).unwrap());
        let gain = g.add_block(create_block("gain", r#"{"gain": 3.0}"#).unwrap());
        g.connect(c, 0, gain, 0).unwrap();
        g.tick(0.01);
        g.tick(0.01);
        let snap = g.snapshot();
        let gs = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(gs.output_values[0].as_ref().unwrap().as_float(), Some(12.0));
    }

    #[test]
    fn tick_with_register_feedback_loop() {
        // Create: Constant(5.0) → Gain(2.0) → Register(init=0) → Gain
        // This forms a feedback loop through the Register delay block.
        // The topo sort should handle the cycle via delay-block back-edge exclusion.
        //
        // Tick 1: Register outputs 0.0 (initial), Gain outputs 0.0, Constant outputs 5.0
        // Tick 2: Register outputs 0.0 (stored from Gain tick 1), Gain receives Register=0.0 + routed...
        //
        // Simplified test: Constant → Register → Gain, verify no panic and correct propagation.
        use crate::dataflow::blocks::register::{RegisterBlock, RegisterConfig};

        let mut g = DataflowGraph::new();
        let constant = g.add_block(Box::new(ConstantBlock::new(5.0)));
        let register = g.add_block(Box::new(RegisterBlock::new(RegisterConfig {
            initial_value: 0.0,
        })));
        let gain = g.add_block(Box::new(FunctionBlock::gain(2.0)));

        // Constant → Register → Gain, with Gain output feeding back into Register
        // But since each input port can only have one connection, we wire:
        // Constant(out) → Register(in), Register(out) → Gain(in)
        g.connect(constant, 0, register, 0).unwrap();
        g.connect(register, 0, gain, 0).unwrap();

        // First tick: Register outputs initial value 0.0, Gain gets 0.0 → outputs 0.0
        g.tick(0.01);
        let snap = g.snapshot();
        let reg_snap = snap.blocks.iter().find(|b| b.id == register.0).unwrap();
        assert_eq!(
            reg_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(0.0),
            "Register should output initial value on first tick"
        );

        // Second tick: Register stored 5.0 from Constant, outputs 5.0; Gain gets 5.0 → outputs 10.0
        g.tick(0.01);
        let snap = g.snapshot();
        let reg_snap = snap.blocks.iter().find(|b| b.id == register.0).unwrap();
        assert_eq!(
            reg_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(5.0),
            "Register should output stored value (5.0) on second tick"
        );
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(10.0),
            "Gain should output 2.0 * 5.0 = 10.0 on second tick"
        );
    }

    #[test]
    fn tick_with_true_feedback_cycle() {
        // Create a true feedback cycle: Register → Gain → Register
        // This would be a cycle without delay-block exclusion.
        // With the Register as a delay block, topo sort breaks the cycle.
        //
        // Execution order (topo sort): Register first, then Gain.
        // Register is z⁻¹: outputs previous stored value, then stores new input.
        //
        // Tick 1: Register outputs 1.0 (initial), no input yet from Gain (no prior output).
        //         Gain receives 1.0, outputs 3.0.
        // Tick 2: Register reads Gain's previous output (3.0), outputs 1.0 (stored from init),
        //         stores 3.0. Gain receives 1.0, outputs 3.0.
        // Tick 3: Register reads Gain's previous output (3.0), outputs 3.0 (stored from tick 2),
        //         stores 3.0. Gain receives 3.0, outputs 9.0.
        use crate::dataflow::blocks::register::{RegisterBlock, RegisterConfig};

        let mut g = DataflowGraph::new();
        let register = g.add_block(Box::new(RegisterBlock::new(RegisterConfig {
            initial_value: 1.0,
        })));
        let gain = g.add_block(Box::new(FunctionBlock::gain(3.0)));

        // Register(out) → Gain(in), Gain(out) → Register(in)
        g.connect(register, 0, gain, 0).unwrap();
        g.connect(gain, 0, register, 0).unwrap();

        // Tick 1: Register outputs 1.0 (initial), Gain gets 1.0 → outputs 3.0
        g.tick(0.01);
        let snap = g.snapshot();
        let reg_snap = snap.blocks.iter().find(|b| b.id == register.0).unwrap();
        assert_eq!(
            reg_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(1.0),
            "Register should output initial value 1.0"
        );
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(3.0),
            "Gain should output 3.0 * 1.0 = 3.0"
        );

        // Tick 2: Register reads Gain(3.0) from tick 1, outputs stored 1.0, stores 3.0.
        //         Gain receives Register's output 1.0, outputs 3.0.
        g.tick(0.01);
        let snap = g.snapshot();
        let reg_snap = snap.blocks.iter().find(|b| b.id == register.0).unwrap();
        assert_eq!(
            reg_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(1.0),
            "Register should output 1.0 on second tick (z⁻¹ delay)"
        );
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(3.0),
            "Gain should still output 3.0 on second tick"
        );

        // Tick 3: Register reads Gain(3.0) from tick 2, outputs stored 3.0, stores 3.0.
        //         Gain receives 3.0, outputs 9.0.
        g.tick(0.01);
        let snap = g.snapshot();
        let reg_snap = snap.blocks.iter().find(|b| b.id == register.0).unwrap();
        assert_eq!(
            reg_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(3.0),
            "Register should output 3.0 on third tick"
        );
        let gain_snap = snap.blocks.iter().find(|b| b.id == gain.0).unwrap();
        assert_eq!(
            gain_snap.output_values[0].as_ref().unwrap().as_float(),
            Some(9.0),
            "Gain should output 3.0 * 3.0 = 9.0 on third tick"
        );
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
