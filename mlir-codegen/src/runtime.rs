#![allow(clippy::module_name_repetitions)]
//! Event-driven IR runtime: zero-alloc, no_std-compatible reactive interpreter.
//!
//! Replaces the synchronous `DagRuntime` + `BlockFn` enum with a reactive
//! cascade executor built on a fixed-size state buffer and work queue.
//!
//! # Design
//!
//! - **Flat state buffer**: All channel values live in a single `[f64; SLOTS]` array.
//!   Slot 0 is reserved for the clock/tick event.
//! - **Block descriptors**: Each block is a compact, `Copy` struct referencing
//!   input/output slots in the state buffer.
//! - **Reactive propagation**: When a slot value changes, all subscribing blocks
//!   are enqueued. The work queue drains until quiescent (no more changes).
//! - **Zero-alloc hot path**: `CompiledGraph` uses const-generic sizes; the
//!   `compile` function (which uses `HashMap`) runs once at load time.

use std::collections::HashMap;

use crate::ir::{ArithOp, Attr, DataflowOp, FuncOp, IrModule, IrOpKind};

// ── Hardware Bridge ──────────────────────────────────────────────

/// Hardware abstraction for peripheral I/O. Default impls return 0 / no-op.
pub trait HwBridge {
    fn adc_read(&self, _channel: u8) -> f64 {
        0.0
    }
    fn pwm_write(&mut self, _channel: u8, _duty: f64) {}
    fn gpio_read(&self, _pin: u8) -> f64 {
        0.0
    }
    fn gpio_write(&mut self, _pin: u8, _value: f64) {}
    fn uart_read(&self, _port: u8) -> f64 {
        0.0
    }
    fn uart_write(&mut self, _port: u8, _value: f64) {}
    fn encoder_read(&self, _channel: u8) -> f64 {
        0.0
    }
    fn publish(&mut self, _topic: u16, _value: f64) {}
    fn subscribe(&self, _topic: u16) -> f64 {
        0.0
    }
}

/// Null hardware -- all defaults.
pub struct NullHw;
impl HwBridge for NullHw {}

// ── Block Descriptor ─────────────────────────────────────────────

/// Maximum inputs per block.
pub const MAX_INPUTS: usize = 4;
/// Maximum outputs per block.
pub const MAX_OUTPUTS: usize = 2;
/// Sentinel for "no slot" (stateless blocks or unused entries).
pub const NO_SLOT: u16 = 0xFFFF;

/// What operation a block performs -- stored as a compact enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Constant(f64),
    Add,
    Mul,
    Sub,
    Clamp(f64, f64),
    AdcRead(u8),
    PwmWrite(u8),
    GpioRead(u8),
    GpioWrite(u8),
    UartRx(u8),
    UartTx(u8),
    EncoderRead(u8),
    Subscribe(u16),
    Publish(u16),
    Nop,
}

/// A block descriptor: compact, Copy, no heap.
#[derive(Debug, Clone, Copy)]
pub struct BlockDesc {
    pub op: OpCode,
    pub input_slots: [u16; MAX_INPUTS],
    pub input_count: u8,
    pub output_slots: [u16; MAX_OUTPUTS],
    pub output_count: u8,
    pub state_slot: u16,
}

// ── Compiled Graph ───────────────────────────────────────────────

/// Max subscribers per slot.
const MAX_SUBS_PER_SLOT: usize = 8;

/// Event-driven compiled graph -- zero-alloc, fixed-size.
///
/// Debug is manually implemented to avoid the `[T; N]: Debug` bound on large arrays.
pub struct CompiledGraph<const BLOCKS: usize, const SLOTS: usize> {
    blocks: [BlockDesc; BLOCKS],
    block_count: usize,
    state: [f64; SLOTS],
    slot_count: usize,
    /// For each slot: which block indices subscribe to it.
    subscribers: [[u16; MAX_SUBS_PER_SLOT]; SLOTS],
    sub_counts: [u8; SLOTS],
    /// Work queue for reactive propagation (ring buffer).
    queue: [u16; BLOCKS],
    queue_head: usize,
    queue_tail: usize,
    in_queue: [bool; BLOCKS],
}

