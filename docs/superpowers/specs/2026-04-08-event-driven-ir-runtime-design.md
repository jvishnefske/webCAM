# Event-Driven IR Runtime

## Overview

Rewrite `mlir-codegen/src/runtime.rs` as a `no_std`, zero-alloc, event-driven interpreter for `IrModule`. Replaces the synchronous `DagRuntime` + `BlockFn` enum with a reactive cascade executor that runs identically on WASM, MCU, and host.

## Core Constraints

- `no_std` compatible, no allocator required
- Fixed-size state buffer (compile-time known size)
- Reactive propagation: events cascade through channels until quiescent
- Bounded execution: fixed-size work queue prevents infinite loops
- Same Rust code on all targets (WASM, Cortex-M, host)

## Data Structures

### State Buffer

All channel values and block state live in a single flat `[f64]` array. Each value occupies one slot. The compiler assigns slot indices when compiling `IrModule` → `CompiledGraph`.

```rust
/// Slot 0 is reserved for the clock/tick event.
/// Slots 1..N are assigned to block outputs and state variables.
state: [f64; MAX_SLOTS]
```

### Block Descriptor

Each block is a fixed-size descriptor referencing slots in the state buffer.

```rust
#[derive(Clone, Copy)]
pub struct BlockDesc {
    pub op: IrOpKind,           // what computation (from ir.rs)
    pub input_slots: [u16; 4],  // indices into state buffer
    pub input_count: u8,        // how many inputs are valid
    pub output_slots: [u16; 2], // indices into state buffer
    pub output_count: u8,       // how many outputs are valid
    pub state_slot: u16,        // 0xFFFF = stateless
}
```

Max 4 inputs, 2 outputs per block. Covers all current block types (add has 2 in / 1 out, encoder has 0 in / 2 out). If a block needs more, chain multiple descriptors.

### Subscriber Table

Maps each slot to the blocks that read from it. When a slot's value changes, those blocks are enqueued for execution.

```rust
/// For each slot, up to MAX_SUBSCRIBERS_PER_SLOT blocks that depend on it.
pub struct SubscriberTable<const SLOTS: usize, const SUBS: usize> {
    table: [[u16; SUBS]; SLOTS],  // block indices, 0xFFFF = empty
    counts: [u8; SLOTS],
}
```

### Work Queue

Fixed-size ring buffer for reactive propagation. Each block can appear at most once (dedup via bitset).

```rust
pub struct WorkQueue<const MAX: usize> {
    buf: [u16; MAX],
    head: usize,
    tail: usize,
    enqueued: [bool; MAX],  // dedup bitset
}
```

### Compiled Graph

The top-level structure produced by compiling an `IrModule`.

```rust
pub struct CompiledGraph<
    const MAX_BLOCKS: usize,
    const MAX_SLOTS: usize,
    const MAX_SUBS: usize,
> {
    blocks: [BlockDesc; MAX_BLOCKS],
    block_count: usize,
    subscribers: SubscriberTable<MAX_SLOTS, MAX_SUBS>,
    state: [f64; MAX_SLOTS],
    slot_count: usize,
    queue: WorkQueue<MAX_BLOCKS>,
}
```

## Execution Model

### Event Injection

External events write a value to a slot and trigger propagation:

```rust
impl CompiledGraph {
    /// Inject a value into a slot and propagate reactively.
    pub fn inject(&mut self, slot: u16, value: f64, hw: &mut dyn HwBridge) {
        self.state[slot as usize] = value;
        self.enqueue_subscribers(slot);
        self.propagate(hw);
    }

    /// Tick = inject current time into slot 0.
    pub fn tick(&mut self, time: f64, hw: &mut dyn HwBridge) {
        self.inject(0, time, hw);
    }
}
```

### Reactive Propagation

```rust
fn propagate(&mut self, hw: &mut dyn HwBridge) {
    while let Some(block_idx) = self.queue.dequeue() {
        let block = &self.blocks[block_idx];
        
        // Read inputs
        let inputs: [f64; 4] = read_slots(&self.state, &block.input_slots, block.input_count);
        
        // Read current state (if stateful)
        let prev_state = if block.state_slot != 0xFFFF {
            self.state[block.state_slot as usize]
        } else {
            0.0
        };
        
        // Execute operation
        let (outputs, new_state) = execute_op(block.op, &inputs, prev_state, hw);
        
        // Write state
        if block.state_slot != 0xFFFF {
            self.state[block.state_slot as usize] = new_state;
        }
        
        // Write outputs, enqueue subscribers if value changed
        for i in 0..block.output_count as usize {
            let slot = block.output_slots[i];
            let old = self.state[slot as usize];
            let new_val = outputs[i];
            if old != new_val {
                self.state[slot as usize] = new_val;
                self.enqueue_subscribers(slot);
            }
        }
    }
}
```

### Operation Execution

`execute_op` is a match on `IrOpKind` — the same enum from `ir.rs`:

