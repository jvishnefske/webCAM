//! Lightweight DAG runtime: deserialize a graph, build a tickable object
//! that receives channel calls.
//!
//! # Design
//!
//! - **Curried enums**: [`BlockFn`] variants capture block config (partial
//!   application). [`BlockFn::call`] supplies only the dynamic inputs.
//! - **Typeless container**: All values live in a flat `Vec<f64>` state buffer.
//!   Type erasure through numeric representation — mirrors the MLIR backend's
//!   collapse of every `PortKind` to `double`.
//! - **Receivable object**: [`DagRuntime::receive`] injects values by topic
//!   name; [`DagRuntime::tick`] executes the full graph.

use std::collections::{HashMap, VecDeque};

use crate::lower::{BlockId, BlockSnapshot, Channel, GraphSnapshot};

// ---------------------------------------------------------------------------
// Hardware bridge trait
// ---------------------------------------------------------------------------

/// Hardware abstraction for peripheral blocks.
///
/// Default implementations return zero / do nothing, so a test harness only
/// needs to override the methods it cares about.
#[allow(unused_variables)]
pub trait HardwareBridge {
    fn adc_read(&self, channel: u8) -> f64 { 0.0 }
    fn pwm_write(&mut self, channel: u8, duty: f64) {}
    fn gpio_read(&self, pin: u8) -> f64 { 0.0 }
    fn gpio_write(&mut self, pin: u8, value: f64) {}
    fn uart_read(&self, port: u8) -> f64 { 0.0 }
    fn uart_write(&mut self, port: u8, value: f64) {}
    fn encoder_read(&self, channel: u8) -> f64 { 0.0 }
    fn display_write(&mut self, bus: u8, addr: u8, line1: f64, line2: f64) {}
    fn stepper_move(&mut self, port: u8, target: f64) {}
    fn stepper_position(&self, port: u8) -> f64 { 0.0 }
    fn stepper_enable(&mut self, port: u8, enabled: bool) {}
    fn stallguard_read(&self, port: u8, addr: u8) -> f64 { 0.0 }
    fn publish(&mut self, topic: &str, value: f64) {}
    fn subscribe(&self, topic: &str) -> f64 { 0.0 }
}

/// No-op hardware bridge for pure-logic testing.
pub struct NullHardware;
impl HardwareBridge for NullHardware {}

// ---------------------------------------------------------------------------
// Curried block function enum
// ---------------------------------------------------------------------------

/// State machine transition descriptor.
#[derive(Debug, Clone)]
pub struct SmTransition {
    pub from: u8,
    pub guard: u8,
    pub to: u8,
}

/// Curried block function.
///
/// Each variant captures its configuration via partial application.
/// [`BlockFn::call`] supplies only the dynamic inputs read from the
/// state buffer, producing outputs written back into it.
///
/// Stored in a typeless container (flat `Vec<f64>`) — the enum itself
/// is the only place where block semantics survive.
#[derive(Debug, Clone)]
pub enum BlockFn {
    /// `constant(value)() → [value]`
    Constant(f64),
    /// `gain(factor)(x) → [x * factor]`
    Gain(f64),
    /// `add()(a, b) → [a + b]`
    Add,
    /// `multiply()(a, b) → [a * b]`
    Multiply,
    /// `clamp(min, max)(x) → [x.clamp(min, max)]`
    Clamp(f64, f64),
    /// `adc_read(channel)() → [voltage]`
    AdcRead(u8),
    /// `pwm_write(channel)(duty) → []`
    PwmWrite(u8),
    /// `gpio_read(pin)() → [level]`
    GpioRead(u8),
    /// `gpio_write(pin)(value) → []`
    GpioWrite(u8),
    /// `uart_rx(port)() → [data]`
    UartRx(u8),
    /// `uart_tx(port)(data) → []`
    UartTx(u8),
    /// `encoder_read(channel)() → [position, velocity]`
    EncoderRead(u8),
    /// `display_write(bus, addr)(line1, line2) → []`
    DisplayWrite(u8, u8),
    /// `stepper(port)(target, enable) → [position]`
    Stepper(u8),
    /// `stallguard(port, addr, threshold)() → [value, detected]`
    StallGuard { port: u8, addr: u8, threshold: f64 },
    /// `subscribe(topic)() → [value]`
    Subscribe(String),
    /// `publish(topic)(value) → []`
    Publish(String),
    /// Curried transition table: config captured, guards are dynamic inputs.
    StateMachine {
        n_states: u8,
        initial: u8,
        transitions: Vec<SmTransition>,
    },
    /// Skipped block (plot, json_encode, json_decode).
    Nop,
}