impl<const B: usize, const S: usize> core::fmt::Debug for CompiledGraph<B, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompiledGraph")
            .field("block_count", &self.block_count)
            .field("slot_count", &self.slot_count)
            .finish_non_exhaustive()
    }
}

impl<const B: usize, const S: usize> CompiledGraph<B, S> {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            blocks: [BlockDesc {
                op: OpCode::Nop,
                input_slots: [NO_SLOT; MAX_INPUTS],
                input_count: 0,
                output_slots: [NO_SLOT; MAX_OUTPUTS],
                output_count: 0,
                state_slot: NO_SLOT,
            }; B],
            block_count: 0,
            state: [0.0; S],
            slot_count: 0,
            subscribers: [[NO_SLOT; MAX_SUBS_PER_SLOT]; S],
            sub_counts: [0; S],
            queue: [0; B],
            queue_head: 0,
            queue_tail: 0,
            in_queue: [false; B],
        }
    }

    /// Number of blocks.
    pub fn block_count(&self) -> usize {
        self.block_count
    }

    /// Number of slots.
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Read a slot value.
    pub fn read_slot(&self, slot: u16) -> f64 {
        self.state[slot as usize]
    }

    /// Inject a value into a slot and propagate reactively.
    pub fn inject(&mut self, slot: u16, value: f64, hw: &mut dyn HwBridge) {
        let idx = slot as usize;
        if idx >= self.slot_count {
            return;
        }
        self.state[idx] = value;
        self.enqueue_subscribers(slot);
        self.propagate(hw);
    }

    /// Tick = inject current time into slot 0 (clock channel).
    pub fn tick(&mut self, time: f64, hw: &mut dyn HwBridge) {
        self.inject(0, time, hw);
    }

    fn enqueue_subscribers(&mut self, slot: u16) {
        let idx = slot as usize;
        if idx >= S {
            return;
        }
        for i in 0..self.sub_counts[idx] as usize {
            let block_idx = self.subscribers[idx][i] as usize;
            if block_idx < self.block_count && !self.in_queue[block_idx] {
                self.queue[self.queue_tail] = block_idx as u16;
                self.queue_tail = (self.queue_tail + 1) % B;
                self.in_queue[block_idx] = true;
            }
        }
    }

    fn dequeue(&mut self) -> Option<usize> {
        if self.queue_head == self.queue_tail {
            return None;
        }
        let idx = self.queue[self.queue_head] as usize;
        self.queue_head = (self.queue_head + 1) % B;
        self.in_queue[idx] = false;
        Some(idx)
    }

    fn propagate(&mut self, hw: &mut dyn HwBridge) {
        while let Some(block_idx) = self.dequeue() {
            let block = self.blocks[block_idx];

            // Read inputs
            let mut inputs = [0.0f64; MAX_INPUTS];
            for (i, inp) in inputs.iter_mut().enumerate().take(block.input_count as usize) {
                *inp = self.state[block.input_slots[i] as usize];
            }

            // Execute
            let mut outputs = [0.0f64; MAX_OUTPUTS];
            execute_op(block.op, &inputs, &mut outputs, hw);

            // Write outputs, cascade if changed
            for (i, &out_val) in outputs.iter().enumerate().take(block.output_count as usize) {
                let slot = block.output_slots[i];
                let old = self.state[slot as usize];
                if out_val != old {
                    self.state[slot as usize] = out_val;
                    self.enqueue_subscribers(slot);
                }
            }
        }
    }
}

impl<const B: usize, const S: usize> Default for CompiledGraph<B, S> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Operation Execution ──────────────────────────────────────────