```rust
fn execute_op(op: IrOpKind, inputs: &[f64; 4], state: f64, hw: &mut dyn HwBridge) -> ([f64; 2], f64) {
    let mut out = [0.0f64; 2];
    let new_state = state;
    match op {
        IrOpKind::Constant { value } => { out[0] = value; }
        IrOpKind::ArithAdd => { out[0] = inputs[0] + inputs[1]; }
        IrOpKind::ArithMul => { out[0] = inputs[0] * inputs[1]; }
        IrOpKind::ArithSub => { out[0] = inputs[0] - inputs[1]; }
        IrOpKind::Clamp { lo, hi } => { out[0] = inputs[0].max(lo).min(hi); }
        IrOpKind::AdcRead { channel } => { out[0] = hw.adc_read(channel); }
        IrOpKind::PwmWrite { channel } => { hw.pwm_write(channel, inputs[0]); }
        IrOpKind::GpioRead { pin } => { out[0] = hw.gpio_read(pin); }
        IrOpKind::GpioWrite { pin } => { hw.gpio_write(pin, inputs[0]); }
        IrOpKind::Subscribe { topic_slot } => { out[0] = inputs[0]; } // passthrough from injected slot
        IrOpKind::Publish { topic_slot } => { hw.publish(topic_slot, inputs[0]); }
        // ... other ops
    }
    (out, new_state)
}
```

## Hardware Bridge Trait

Simplified from current `HardwareBridge` — same interface, `no_std` compatible:

```rust
pub trait HwBridge {
    fn adc_read(&self, channel: u8) -> f64 { 0.0 }
    fn pwm_write(&mut self, channel: u8, duty: f64) {}
    fn gpio_read(&self, pin: u8) -> f64 { 0.0 }
    fn gpio_write(&mut self, pin: u8, value: f64) {}
    fn publish(&mut self, topic: u16, value: f64) {}
    fn subscribe(&self, topic: u16) -> f64 { 0.0 }
}
```

Note: `topic` is a slot index (u16), not a string. Topic names are resolved at compile time.

## Compilation: IrModule → CompiledGraph

A new function `compile()` in `mlir-codegen/src/runtime.rs`:

1. Walk `IrModule.funcs[0].ops` (the tick function)
2. For each `IrOp`:
   - Assign output slots (incrementing counter)
   - Map `IrOp.name` → `IrOpKind` enum variant
   - Record input operands as slot references
   - Build subscriber entries (input slot → this block)
3. Return `CompiledGraph` with all slots, blocks, and subscriber table populated

## Tick as Event Source

The clock is slot 0. Blocks that need periodic execution subscribe to slot 0:

```
constant(42) → slot 1  (subscribes to slot 0 — runs every tick)
gain(2.0)    → slot 2  (subscribes to slot 1 — runs when constant changes)
pwm_write    → no output (subscribes to slot 2 — runs when gain changes)
```

A single `graph.tick(time)` writes to slot 0, which cascades:
`slot 0 changed → constant runs → slot 1 changed → gain runs → slot 2 changed → pwm_write runs`

## What Gets Removed

- `DagRuntime` struct (replaced by `CompiledGraph`)
- `BlockFn` enum (replaced by `IrOpKind` dispatch in `execute_op`)
- `Node` struct (replaced by `BlockDesc`)
- `HardwareBridge` trait (replaced by simplified `HwBridge`)
- Topic string HashMap (replaced by compile-time slot assignment)

## What Stays

- `IrModule`, `IrBuilder`, `IrOp`, `IrOpKind` (ir.rs) — the IR AST
- `lower_graph_ir()` (lower.rs) — graph snapshot → IR
- `print_mlir()` (printer.rs) — IR → MLIR text for debugging
- `emit_rust()` (emit_rust.rs) — IR → Rust source for AOT compilation

## File Changes

| File | Change |
|------|--------|
| `mlir-codegen/src/runtime.rs` | Complete rewrite: CompiledGraph, BlockDesc, WorkQueue, SubscriberTable, execute_op, compile() |
| `mlir-codegen/src/ir.rs` | Add IrOpKind enum (typed version of string op names) if not already present |
| `mlir-codegen/src/lib.rs` | Update public API: export CompiledGraph, compile(), HwBridge |
| `mlir-codegen/Cargo.toml` | Ensure no_std compatible (may need feature gates) |

## Testing

- Unit test: compile constant → tick → read output slot
- Unit test: compile chain (constant → gain) → tick → verify cascade
- Unit test: reactive propagation only runs downstream blocks
- Unit test: inject external event → verify only affected subgraph runs
- Unit test: work queue dedup — block enqueued twice only runs once
- Unit test: stateful block preserves state across events
- Property test: propagation always terminates (queue bounded by block count)
- Integration test: same CompiledGraph produces identical results as current DataflowGraph.tick()

## Default Sizes

For WASM/host: `MAX_BLOCKS = 256, MAX_SLOTS = 1024, MAX_SUBS = 8`
For MCU: `MAX_BLOCKS = 64, MAX_SLOTS = 256, MAX_SUBS = 4`

These are const generics — the user picks the size at compile time for their target.