impl BlockFn {
    /// Construct a curried `BlockFn` from a serialized block snapshot.
    ///
    /// The block's `config` JSON is consumed here (partial application);
    /// only dynamic port values remain for [`call`](Self::call).
    pub fn from_snapshot(block: &BlockSnapshot) -> Result<Self, String> {
        let cfg = &block.config;
        Ok(match block.block_type.as_str() {
            "constant" => Self::Constant(
                cfg.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0),
            ),
            "gain" => Self::Gain(
                cfg.get("param1").and_then(|v| v.as_f64()).unwrap_or(1.0),
            ),
            "add" => Self::Add,
            "multiply" => Self::Multiply,
            "clamp" => Self::Clamp(
                cfg.get("param1").and_then(|v| v.as_f64()).unwrap_or(f64::MIN),
                cfg.get("param2").and_then(|v| v.as_f64()).unwrap_or(f64::MAX),
            ),
            "adc_source" => Self::AdcRead(cfg_u8(cfg, "channel")),
            "pwm_sink" => Self::PwmWrite(cfg_u8(cfg, "channel")),
            "gpio_in" => Self::GpioRead(cfg_u8(cfg, "pin")),
            "gpio_out" => Self::GpioWrite(cfg_u8(cfg, "pin")),
            "uart_rx" => Self::UartRx(cfg_u8(cfg, "port")),
            "uart_tx" => Self::UartTx(cfg_u8(cfg, "port")),
            "encoder" => Self::EncoderRead(cfg_u8(cfg, "channel")),
            "ssd1306_display" => Self::DisplayWrite(
                cfg_u8(cfg, "i2c_bus"),
                cfg.get("address").and_then(|v| v.as_u64()).unwrap_or(0x3C) as u8,
            ),
            "tmc2209_stepper" => Self::Stepper(cfg_u8(cfg, "uart_port")),
            "tmc2209_stallguard" => Self::StallGuard {
                port: cfg_u8(cfg, "uart_port"),
                addr: cfg_u8(cfg, "uart_addr"),
                threshold: cfg.get("threshold").and_then(|v| v.as_f64()).unwrap_or(0.0),
            },
            "pubsub_source" => Self::Subscribe(cfg_string(cfg, "topic")),
            "pubsub_sink" => Self::Publish(cfg_string(cfg, "topic")),
            "state_machine" => parse_state_machine(cfg)?,
            "plot" | "json_encode" | "json_decode" => Self::Nop,
            other => return Err(format!("unsupported block type: {other}")),
        })
    }

    /// Number of output slots this curried function produces.
    pub fn n_outputs(&self) -> usize {
        match self {
            Self::Constant(_) | Self::Gain(_) | Self::Add | Self::Multiply
            | Self::Clamp(_, _) | Self::AdcRead(_) | Self::GpioRead(_)
            | Self::UartRx(_) | Self::Subscribe(_) | Self::Stepper(_) => 1,
            Self::EncoderRead(_) | Self::StallGuard { .. } => 2,
            Self::StateMachine { n_states, .. } => 1 + *n_states as usize,
            Self::PwmWrite(_) | Self::GpioWrite(_) | Self::UartTx(_)
            | Self::DisplayWrite(_, _) | Self::Publish(_) | Self::Nop => 0,
        }
    }

    /// Execute the curried function.
    ///
    /// Reads dynamic `inputs` from the caller, writes results into `outputs`.
    /// Hardware side-effects go through `hw`.
    pub fn call(&self, inputs: &[f64], outputs: &mut [f64], hw: &mut dyn HardwareBridge) {
        match self {
            Self::Constant(v) => {
                set(outputs, 0, *v);
            }
            Self::Gain(factor) => {
                set(outputs, 0, inp(inputs, 0) * factor);
            }
            Self::Add => {
                set(outputs, 0, inp(inputs, 0) + inp(inputs, 1));
            }
            Self::Multiply => {
                set(outputs, 0, inp(inputs, 0) * inp(inputs, 1));
            }
            Self::Clamp(min, max) => {
                set(outputs, 0, inp(inputs, 0).clamp(*min, *max));
            }
            Self::AdcRead(ch) => {
                set(outputs, 0, hw.adc_read(*ch));
            }
            Self::PwmWrite(ch) => {
                hw.pwm_write(*ch, inp(inputs, 0));
            }
            Self::GpioRead(pin) => {
                set(outputs, 0, hw.gpio_read(*pin));
            }
            Self::GpioWrite(pin) => {
                hw.gpio_write(*pin, inp(inputs, 0));
            }
            Self::UartRx(port) => {
                set(outputs, 0, hw.uart_read(*port));
            }
            Self::UartTx(port) => {
                hw.uart_write(*port, inp(inputs, 0));
            }
            Self::EncoderRead(ch) => {
                set(outputs, 0, hw.encoder_read(*ch));
                set(outputs, 1, 0.0); // velocity placeholder
            }
            Self::DisplayWrite(bus, addr) => {
                hw.display_write(*bus, *addr, inp(inputs, 0), inp(inputs, 1));
            }
            Self::Stepper(port) => {
                hw.stepper_enable(*port, inp(inputs, 1) > 0.5);
                hw.stepper_move(*port, inp(inputs, 0));
                set(outputs, 0, hw.stepper_position(*port));
            }
            Self::StallGuard { port, addr, threshold } => {
                let val = hw.stallguard_read(*port, *addr);
                set(outputs, 0, val);
                set(outputs, 1, if val < *threshold { 1.0 } else { 0.0 });
            }
            Self::Subscribe(_) => {
                // Passive mailbox: value is injected by DagRuntime::receive().
                // Output slot retains whatever was written there externally.
            }
            Self::Publish(topic) => {
                hw.publish(topic, inp(inputs, 0));
            }
            Self::StateMachine { n_states, initial, transitions } => {
                // Current state persists in outputs[0] across ticks
                let current = outputs.first().copied().unwrap_or(*initial as f64) as u8;
                let current = if current >= *n_states { *initial } else { current };

                // Evaluate guards — first matching transition wins
                let mut next = current;
                for t in transitions {
                    if t.from == current && inp(inputs, t.guard as usize) > 0.5 {
                        next = t.to;
                        break;
                    }
                }

                // State index
                set(outputs, 0, next as f64);
                // Active flags: 1.0 for current state, 0.0 for others
                for i in 0..*n_states as usize {
                    set(outputs, 1 + i, if i == next as usize { 1.0 } else { 0.0 });
                }
            }
            Self::Nop => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime graph
// ---------------------------------------------------------------------------

/// A wired-up node: curried function + slot indices into the state buffer.
#[derive(Debug)]
pub struct Node {
    pub block_id: u32,
    pub func: BlockFn,
    pub input_slots: Vec<u16>,
    pub output_slots: Vec<u16>,
}

/// Lightweight DAG runtime.
///
/// Deserializes a [`GraphSnapshot`], topo-sorts the blocks, curries each
/// block's config into a [`BlockFn`], and maps ports to indices in a flat
/// `f64` state buffer (the typeless container).
///
/// ```text
/// GraphSnapshot (JSON)
///   → DagRuntime::from_json()
///     → Vec<Node>          topo-sorted curried blocks
///     → Vec<f64>           flat state buffer
///     → HashMap<topic,u16> pub/sub receive map
/// ```
#[derive(Debug)]
pub struct DagRuntime {
    nodes: Vec<Node>,
    state: Vec<f64>,
    /// Pub/sub topic → state-buffer slot for external injection.
    topic_map: HashMap<String, u16>,
}

impl DagRuntime {
    /// Build a runtime from a deserialized [`GraphSnapshot`].
    pub fn from_snapshot(snap: &GraphSnapshot) -> Result<Self, String> {
        let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
        let sorted = topo_sort(&block_ids, &snap.channels)?;
        let block_map: HashMap<u32, &BlockSnapshot> =
            snap.blocks.iter().map(|b| (b.id, b)).collect();

        // --- First pass: curry each block, allocate output slots ---
        let mut slot_counter: u16 = 0;
        let mut slot_map: HashMap<(u32, usize), u16> = HashMap::new();
        let mut staged: Vec<(u32, BlockFn, Vec<u16>)> = Vec::with_capacity(sorted.len());
        let mut topic_map: HashMap<String, u16> = HashMap::new();

        for &BlockId(id) in &sorted {
            let block = block_map
                .get(&id)
                .ok_or_else(|| format!("block {id} not in snapshot"))?;
            let func = BlockFn::from_snapshot(block)?;
            let n_out = func.n_outputs();

            let mut output_slots = Vec::with_capacity(n_out);
            for port_idx in 0..n_out {
                let slot = slot_counter;
                slot_counter = slot_counter
                    .checked_add(1)
                    .ok_or_else(|| "state buffer overflow (>65535 slots)".to_string())?;
                slot_map.insert((id, port_idx), slot);
                output_slots.push(slot);
            }

            // Register pub/sub source topics for receive()
            if let BlockFn::Subscribe(ref topic) = func {
                if !topic.is_empty() {
                    if let Some(&slot) = output_slots.first() {
                        topic_map.insert(topic.clone(), slot);
                    }
                }
            }

            staged.push((id, func, output_slots));
        }

        // Reserve one extra slot that always stays 0.0 (unconnected inputs)
        let zero_slot = slot_counter;
        let state_len = slot_counter as usize + 1;

        // --- Second pass: resolve input wiring ---
        let mut nodes = Vec::with_capacity(staged.len());
        for (id, func, output_slots) in staged {
            let block = block_map[&id];
            let n_in = block.inputs.len();
            let mut input_slots = vec![zero_slot; n_in];

            for ch in &snap.channels {
                if ch.to_block.0 == id && ch.to_port < n_in {
                    if let Some(&slot) = slot_map.get(&(ch.from_block.0, ch.from_port)) {
                        input_slots[ch.to_port] = slot;
                    }
                }
            }

            nodes.push(Node { block_id: id, func, input_slots, output_slots });
        }

        Ok(DagRuntime {
            nodes,
            state: vec![0.0_f64; state_len],
            topic_map,
        })
    }

    /// Deserialize a JSON `GraphSnapshot` and build the runtime.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let snap: GraphSnapshot =
            serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;
        Self::from_snapshot(&snap)
    }

    /// Inject a value into a pub/sub topic's state slot.
    ///
    /// This is how external channel calls reach the graph — the runtime
    /// is an object capable of *receiving* messages by topic name.
    pub fn receive(&mut self, topic: &str, value: f64) {
        if let Some(&slot) = self.topic_map.get(topic) {
            self.state[slot as usize] = value;
        }
    }

    /// Execute one tick of the entire graph.
    pub fn tick(&mut self, hw: &mut dyn HardwareBridge) {
        for idx in 0..self.nodes.len() {
            let inputs: Vec<f64> = self.nodes[idx]
                .input_slots
                .iter()
                .map(|&s| self.state[s as usize])
                .collect();

            let out_slots = &self.nodes[idx].output_slots;
            let mut outputs: Vec<f64> = out_slots.iter().map(|&s| self.state[s as usize]).collect();

            self.nodes[idx].func.call(&inputs, &mut outputs, hw);

            for (i, &slot) in out_slots.iter().enumerate() {
                if let Some(&val) = outputs.get(i) {
                    self.state[slot as usize] = val;
                }
            }
        }
    }

    /// Read the output value of a specific block port.
    pub fn read_output(&self, block_id: u32, port: usize) -> Option<f64> {
        self.nodes
            .iter()
            .find(|n| n.block_id == block_id)
            .and_then(|n| n.output_slots.get(port))
            .map(|&slot| self.state[slot as usize])
    }

    /// Number of state slots allocated.
    pub fn state_len(&self) -> usize {
        self.state.len()
    }

    /// Number of nodes in the runtime graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// List registered pub/sub topics that accept [`receive`](Self::receive).
    pub fn topics(&self) -> Vec<&str> {
        self.topic_map.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn inp(inputs: &[f64], idx: usize) -> f64 {
    inputs.get(idx).copied().unwrap_or(0.0)
}

fn set(outputs: &mut [f64], idx: usize, val: f64) {
    if let Some(o) = outputs.get_mut(idx) {
        *o = val;
    }
}

fn cfg_u8(cfg: &serde_json::Value, key: &str) -> u8 {
    cfg.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as u8
}

fn cfg_string(cfg: &serde_json::Value, key: &str) -> String {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_state_machine(cfg: &serde_json::Value) -> Result<BlockFn, String> {
    let state_names: Vec<&str> = cfg
        .get("states")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();

    let n_states = state_names.len() as u8;
    if n_states == 0 {
        return Err("state_machine has no states".to_string());
    }

    let initial_name = cfg.get("initial").and_then(|v| v.as_str()).unwrap_or("");
    let initial = state_names
        .iter()
        .position(|&s| s == initial_name)
        .unwrap_or(0) as u8;

    let transitions = cfg
        .get("transitions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let from_name = t.get("from")?.as_str()?;
                    let to_name = t.get("to")?.as_str()?;
                    let from = state_names.iter().position(|&s| s == from_name)?;
                    let to = state_names.iter().position(|&s| s == to_name)?;
                    let guard = t.get("guard_port")?.as_u64()? as u8;
                    Some(SmTransition { from: from as u8, guard, to: to as u8 })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(BlockFn::StateMachine { n_states, initial, transitions })
}

// ---------------------------------------------------------------------------
// Topological sort (Kahn's algorithm, mirrors lower.rs)
// ---------------------------------------------------------------------------

fn topo_sort(block_ids: &[BlockId], channels: &[Channel]) -> Result<Vec<BlockId>, String> {
    let mut in_degree: HashMap<BlockId, usize> = block_ids.iter().map(|&id| (id, 0)).collect();
    let mut adj: HashMap<BlockId, Vec<BlockId>> = block_ids.iter().map(|&id| (id, Vec::new())).collect();

    for ch in channels {
        if in_degree.contains_key(&ch.from_block) && in_degree.contains_key(&ch.to_block) {
            *in_degree.entry(ch.to_block).or_insert(0) += 1;
            adj.entry(ch.from_block).or_default().push(ch.to_block);
        }
    }

    let mut queue: VecDeque<BlockId> = {
        let mut sources: Vec<BlockId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        sources.sort_by_key(|id| id.0);
        sources.into_iter().collect()
    };

    let mut result = Vec::with_capacity(block_ids.len());
    while let Some(node) = queue.pop_front() {
        result.push(node);
        let mut neighbors: Vec<BlockId> = adj.get(&node).cloned().unwrap_or_default();
        neighbors.sort_by_key(|id| id.0);
        neighbors.dedup();
        for &neighbor in &neighbors {
            let edge_count = adj
                .get(&node)
                .map(|v| v.iter().filter(|&&n| n == neighbor).count())
                .unwrap_or(0);
            let deg = in_degree.get_mut(&neighbor).expect("block in adj but not in_degree");
            *deg = deg.saturating_sub(edge_count);
            if *deg == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if result.len() != block_ids.len() {
        Err("cycle detected in dataflow graph".to_string())
    } else {
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{ChannelId, PortDef};
    use module_traits::value::PortKind;

    // -- test helpers -------------------------------------------------------

    fn block(id: u32, bt: &str, cfg: serde_json::Value) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: bt.to_string(),
            name: format!("{bt}_{id}"),
            inputs: vec![],
            outputs: vec![PortDef { name: "out".into(), kind: PortKind::Float }],
            config: cfg,
            output_values: vec![],
            custom_codegen: None,
        }
    }

    fn block_with_inputs(
        id: u32,
        bt: &str,
        cfg: serde_json::Value,
        inputs: Vec<(&str, PortKind)>,
    ) -> BlockSnapshot {
        let mut b = block(id, bt, cfg);
        b.inputs = inputs
            .into_iter()
            .map(|(n, k)| PortDef { name: n.into(), kind: k })
            .collect();
        b
    }

    fn chan(id: u32, from: u32, fp: usize, to: u32, tp: usize) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from),
            from_port: fp,
            to_block: BlockId(to),
            to_port: tp,
        }
    }

    fn snap(blocks: Vec<BlockSnapshot>, channels: Vec<Channel>) -> GraphSnapshot {
        GraphSnapshot { blocks, channels, tick_count: 0, time: 0.0 }
    }

    // -- BlockFn unit tests -------------------------------------------------

    #[test]
    fn constant_produces_value() {
        let f = BlockFn::Constant(42.0);
        let mut out = [0.0];
        f.call(&[], &mut out, &mut NullHardware);
        assert_eq!(out[0], 42.0);
    }

    #[test]
    fn gain_multiplies() {
        let f = BlockFn::Gain(3.0);
        let mut out = [0.0];
        f.call(&[7.0], &mut out, &mut NullHardware);
        assert_eq!(out[0], 21.0);
    }

    #[test]
    fn add_sums() {
        let f = BlockFn::Add;
        let mut out = [0.0];
        f.call(&[2.5, 3.5], &mut out, &mut NullHardware);
        assert_eq!(out[0], 6.0);
    }

    #[test]
    fn clamp_bounds() {
        let f = BlockFn::Clamp(0.0, 1.0);
        let mut out = [0.0];
        f.call(&[2.0], &mut out, &mut NullHardware);
        assert_eq!(out[0], 1.0);
        f.call(&[-1.0], &mut out, &mut NullHardware);
        assert_eq!(out[0], 0.0);
        f.call(&[0.5], &mut out, &mut NullHardware);
        assert_eq!(out[0], 0.5);
    }

    #[test]
    fn nop_produces_nothing() {
        let f = BlockFn::Nop;
        assert_eq!(f.n_outputs(), 0);
        f.call(&[], &mut [], &mut NullHardware);
    }

    // -- DagRuntime integration tests ---------------------------------------

    #[test]
    fn constant_gain_chain() {
        let s = snap(
            vec![
                block(1, "constant", serde_json::json!({"value": 10.0})),
                block_with_inputs(2, "gain", serde_json::json!({"param1": 5.0}),
                    vec![("in", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 2, 0)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(1, 0), Some(10.0));
        assert_eq!(rt.read_output(2, 0), Some(50.0));
    }

    #[test]
    fn three_block_chain() {
        // const(3) → gain(×2) → gain(×4) = 24
        let s = snap(
            vec![
                block(1, "constant", serde_json::json!({"value": 3.0})),
                block_with_inputs(2, "gain", serde_json::json!({"param1": 2.0}),
                    vec![("in", PortKind::Float)]),
                block_with_inputs(3, "gain", serde_json::json!({"param1": 4.0}),
                    vec![("in", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 2, 0), chan(2, 2, 0, 3, 0)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(3, 0), Some(24.0));
    }

    #[test]
    fn add_two_constants() {
        let s = snap(
            vec![
                block(1, "constant", serde_json::json!({"value": 7.0})),
                block(2, "constant", serde_json::json!({"value": 8.0})),
                block_with_inputs(3, "add", serde_json::json!({}),
                    vec![("a", PortKind::Float), ("b", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 3, 0), chan(2, 2, 0, 3, 1)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(3, 0), Some(15.0));
    }

    #[test]
    fn pubsub_receive_injects_value() {
        let s = snap(
            vec![
                block(1, "pubsub_source", serde_json::json!({"topic": "sensor/temp"})),
                block_with_inputs(2, "gain", serde_json::json!({"param1": 2.0}),
                    vec![("in", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 2, 0)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();

        // Before receive: subscribe returns 0
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(2, 0), Some(0.0));

        // Inject via receive
        rt.receive("sensor/temp", 25.0);
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(2, 0), Some(50.0));
    }

    #[test]
    fn topics_lists_registered() {
        let s = snap(
            vec![
                block(1, "pubsub_source", serde_json::json!({"topic": "alpha"})),
                block(2, "pubsub_source", serde_json::json!({"topic": "beta"})),
            ],
            vec![],
        );
        let rt = DagRuntime::from_snapshot(&s).unwrap();
        let mut topics = rt.topics();
        topics.sort();
        assert_eq!(topics, vec!["alpha", "beta"]);
    }

    #[test]
    fn state_machine_transitions() {
        let cfg = serde_json::json!({
            "states": ["idle", "running", "done"],
            "initial": "idle",
            "transitions": [
                {"from": "idle", "to": "running", "guard_port": 0},
                {"from": "running", "to": "done", "guard_port": 1}
            ]
        });
        let mut b = block(1, "state_machine", cfg);
        b.inputs = vec![
            PortDef { name: "start".into(), kind: PortKind::Float },
            PortDef { name: "finish".into(), kind: PortKind::Float },
        ];
        // state_idx + 3 active flags
        b.outputs = vec![
            PortDef { name: "state".into(), kind: PortKind::Float },
            PortDef { name: "idle".into(), kind: PortKind::Float },
            PortDef { name: "running".into(), kind: PortKind::Float },
            PortDef { name: "done".into(), kind: PortKind::Float },
        ];

        let s = snap(vec![b], vec![]);
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();

        // Initial state: idle (0)
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(1, 0), Some(0.0)); // state_idx = 0 (idle)
        assert_eq!(rt.read_output(1, 1), Some(1.0)); // idle active
        assert_eq!(rt.read_output(1, 2), Some(0.0)); // running inactive

        // No guard fired — stays idle
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(1, 0), Some(0.0));
    }

    #[test]
    fn state_machine_guard_fires() {
        let cfg = serde_json::json!({
            "states": ["off", "on"],
            "initial": "off",
            "transitions": [
                {"from": "off", "to": "on", "guard_port": 0}
            ]
        });
        let mut b = block(1, "state_machine", cfg);
        b.inputs = vec![PortDef { name: "trigger".into(), kind: PortKind::Float }];
        b.outputs = vec![
            PortDef { name: "state".into(), kind: PortKind::Float },
            PortDef { name: "off".into(), kind: PortKind::Float },
            PortDef { name: "on".into(), kind: PortKind::Float },
        ];

        // const(1.0) → guard input
        let s = snap(
            vec![
                block(0, "constant", serde_json::json!({"value": 1.0})),
                b,
            ],
            vec![chan(1, 0, 0, 1, 0)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();

        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(1, 0), Some(1.0)); // state_idx = 1 (on)
        assert_eq!(rt.read_output(1, 1), Some(0.0)); // off inactive
        assert_eq!(rt.read_output(1, 2), Some(1.0)); // on active
    }

    #[test]
    fn skipped_blocks_become_nop() {
        let s = snap(
            vec![
                block(1, "constant", serde_json::json!({"value": 1.0})),
                block(2, "plot", serde_json::json!({})),
            ],
            vec![],
        );
        let rt = DagRuntime::from_snapshot(&s).unwrap();
        // plot is Nop, still counts as a node but produces no outputs
        assert_eq!(rt.node_count(), 2);
        assert_eq!(rt.read_output(2, 0), None);
    }

    #[test]
    fn cycle_detection() {
        let s = snap(
            vec![
                block_with_inputs(1, "gain", serde_json::json!({"param1": 1.0}),
                    vec![("in", PortKind::Float)]),
                block_with_inputs(2, "gain", serde_json::json!({"param1": 1.0}),
                    vec![("in", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 2, 0), chan(2, 2, 0, 1, 0)],
        );
        let err = DagRuntime::from_snapshot(&s).unwrap_err();
        assert!(err.contains("cycle"));
    }

    #[test]
    fn from_json_round_trip() {
        let json = r#"{
            "blocks": [
                {
                    "id": 1,
                    "block_type": "constant",
                    "name": "c",
                    "inputs": [],
                    "outputs": [{"name": "out", "kind": "Float"}],
                    "config": {"value": 99.0},
                    "output_values": []
                },
                {
                    "id": 2,
                    "block_type": "gain",
                    "name": "g",
                    "inputs": [{"name": "in", "kind": "Float"}],
                    "outputs": [{"name": "out", "kind": "Float"}],
                    "config": {"param1": 0.5},
                    "output_values": []
                }
            ],
            "channels": [
                {"id": 1, "from_block": 1, "from_port": 0, "to_block": 2, "to_port": 0}
            ],
            "tick_count": 0,
            "time": 0.0
        }"#;
        let mut rt = DagRuntime::from_json(json).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(2, 0), Some(49.5));
    }

    #[test]
    fn unconnected_inputs_default_zero() {
        // gain with no input connection → input is 0.0 → output is 0.0
        let s = snap(
            vec![block_with_inputs(1, "gain", serde_json::json!({"param1": 5.0}),
                vec![("in", PortKind::Float)])],
            vec![],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(1, 0), Some(0.0));
    }

    #[test]
    fn hardware_bridge_adc_pwm() {
        struct TestHw {
            adc_val: f64,
            last_pwm: Option<(u8, f64)>,
        }
        impl HardwareBridge for TestHw {
            fn adc_read(&self, _ch: u8) -> f64 { self.adc_val }
            fn pwm_write(&mut self, ch: u8, duty: f64) { self.last_pwm = Some((ch, duty)); }
        }

        let mut adc = block(1, "adc_source", serde_json::json!({"channel": 0}));
        adc.inputs = vec![];

        let mut pwm = block(2, "pwm_sink", serde_json::json!({"channel": 3}));
        pwm.inputs = vec![PortDef { name: "duty".into(), kind: PortKind::Float }];
        pwm.outputs = vec![];

        let s = snap(vec![adc, pwm], vec![chan(1, 1, 0, 2, 0)]);
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();

        let mut hw = TestHw { adc_val: 0.75, last_pwm: None };
        rt.tick(&mut hw);

        assert_eq!(rt.read_output(1, 0), Some(0.75));
        assert_eq!(hw.last_pwm, Some((3, 0.75)));
    }

    #[test]
    fn multiply_block() {
        let s = snap(
            vec![
                block(1, "constant", serde_json::json!({"value": 6.0})),
                block(2, "constant", serde_json::json!({"value": 7.0})),
                block_with_inputs(3, "multiply", serde_json::json!({}),
                    vec![("a", PortKind::Float), ("b", PortKind::Float)]),
            ],
            vec![chan(1, 1, 0, 3, 0), chan(2, 2, 0, 3, 1)],
        );
        let mut rt = DagRuntime::from_snapshot(&s).unwrap();
        rt.tick(&mut NullHardware);
        assert_eq!(rt.read_output(3, 0), Some(42.0));
    }

    #[test]
    fn unsupported_block_type_errors() {
        let s = snap(vec![block(1, "quantum_flux", serde_json::json!({}))], vec![]);
        let err = DagRuntime::from_snapshot(&s).unwrap_err();
        assert!(err.contains("unsupported"));
    }
}