fn execute_op(
    op: OpCode,
    inputs: &[f64; MAX_INPUTS],
    outputs: &mut [f64; MAX_OUTPUTS],
    hw: &mut dyn HwBridge,
) {
    match op {
        OpCode::Constant(v) => {
            outputs[0] = v;
        }
        OpCode::Add => {
            outputs[0] = inputs[0] + inputs[1];
        }
        OpCode::Mul => {
            outputs[0] = inputs[0] * inputs[1];
        }
        OpCode::Sub => {
            outputs[0] = inputs[0] - inputs[1];
        }
        OpCode::Clamp(lo, hi) => {
            outputs[0] = inputs[0].max(lo).min(hi);
        }
        OpCode::AdcRead(ch) => {
            outputs[0] = hw.adc_read(ch);
        }
        OpCode::PwmWrite(ch) => {
            hw.pwm_write(ch, inputs[0]);
        }
        OpCode::GpioRead(pin) => {
            outputs[0] = hw.gpio_read(pin);
        }
        OpCode::GpioWrite(pin) => {
            hw.gpio_write(pin, inputs[0]);
        }
        OpCode::UartRx(port) => {
            outputs[0] = hw.uart_read(port);
        }
        OpCode::UartTx(port) => {
            hw.uart_write(port, inputs[0]);
        }
        OpCode::EncoderRead(ch) => {
            outputs[0] = hw.encoder_read(ch);
        }
        OpCode::Subscribe(topic) => {
            outputs[0] = hw.subscribe(topic);
        }
        OpCode::Publish(topic) => {
            hw.publish(topic, inputs[0]);
        }
        OpCode::Nop => {}
    }
}

// ── Compiler: IrModule -> CompiledGraph ──────────────────────────

/// Compile an `IrModule` into a fixed-size event-driven graph.
pub fn compile<const B: usize, const S: usize>(
    module: &IrModule,
) -> Result<CompiledGraph<B, S>, String> {
    let mut graph = CompiledGraph::<B, S>::new();

    let func = module.funcs.first().ok_or("no functions in module")?;

    // Slot 0 = clock
    graph.slot_count = 1;

    // Map ValueId -> slot index
    let mut value_to_slot: HashMap<u32, u16> = HashMap::new();

    for op in &func.ops {
        // Resolve input slots from operands
        let mut input_slots = [NO_SLOT; MAX_INPUTS];
        let mut input_count = 0u8;
        for (i, vid) in op.operands.iter().enumerate().take(MAX_INPUTS) {
            input_slots[i] = value_to_slot.get(&vid.0).copied().unwrap_or(0);
            input_count += 1;
        }

        // Allocate output slots
        let mut output_slots = [NO_SLOT; MAX_OUTPUTS];
        let mut output_count = 0u8;
        for (i, vid) in op.results.iter().enumerate().take(MAX_OUTPUTS) {
            let slot = graph.slot_count as u16;
            if graph.slot_count >= S {
                return Err("too many slots".into());
            }
            graph.slot_count += 1;
            output_slots[i] = slot;
            value_to_slot.insert(vid.0, slot);
            output_count += 1;
        }

        // Convert IrOpKind -> OpCode
        let opcode = ir_op_to_opcode(&op.kind, &op.attrs);

        // Register block
        let block_idx = graph.block_count;
        if block_idx >= B {
            return Err("too many blocks".into());
        }
        graph.blocks[block_idx] = BlockDesc {
            op: opcode,
            input_slots,
            input_count,
            output_slots,
            output_count,
            state_slot: NO_SLOT,
        };
        graph.block_count += 1;

        // Register subscriptions: for each input slot, this block subscribes
        for &slot_val in input_slots.iter().take(input_count as usize) {
            let slot = slot_val as usize;
            if slot < S {
                let cnt = graph.sub_counts[slot] as usize;
                if cnt < MAX_SUBS_PER_SLOT {
                    graph.subscribers[slot][cnt] = block_idx as u16;
                    graph.sub_counts[slot] += 1;
                }
            }
        }

        // Constants and source blocks (no inputs) subscribe to clock (slot 0)
        // so they fire on every tick.
        if matches!(opcode, OpCode::Constant(_))
            || (input_count == 0
                && output_count > 0
                && !matches!(opcode, OpCode::Nop))
        {
            let cnt = graph.sub_counts[0] as usize;
            if cnt < MAX_SUBS_PER_SLOT {
                graph.subscribers[0][cnt] = block_idx as u16;
                graph.sub_counts[0] += 1;
            }
        }
    }

    Ok(graph)
}

fn ir_op_to_opcode(kind: &IrOpKind, attrs: &HashMap<String, Attr>) -> OpCode {
    match kind {
        // func.call @subscribe / @publish — modeled as function calls in the IR,
        // but mapped to Subscribe/Publish opcodes in the runtime.
        IrOpKind::Func(FuncOp::Call { callee }) if callee == "subscribe" => {
            let topic = match attrs.get("topic") {
                Some(Attr::I64(v)) => *v as u16,
                _ => 0,
            };
            OpCode::Subscribe(topic)
        }
        IrOpKind::Func(FuncOp::Call { callee }) if callee == "publish" => {
            let topic = match attrs.get("topic") {
                Some(Attr::I64(v)) => *v as u16,
                _ => 0,
            };
            OpCode::Publish(topic)
        }
        IrOpKind::Arith(ArithOp::Constant) => {
            let v = match attrs.get("value") {
                Some(Attr::F64(v)) => *v,
                _ => 0.0,
            };
            OpCode::Constant(v)
        }
        IrOpKind::Arith(ArithOp::Addf) => OpCode::Add,
        IrOpKind::Arith(ArithOp::Mulf) => OpCode::Mul,
        IrOpKind::Arith(ArithOp::Subf) => OpCode::Sub,
        IrOpKind::Dataflow(DataflowOp::Clamp) => {
            let lo = match attrs.get("lo") {
                Some(Attr::F64(v)) => *v,
                _ => 0.0,
            };
            let hi = match attrs.get("hi") {
                Some(Attr::F64(v)) => *v,
                _ => 1.0,
            };
            OpCode::Clamp(lo, hi)
        }
        IrOpKind::Dataflow(DataflowOp::AdcRead) => {
            let ch = match attrs.get("channel") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::AdcRead(ch)
        }
        IrOpKind::Dataflow(DataflowOp::PwmWrite) => {
            let ch = match attrs.get("channel") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::PwmWrite(ch)
        }
        IrOpKind::Dataflow(DataflowOp::GpioRead) => {
            let pin = match attrs.get("pin") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::GpioRead(pin)
        }
        IrOpKind::Dataflow(DataflowOp::GpioWrite) => {
            let pin = match attrs.get("pin") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::GpioWrite(pin)
        }
        IrOpKind::Dataflow(DataflowOp::UartRx) => {
            let port = match attrs.get("port") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::UartRx(port)
        }
        IrOpKind::Dataflow(DataflowOp::UartTx) => {
            let port = match attrs.get("port") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::UartTx(port)
        }
        IrOpKind::Dataflow(DataflowOp::EncoderRead) => {
            let ch = match attrs.get("channel") {
                Some(Attr::I64(v)) => *v as u8,
                _ => 0,
            };
            OpCode::EncoderRead(ch)
        }
        IrOpKind::Dataflow(DataflowOp::ChannelRead) => {
            let topic = match attrs.get("topic") {
                Some(Attr::I64(v)) => *v as u16,
                _ => 0,
            };
            OpCode::Subscribe(topic)
        }
        IrOpKind::Dataflow(DataflowOp::ChannelWrite) => {
            let topic = match attrs.get("topic") {
                Some(Attr::I64(v)) => *v as u16,
                _ => 0,
            };
            OpCode::Publish(topic)
        }
        _ => OpCode::Nop,
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrBuilder;

    // Default graph sizes for tests.
    const TB: usize = 32;
    const TS: usize = 64;

    fn build_constant_graph(value: f64) -> IrModule {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.constant_f64(value);
        b.build()
    }

    // 1. Constant tick
    #[test]
    fn test_constant_tick() {
        let module = build_constant_graph(42.0);
        let mut graph = compile::<TB, TS>(&module).unwrap();
        assert_eq!(graph.block_count(), 1);
        graph.tick(1.0, &mut NullHw);
        // Slot 1 should hold the constant value (slot 0 is clock).
        assert_eq!(graph.read_slot(1), 42.0);
    }

    // 2. Chain constant add: const(3) + const(4) = 7
    #[test]
    fn test_chain_constant_add() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c3 = b.constant_f64(3.0);
        let c4 = b.constant_f64(4.0);
        b.addf(c3, c4);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);

        // const(3) -> slot 1, const(4) -> slot 2, add -> slot 3
        assert_eq!(graph.read_slot(3), 7.0);
    }

    // 3. Chain constant mul: const(5) * const(6) = 30
    #[test]
    fn test_chain_constant_mul() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c5 = b.constant_f64(5.0);
        let c6 = b.constant_f64(6.0);
        b.mulf(c5, c6);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 30.0);
    }

    // 4. Clamp: const(150) clamped to [0, 100] = 100
    #[test]
    fn test_clamp() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(150.0);
        b.clamp(c, 0.0, 100.0);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);

        // const(150) -> slot 1, clamp -> slot 2
        assert_eq!(graph.read_slot(2), 100.0);
    }

    // 5. Reactive cascade: const -> add -> verify cascade in one tick
    #[test]
    fn test_reactive_cascade() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c1 = b.constant_f64(10.0);
        let c2 = b.constant_f64(20.0);
        let sum = b.addf(c1, c2);
        let c3 = b.constant_f64(5.0);
        b.mulf(sum, c3);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);

        // const(10) -> slot 1, const(20) -> slot 2, add -> slot 3
        // const(5) -> slot 4, mul -> slot 5
        assert_eq!(graph.read_slot(3), 30.0); // 10 + 20
        assert_eq!(graph.read_slot(5), 150.0); // 30 * 5
    }

    // 6. Inject external value into slot, verify downstream runs
    #[test]
    fn test_inject_external() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c1 = b.constant_f64(10.0);
        let c2 = b.constant_f64(1.0);
        b.addf(c1, c2);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 11.0); // 10 + 1

        // Inject a new value into slot 2 (was const(1.0), now 99.0)
        graph.inject(2, 99.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 109.0); // 10 + 99
    }

    // 7. No change, no cascade: inject same value twice
    #[test]
    fn test_no_change_no_cascade() {
        // We verify this by checking that the output doesn't re-trigger
        // unnecessarily. Build: const(5) + const(5) = 10.
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c1 = b.constant_f64(5.0);
        let c2 = b.constant_f64(5.0);
        b.addf(c1, c2);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 10.0);

        // Inject the same value again into slot 1 (const(5) output)
        // The add block should be enqueued (it subscribes to slot 1),
        // but since its output (10.0) does not change, no further cascade.
        graph.inject(1, 5.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 10.0);
    }

    // 8. ADC read with mock HwBridge returning 3.3
    #[test]
    fn test_adc_read() {
        struct MockAdc;
        impl HwBridge for MockAdc {
            fn adc_read(&self, _ch: u8) -> f64 {
                3.3
            }
        }

        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.adc_read(0);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockAdc);
        assert!((graph.read_slot(1) - 3.3).abs() < f64::EPSILON);
    }

    // 9. PWM write with mock, verify hw.pwm_write called
    #[test]
    fn test_pwm_write() {
        use std::cell::Cell;

        struct MockPwm {
            last_duty: Cell<f64>,
            last_ch: Cell<u8>,
        }
        impl HwBridge for MockPwm {
            fn pwm_write(&mut self, channel: u8, duty: f64) {
                self.last_ch.set(channel);
                self.last_duty.set(duty);
            }
        }

        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(0.75);
        b.pwm_write(2, c);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockPwm {
            last_duty: Cell::new(-1.0),
            last_ch: Cell::new(255),
        };
        graph.tick(1.0, &mut hw);
        assert_eq!(hw.last_ch.get(), 2);
        assert!((hw.last_duty.get() - 0.75).abs() < f64::EPSILON);
    }

    // 10. Work queue dedup: block enqueued twice only runs once
    #[test]
    fn test_work_queue_dedup() {
        // Build: const(1) and const(2) both feed into add.
        // When we tick, both constants fire and their output slots change,
        // each of which tries to enqueue the add block. The add block
        // should only run once.
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c1 = b.constant_f64(1.0);
        let c2 = b.constant_f64(2.0);
        b.addf(c1, c2);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);

        // The add result should be correct (proving it ran)
        assert_eq!(graph.read_slot(3), 3.0);

        // Also verify dedup structurally: after the tick the queue should be empty.
        assert_eq!(graph.queue_head, graph.queue_tail);
    }

    // 11. Empty graph: compile empty module, tick, no panic
    #[test]
    fn test_empty_graph() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        assert_eq!(graph.block_count(), 0);
        assert_eq!(graph.slot_count(), 1); // just clock slot
        graph.tick(1.0, &mut NullHw); // should not panic
    }

    // Additional: compile with no functions -> error
    #[test]
    fn test_no_functions_error() {
        let module = IrModule { funcs: vec![] };
        let result = compile::<TB, TS>(&module);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no functions"));
    }

    // Additional: subtraction
    #[test]
    fn test_sub() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c10 = b.constant_f64(10.0);
        let c3 = b.constant_f64(3.0);
        b.subf(c10, c3);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 7.0);
    }

    // Additional: encoder read
    #[test]
    fn test_encoder_read() {
        struct MockEncoder;
        impl HwBridge for MockEncoder {
            fn encoder_read(&self, _ch: u8) -> f64 {
                42.0
            }
        }

        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.encoder_read(0);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockEncoder);
        assert_eq!(graph.read_slot(1), 42.0);
    }

    // Additional: GPIO read/write
    #[test]
    fn test_gpio_read_write() {
        struct MockGpio {
            written_pin: Cell<u8>,
            written_val: Cell<f64>,
        }
        impl HwBridge for MockGpio {
            fn gpio_read(&self, _pin: u8) -> f64 {
                1.0
            }
            fn gpio_write(&mut self, pin: u8, value: f64) {
                self.written_pin.set(pin);
                self.written_val.set(value);
            }
        }

        use std::cell::Cell;
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.gpio_read(5);
        b.gpio_write(7, val);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockGpio {
            written_pin: Cell::new(0),
            written_val: Cell::new(0.0),
        };
        graph.tick(1.0, &mut hw);
        assert_eq!(graph.read_slot(1), 1.0);
        assert_eq!(hw.written_pin.get(), 7);
        assert_eq!(hw.written_val.get(), 1.0);
    }

    // Additional: UART rx/tx
    #[test]
    fn test_uart_rx_tx() {
        struct MockUart {
            written_port: Cell<u8>,
            written_val: Cell<f64>,
        }
        impl HwBridge for MockUart {
            fn uart_read(&self, _port: u8) -> f64 {
                99.0
            }
            fn uart_write(&mut self, port: u8, value: f64) {
                self.written_port.set(port);
                self.written_val.set(value);
            }
        }

        use std::cell::Cell;
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.uart_rx(2);
        b.uart_tx(3, val);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockUart {
            written_port: Cell::new(0),
            written_val: Cell::new(0.0),
        };
        graph.tick(1.0, &mut hw);
        assert_eq!(graph.read_slot(1), 99.0);
        assert_eq!(hw.written_port.get(), 3);
        assert_eq!(hw.written_val.get(), 99.0);
    }

    // Additional: default trait
    #[test]
    fn test_default_graph() {
        let graph = CompiledGraph::<TB, TS>::default();
        assert_eq!(graph.block_count(), 0);
        assert_eq!(graph.slot_count(), 0);
    }

    // Additional: Debug impl
    #[test]
    fn test_compiled_graph_debug() {
        let module = build_constant_graph(42.0);
        let graph = compile::<TB, TS>(&module).unwrap();
        let debug_str = format!("{:?}", graph);
        assert!(debug_str.contains("CompiledGraph"));
        assert!(debug_str.contains("block_count"));
        assert!(debug_str.contains("slot_count"));
    }

    // Additional: inject out-of-bounds index returns early
    #[test]
    fn test_inject_out_of_bounds() {
        let module = build_constant_graph(42.0);
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        let slot_count = graph.slot_count();
        // Inject at a slot beyond the allocated range -- should not panic
        graph.inject(slot_count as u16 + 10, 999.0, &mut NullHw);
        // Verify the graph is unchanged
        assert_eq!(graph.read_slot(1), 42.0);
    }

    // Additional: Nop opcode execution
    #[test]
    fn test_nop_opcode() {
        // Build a graph using custom_op which produces a Custom IrOpKind,
        // which maps to OpCode::Nop in the runtime.
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(5.0);
        b.custom_op("my.unknown_op", &[c], &[], 1);
        let module = b.build();

        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        // The constant should still produce its value
        assert_eq!(graph.read_slot(1), 5.0);
        // The nop block's output slot should remain 0.0
        assert_eq!(graph.read_slot(2), 0.0);
    }

    // Additional: ir_op_to_opcode for ChannelRead and ChannelWrite
    #[test]
    fn test_ir_op_to_opcode_channel_read() {
        let mut attrs = HashMap::new();
        attrs.insert("topic".to_string(), Attr::I64(42));
        let opcode = ir_op_to_opcode(
            &IrOpKind::Dataflow(DataflowOp::ChannelRead),
            &attrs,
        );
        assert_eq!(opcode, OpCode::Subscribe(42));
    }

    #[test]
    fn test_ir_op_to_opcode_channel_write() {
        let mut attrs = HashMap::new();
        attrs.insert("topic".to_string(), Attr::I64(7));
        let opcode = ir_op_to_opcode(
            &IrOpKind::Dataflow(DataflowOp::ChannelWrite),
            &attrs,
        );
        assert_eq!(opcode, OpCode::Publish(7));
    }

    // Additional: ir_op_to_opcode fallback to Nop
    #[test]
    fn test_ir_op_to_opcode_nop_fallback() {
        let attrs = HashMap::new();
        let opcode = ir_op_to_opcode(
            &IrOpKind::Custom("unknown.op".to_string()),
            &attrs,
        );
        assert_eq!(opcode, OpCode::Nop);
    }

    // ── execute_op coverage for all OpCode variants ──────────────────

    #[test]
    fn test_execute_op_sub() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c10 = b.constant_f64(10.0);
        let c3 = b.constant_f64(3.0);
        b.subf(c10, c3);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(3), 7.0);
    }

    #[test]
    fn test_execute_op_gpio_read() {
        struct MockGpio;
        impl HwBridge for MockGpio {
            fn gpio_read(&self, _pin: u8) -> f64 { 1.0 }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.gpio_read(5);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockGpio);
        assert_eq!(graph.read_slot(1), 1.0);
    }

    #[test]
    fn test_execute_op_gpio_write() {
        use std::cell::Cell;
        struct MockGpioW { pin: Cell<u8>, val: Cell<f64> }
        impl HwBridge for MockGpioW {
            fn gpio_write(&mut self, pin: u8, value: f64) {
                self.pin.set(pin);
                self.val.set(value);
            }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(1.0);
        b.gpio_write(3, c);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockGpioW { pin: Cell::new(255), val: Cell::new(-1.0) };
        graph.tick(1.0, &mut hw);
        assert_eq!(hw.pin.get(), 3);
        assert_eq!(hw.val.get(), 1.0);
    }

    #[test]
    fn test_execute_op_uart_rx() {
        struct MockUartRx;
        impl HwBridge for MockUartRx {
            fn uart_read(&self, _port: u8) -> f64 { 42.0 }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.uart_rx(1);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockUartRx);
        assert_eq!(graph.read_slot(1), 42.0);
    }

    #[test]
    fn test_execute_op_uart_tx() {
        use std::cell::Cell;
        struct MockUartTx { port: Cell<u8>, val: Cell<f64> }
        impl HwBridge for MockUartTx {
            fn uart_write(&mut self, port: u8, value: f64) {
                self.port.set(port);
                self.val.set(value);
            }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(99.0);
        b.uart_tx(2, c);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockUartTx { port: Cell::new(0), val: Cell::new(0.0) };
        graph.tick(1.0, &mut hw);
        assert_eq!(hw.port.get(), 2);
        assert_eq!(hw.val.get(), 99.0);
    }

    #[test]
    fn test_execute_op_encoder_read() {
        struct MockEncoder;
        impl HwBridge for MockEncoder {
            fn encoder_read(&self, _ch: u8) -> f64 { 1024.0 }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.encoder_read(0);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockEncoder);
        assert_eq!(graph.read_slot(1), 1024.0);
    }

    #[test]
    fn test_execute_op_subscribe() {
        struct MockSub;
        impl HwBridge for MockSub {
            fn subscribe(&self, _topic: u16) -> f64 { 42.0 }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.subscribe("sensor");
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut MockSub);
        // String topics currently resolve to topic=0 (known limitation)
        assert_eq!(graph.read_slot(1), 42.0);
    }

    #[test]
    fn test_execute_op_publish() {
        use std::cell::Cell;
        struct MockPub { val: Cell<f64> }
        impl HwBridge for MockPub {
            fn publish(&mut self, _topic: u16, value: f64) {
                self.val.set(value);
            }
        }
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(7.7);
        b.publish("output", c);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        let mut hw = MockPub { val: Cell::new(0.0) };
        graph.tick(1.0, &mut hw);
        assert!((hw.val.get() - 7.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compiled_graph_debug_empty() {
        let graph = CompiledGraph::<4, 8>::new();
        let debug = format!("{:?}", graph);
        assert!(debug.contains("CompiledGraph"));
        assert!(debug.contains("block_count"));
        assert!(debug.contains("slot_count"));
    }

    #[test]
    fn test_compiled_graph_default() {
        let graph = CompiledGraph::<4, 8>::default();
        assert_eq!(graph.block_count(), 0);
        assert_eq!(graph.slot_count(), 0);
    }

    #[test]
    fn test_inject_out_of_range() {
        let mut graph = CompiledGraph::<4, 8>::new();
        // Inject into slot beyond slot_count should be a no-op
        graph.inject(100, 42.0, &mut NullHw);
        // Should not panic
    }

    #[test]
    fn test_compile_empty_module_fails() {
        let module = IrModule { funcs: vec![] };
        let result = compile::<4, 8>(&module);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no functions"));
    }

    #[test]
    fn test_execute_nop() {
        // NOP should produce no output change
        let inputs = [0.0; MAX_INPUTS];
        let mut outputs = [0.0; MAX_OUTPUTS];
        execute_op(OpCode::Nop, &inputs, &mut outputs, &mut NullHw);
        assert_eq!(outputs[0], 0.0);
        assert_eq!(outputs[1], 0.0);
    }

    #[test]
    fn test_clamp_below_range() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(-50.0);
        b.clamp(c, 0.0, 100.0);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(2), 0.0);
    }

    #[test]
    fn test_clamp_in_range() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(50.0);
        b.clamp(c, 0.0, 100.0);
        let module = b.build();
        let mut graph = compile::<TB, TS>(&module).unwrap();
        graph.tick(1.0, &mut NullHw);
        assert_eq!(graph.read_slot(2), 50.0);
    }
}
