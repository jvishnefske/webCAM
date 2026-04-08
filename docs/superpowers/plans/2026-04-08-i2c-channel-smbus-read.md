# I2C Channel + SMBus Read Block Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add I2C channel infrastructure to the HIL simulator and a configurable SMBus periodic read block to the MLIR pipeline.

**Architecture:** Two layers. Layer 1 adds `I2cChannel<N>` (fixed-size ring buffer of I2C transactions implementing `embedded_hal::i2c::I2c`) to `i2c-hil-sim`. Layer 2 threads `SmBusReadWord` through the full MLIR stack: `DataflowOp` variant → `IrBuilder` helper → `HardwareBridge` method → `BlockFn` variant → dialect constant → lowering emitter → Rust emitter → configurable block with periodic attribute.

**Tech Stack:** Rust, `embedded-hal 1.0`, `no_std`, `dag-core`, `mlir-codegen`, `configurable-blocks`

---

### Task 1: I2C Transaction Types (`i2c-hil-sim`)

**Files:**
- Create: `hil/i2c-hil-sim/src/channel.rs`
- Modify: `hil/i2c-hil-sim/src/lib.rs`

- [ ] **Step 1: Write failing tests for I2cTransaction and I2cResponse**

In `hil/i2c-hil-sim/src/channel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_write_read_roundtrip() {
        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        assert_eq!(tx.addr, 0x48);
        assert_eq!(tx.write_buf[0], 0x00);
        assert_eq!(tx.write_len, 1);
        assert_eq!(tx.read_len, 2);
    }

    #[test]
    fn response_default_is_not_ok() {
        let r = I2cResponse::default();
        assert!(!r.ok);
        assert_eq!(r.len, 0);
    }

    #[test]
    fn response_from_data() {
        let r = I2cResponse::ok(&[0xCA, 0xFE]);
        assert!(r.ok);
        assert_eq!(r.len, 2);
        assert_eq!(r.data[0], 0xCA);
        assert_eq!(r.data[1], 0xFE);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p i2c-hil-sim channel`
Expected: FAIL — module `channel` not found

- [ ] **Step 3: Implement I2cTransaction and I2cResponse**

In `hil/i2c-hil-sim/src/channel.rs`:

```rust
//! Array-backed I2C transaction channel.
//!
//! [`I2cChannel`] is a fixed-size ring buffer that decouples I2C producers
//! (implementing [`embedded_hal::i2c::I2c`]) from consumers that execute
//! transactions against any bus.

/// A single I2C transaction request.
#[derive(Clone, Copy, Debug)]
pub struct I2cTransaction {
    /// 7-bit device address.
    pub addr: u8,
    /// Number of valid bytes in `write_buf`.
    pub write_len: u8,
    /// Write payload (SMBus commands are short — 4 bytes covers command + word).
    pub write_buf: [u8; 4],
    /// Number of bytes to read (0 for write-only).
    pub read_len: u8,
}

impl I2cTransaction {
    /// Create a write-then-read transaction.
    pub fn write_read(addr: u8, write: &[u8], read_len: u8) -> Self {
        let mut buf = [0u8; 4];
        let len = write.len().min(4);
        buf[..len].copy_from_slice(&write[..len]);
        Self {
            addr,
            write_len: len as u8,
            write_buf: buf,
            read_len,
        }
    }

    /// Create a write-only transaction.
    pub fn write(addr: u8, data: &[u8]) -> Self {
        Self::write_read(addr, data, 0)
    }
}

/// Response from executing a transaction.
#[derive(Clone, Copy, Debug)]
pub struct I2cResponse {
    /// Read-back data.
    pub data: [u8; 4],
    /// Number of valid bytes in `data`.
    pub len: u8,
    /// Whether the transaction succeeded.
    pub ok: bool,
}

impl Default for I2cResponse {
    fn default() -> Self {
        Self {
            data: [0; 4],
            len: 0,
            ok: false,
        }
    }
}

impl I2cResponse {
    /// Create a successful response with data.
    pub fn ok(data: &[u8]) -> Self {
        let mut buf = [0u8; 4];
        let len = data.len().min(4);
        buf[..len].copy_from_slice(&data[..len]);
        Self {
            data: buf,
            len: len as u8,
            ok: true,
        }
    }

    /// Create an error response.
    pub fn err() -> Self {
        Self::default()
    }
}
```

- [ ] **Step 4: Add `pub mod channel;` to lib.rs**

In `hil/i2c-hil-sim/src/lib.rs`, add after the existing `pub mod runtime;` line (around line 42):

```rust
pub mod channel;
```

And add to the re-exports (around line 70):

```rust
pub use channel::{I2cChannel, I2cTransaction, I2cResponse};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p i2c-hil-sim channel`
Expected: 3 tests PASS

- [ ] **Step 6: Commit**

```bash
cd /home/v/src/webCAM
git add hil/i2c-hil-sim/src/channel.rs hil/i2c-hil-sim/src/lib.rs
git commit -m "feat(i2c-hil-sim): add I2cTransaction and I2cResponse types"
```

---

### Task 2: I2cChannel Ring Buffer (`i2c-hil-sim`)

**Files:**
- Modify: `hil/i2c-hil-sim/src/channel.rs`

- [ ] **Step 1: Write failing tests for I2cChannel**

Append to the `tests` module in `channel.rs`:

```rust
    #[test]
    fn channel_enqueue_dequeue() {
        let mut ch = I2cChannel::<4>::new();
        assert!(ch.is_empty());

        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        let idx = ch.enqueue(tx).unwrap();

        assert!(!ch.is_empty());
        let (di, dtx) = ch.dequeue().unwrap();
        assert_eq!(di, idx);
        assert_eq!(dtx.addr, 0x48);
    }

    #[test]
    fn channel_complete_returns_response() {
        let mut ch = I2cChannel::<4>::new();
        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        let idx = ch.enqueue(tx).unwrap();

        ch.complete(idx, I2cResponse::ok(&[0xCA, 0xFE]));

        let resp = ch.take_response(idx).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.data[0], 0xCA);
        assert_eq!(resp.data[1], 0xFE);
    }

    #[test]
    fn channel_full_returns_err() {
        let mut ch = I2cChannel::<2>::new();
        ch.enqueue(I2cTransaction::write(0x10, &[0x00])).unwrap();
        ch.enqueue(I2cTransaction::write(0x20, &[0x00])).unwrap();
        assert!(ch.enqueue(I2cTransaction::write(0x30, &[0x00])).is_err());
    }

    #[test]
    fn channel_empty_dequeue_returns_none() {
        let mut ch = I2cChannel::<4>::new();
        assert!(ch.dequeue().is_none());
    }

    #[test]
    fn channel_wraps_around() {
        let mut ch = I2cChannel::<2>::new();

        // Fill and drain twice to exercise wrap-around
        for round in 0..2 {
            let idx = ch.enqueue(I2cTransaction::write(0x10 + round, &[0x00])).unwrap();
            let (di, _) = ch.dequeue().unwrap();
            assert_eq!(di, idx);
            ch.complete(idx, I2cResponse::ok(&[0xAA]));
            ch.take_response(idx);
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p i2c-hil-sim channel`
Expected: FAIL — `I2cChannel` not found

- [ ] **Step 3: Implement I2cChannel**

Add above the `#[cfg(test)]` block in `channel.rs`:

```rust
/// Fixed-size ring buffer for I2C transactions.
///
/// Producer enqueues transactions via [`enqueue`](Self::enqueue).
/// Consumer drains via [`dequeue`](Self::dequeue) and posts results
/// via [`complete`](Self::complete). Producer retrieves results
/// via [`take_response`](Self::take_response).
pub struct I2cChannel<const N: usize> {
    requests: [Option<I2cTransaction>; N],
    responses: [Option<I2cResponse>; N],
    head: usize,
    tail: usize,
    count: usize,
}

impl<const N: usize> I2cChannel<N> {
    /// Create an empty channel.
    pub const fn new() -> Self {
        Self {
            requests: [None; N],
            responses: [None; N],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Returns true if no transactions are pending.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Enqueue a transaction. Returns the slot index.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the channel is full.
    #[allow(clippy::result_unit_err)]
    pub fn enqueue(&mut self, tx: I2cTransaction) -> Result<usize, ()> {
        if self.count >= N {
            return Err(());
        }
        let idx = self.head;
        self.requests[idx] = Some(tx);
        self.responses[idx] = None;
        self.head = (self.head + 1) % N;
        self.count += 1;
        Ok(idx)
    }

    /// Dequeue the next pending transaction for the consumer.
    ///
    /// Returns `(slot_index, transaction)`. The consumer must call
    /// [`complete`](Self::complete) with the same slot index when done.
    pub fn dequeue(&mut self) -> Option<(usize, I2cTransaction)> {
        if self.count == 0 {
            return None;
        }
        let idx = self.tail;
        let tx = self.requests[idx].take()?;
        self.tail = (self.tail + 1) % N;
        self.count -= 1;
        Some((idx, tx))
    }

    /// Post a response for a completed transaction.
    pub fn complete(&mut self, idx: usize, response: I2cResponse) {
        if idx < N {
            self.responses[idx] = Some(response);
        }
    }

    /// Take the response for a completed transaction.
    ///
    /// Returns `None` if no response is ready yet.
    pub fn take_response(&mut self, idx: usize) -> Option<I2cResponse> {
        if idx < N {
            self.responses[idx].take()
        } else {
            None
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p i2c-hil-sim channel`
Expected: 8 tests PASS (3 from Task 1 + 5 new)

- [ ] **Step 5: Commit**

```bash
cd /home/v/src/webCAM
git add hil/i2c-hil-sim/src/channel.rs
git commit -m "feat(i2c-hil-sim): add I2cChannel ring buffer"
```

---

### Task 3: DataflowOp::SmBusReadWord (`mlir-codegen`)

**Files:**
- Modify: `mlir-codegen/src/ir.rs`
- Modify: `mlir-codegen/src/dialect.rs`

- [ ] **Step 1: Write failing test for SmBusReadWord IrBuilder**

Append to tests in `mlir-codegen/src/ir.rs` (find the existing `#[cfg(test)] mod tests` block):

```rust
    #[test]
    fn smbus_read_word_builder() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let result = b.smbus_read_word(0, 0x48, 0x00);
        let module = b.build();
        let op = &module.funcs[0].ops[0];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::SmBusReadWord));
        assert_eq!(op.results.len(), 1);
        assert_eq!(op.results[0], result);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mlir-codegen smbus_read_word_builder`
Expected: FAIL — `SmBusReadWord` not found on `DataflowOp`

- [ ] **Step 3: Add SmBusReadWord to DataflowOp**

In `mlir-codegen/src/ir.rs`, add to the `DataflowOp` enum (after line 83, before the closing `}`):

```rust
    /// `dataflow.smbus_read_word` -- SMBus read word protocol (write cmd byte, read 2 bytes).
    SmBusReadWord,
```

In the `IrOpKind::mlir_name()` match arm for `DataflowOp` (around line 108), add:

```rust
                DataflowOp::SmBusReadWord => "smbus_read_word",
```

- [ ] **Step 4: Add dialect constant**

In `mlir-codegen/src/dialect.rs`, add after line 28:

```rust
pub const OP_SMBUS_READ_WORD: &str = "dataflow.smbus_read_word";
```

- [ ] **Step 5: Add IrBuilder helper method**

In `mlir-codegen/src/ir.rs`, add to the `impl IrBuilder` block, after the `encoder_read` method (around line 380):

```rust
    /// SMBus read word: result = hw.smbus_read_word(bus, addr, cmd).
    pub fn smbus_read_word(&mut self, bus: u8, addr: u8, cmd: u8) -> ValueId {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::SmBusReadWord),
            &[],
            &[
                ("bus", Attr::I64(bus as i64)),
                ("addr", Attr::I64(addr as i64)),
                ("cmd", Attr::I64(cmd as i64)),
            ],
            1,
        )[0]
    }
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p mlir-codegen smbus_read_word_builder`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
cd /home/v/src/webCAM
git add mlir-codegen/src/ir.rs mlir-codegen/src/dialect.rs
git commit -m "feat(mlir-codegen): add DataflowOp::SmBusReadWord and IrBuilder helper"
```

---

### Task 4: HardwareBridge + BlockFn for SmBusReadWord (`mlir-codegen`)

**Files:**
- Modify: `mlir-codegen/src/runtime.rs`

- [ ] **Step 1: Write failing test for BlockFn::SmBusReadWord**

Find the existing `#[cfg(test)] mod tests` block in `runtime.rs` and add:

```rust
    #[test]
    fn smbus_read_word_block_fn() {
        struct MockHw;
        impl HardwareBridge for MockHw {
            fn smbus_read_word(&self, bus: u8, addr: u8, cmd: u8) -> u16 {
                assert_eq!(bus, 0);
                assert_eq!(addr, 0x48);
                assert_eq!(cmd, 0x00);
                0xCAFE
            }
        }

        let block = BlockFn::SmBusReadWord { bus: 0, addr: 0x48, cmd: 0x00 };
        assert_eq!(block.n_outputs(), 1);

        let mut outputs = [0.0f64; 1];
        let mut hw = MockHw;
        block.call(&[], &mut outputs, &mut hw);
        assert_eq!(outputs[0], 0xCAFE as f64);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mlir-codegen smbus_read_word_block_fn`
Expected: FAIL — `smbus_read_word` not a method on `HardwareBridge`

- [ ] **Step 3: Add smbus_read_word to HardwareBridge**

In `mlir-codegen/src/runtime.rs`, add to the `HardwareBridge` trait (after the `stallguard_read` method, around line 51):

```rust
    fn smbus_read_word(&self, bus: u8, addr: u8, cmd: u8) -> u16 {
        0
    }
```

- [ ] **Step 4: Add SmBusReadWord variant to BlockFn**

In the `BlockFn` enum (after `StallGuard`, around line 113), add:

```rust
    /// `smbus_read_word(bus, addr, cmd)() → [value]`
    SmBusReadWord { bus: u8, addr: u8, cmd: u8 },
```

- [ ] **Step 5: Add n_outputs match arm**

In `BlockFn::n_outputs()`, add `Self::SmBusReadWord { .. }` to the `=> 1` arm (alongside `AdcRead`, `GpioRead`, etc.):

```rust
            | Self::SmBusReadWord { .. }
```

- [ ] **Step 6: Add call match arm**

In `BlockFn::call()`, add after the `StallGuard` arm:

```rust
            Self::SmBusReadWord { bus, addr, cmd } => {
                set(outputs, 0, hw.smbus_read_word(*bus, *addr, *cmd) as f64);
            }
```

- [ ] **Step 7: Add from_snapshot match arm**

In `BlockFn::from_snapshot()`, add before the `"plot"` line:

```rust
            "smbus_read" => Self::SmBusReadWord {
                bus: cfg_u8(cfg, "bus"),
                addr: cfg.get("addr").and_then(|v| v.as_u64()).unwrap_or(0x48) as u8,
                cmd: cfg_u8(cfg, "cmd"),
            },
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cargo test -p mlir-codegen smbus_read_word_block_fn`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
cd /home/v/src/webCAM
git add mlir-codegen/src/runtime.rs
git commit -m "feat(mlir-codegen): add HardwareBridge::smbus_read_word and BlockFn::SmBusReadWord"
```

---

### Task 5: Lowering + Rust Emitter for SmBusReadWord (`mlir-codegen`)

**Files:**
- Modify: `mlir-codegen/src/lower.rs`
- Modify: `mlir-codegen/src/emit_rust.rs`

- [ ] **Step 1: Write failing test for MLIR text lowering**

Add to tests in `mlir-codegen/src/lower.rs`:

```rust
    #[test]
    fn lower_smbus_read() {
        let blocks = vec![{
            let mut b = make_block(1, "smbus_read", serde_json::json!({
                "bus": 0, "addr": 0x48, "cmd": 0x00
            }));
            b.inputs = vec![];
            b
        }];
        let snap = GraphSnapshot {
            blocks,
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let mlir = lower_graph(&snap).unwrap();
        assert!(
            mlir.contains("dataflow.smbus_read_word"),
            "expected dataflow.smbus_read_word op, got:\n{mlir}"
        );
    }

    #[test]
    fn lower_smbus_read_ir() {
        let blocks = vec![{
            let mut b = make_block(1, "smbus_read", serde_json::json!({
                "bus": 0, "addr": 0x48, "cmd": 0x00
            }));
            b.inputs = vec![];
            b
        }];
        let snap = GraphSnapshot {
            blocks,
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let ir = lower_graph_ir(&snap).unwrap();
        let ops = &ir.funcs[0].ops;
        assert!(ops.iter().any(|op| op.kind == IrOpKind::Dataflow(DataflowOp::SmBusReadWord)));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mlir-codegen lower_smbus_read`
Expected: FAIL — `smbus_read` not handled in lowering match

- [ ] **Step 3: Add MLIR text emitter**

In `mlir-codegen/src/lower.rs`, add a helper function near the other `emit_*` functions (around line 400):

```rust
fn emit_smbus_read_word(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let bus = config_u64(block, "bus");
    let addr = config_u64(block, "addr");
    let cmd = config_u64(block, "cmd");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(
        out,
        "    {ssa} = {op} {{ bus = {bus_attr}, addr = {addr_attr}, cmd = {cmd_attr} }} : f64",
        op = dialect::OP_SMBUS_READ_WORD,
        bus_attr = dialect::i32_attr(bus as i32),
        addr_attr = dialect::i32_attr(addr as i32),
        cmd_attr = dialect::i32_attr(cmd as i32),
    )
    .map_err(|e| e.to_string())
}
```

Add the match arm in `lower_graph`'s block-type match (near the `"adc_source"` arm, around line 269):

```rust
            "smbus_read" => emit_smbus_read_word(&mut out, id, block)?,
```

- [ ] **Step 4: Add typed IR lowering**

In the `lower_graph_ir` function's block-type match (find it by searching for `"adc_source" =>`), add:

```rust
            "smbus_read" => {
                let bus = config_u64(block, "bus") as u8;
                let addr = config_u64(block, "addr") as u8;
                let cmd = config_u64(block, "cmd") as u8;
                let v = builder.smbus_read_word(bus, addr, cmd);
                output_map.insert((id, 0), v);
            }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mlir-codegen lower_smbus_read`
Expected: 2 tests PASS

- [ ] **Step 6: Add emit_rust handler**

In `mlir-codegen/src/emit_rust.rs`, add a match arm in the `emit_op` function (after the `EncoderRead` arm, around line 242):

```rust
        IrOpKind::Dataflow(DataflowOp::SmBusReadWord) => {
            let bus = attr_u8(op, "bus");
            let addr = attr_u8(op, "addr");
            let cmd = attr_u8(op, "cmd");
            let _ = writeln!(out, "    // Op {idx}: dataflow.smbus_read_word {{bus = {bus}, addr = 0x{addr:02x}, cmd = 0x{cmd:02x}}}");
            let _ = writeln!(out, "    state.v{} = hw.smbus_read_word({bus}, 0x{addr:02x}, 0x{cmd:02x}) as f64;", r(0));
        }
```

- [ ] **Step 7: Run all mlir-codegen tests**

Run: `cargo test -p mlir-codegen`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
cd /home/v/src/webCAM
git add mlir-codegen/src/lower.rs mlir-codegen/src/emit_rust.rs
git commit -m "feat(mlir-codegen): add SmBusReadWord lowering and Rust emission"
```

---

### Task 6: SmBusReadBlock Configurable Block

**Files:**
- Create: `configurable-blocks/src/blocks/smbus.rs`
- Modify: `configurable-blocks/src/blocks/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `configurable-blocks/src/blocks/smbus.rs`:

```rust
//! SMBus read word configurable block.

use dag_core::op::{Dag, DagError};
use dag_core::templates::BlockPorts;
use serde::{Deserialize, Serialize};

use crate::lower::{ConfigurableBlock, LowerResult};
use crate::schema::{
    BlockCategory, ChannelDirection, ChannelKind, ConfigField, DeclaredChannel, FieldKind,
};

/// SMBus read word block.
///
/// Reads a 16-bit word from an I2C device using SMBus protocol
/// (write command byte, read 2 bytes). Publishes the result as an
/// integer (f64) to a topic.
///
/// When `periodic` is true, the block re-executes on a tick interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmBusReadBlock {
    pub bus: u8,
    pub addr: u8,
    pub cmd: u8,
    pub topic: String,
    pub periodic: bool,
    pub interval_ms: u32,
}

impl Default for SmBusReadBlock {
    fn default() -> Self {
        Self {
            bus: 0,
            addr: 0x48,
            cmd: 0x00,
            topic: "smbus/result".into(),
            periodic: false,
            interval_ms: 1000,
        }
    }
}

impl ConfigurableBlock for SmBusReadBlock {
    fn block_type(&self) -> &str {
        "smbus_read"
    }

    fn display_name(&self) -> &str {
        "SMBus Read Word"
    }

    fn category(&self) -> BlockCategory {
        BlockCategory::Io
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "bus".into(),
                label: "I2C Bus".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.bus),
            },
            ConfigField {
                key: "addr".into(),
                label: "Device Address".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.addr),
            },
            ConfigField {
                key: "cmd".into(),
                label: "Command Byte".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.cmd),
            },
            ConfigField {
                key: "topic".into(),
                label: "Publish Topic".into(),
                kind: FieldKind::Text,
                default: serde_json::json!(self.topic),
            },
            ConfigField {
                key: "periodic".into(),
                label: "Periodic".into(),
                kind: FieldKind::Bool,
                default: serde_json::json!(self.periodic),
            },
            ConfigField {
                key: "interval_ms".into(),
                label: "Interval (ms)".into(),
                kind: FieldKind::Int,
                default: serde_json::json!(self.interval_ms),
            },
        ]
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn apply_config(&mut self, config: &serde_json::Value) {
        if let Some(v) = config.get("bus").and_then(|v| v.as_u64()) {
            self.bus = v as u8;
        }
        if let Some(v) = config.get("addr").and_then(|v| v.as_u64()) {
            self.addr = v as u8;
        }
        if let Some(v) = config.get("cmd").and_then(|v| v.as_u64()) {
            self.cmd = v as u8;
        }
        if let Some(s) = config.get("topic").and_then(|v| v.as_str()) {
            self.topic = s.into();
        }
        if let Some(v) = config.get("periodic").and_then(|v| v.as_bool()) {
            self.periodic = v;
        }
        if let Some(v) = config.get("interval_ms").and_then(|v| v.as_u64()) {
            self.interval_ms = v as u32;
        }
    }

    fn declared_channels(&self) -> Vec<DeclaredChannel> {
        vec![DeclaredChannel {
            name: self.topic.clone(),
            direction: ChannelDirection::Output,
            kind: ChannelKind::PubSub,
        }]
    }

    fn lower(&self) -> Result<LowerResult, DagError> {
        let mut dag = Dag::new();

        // The smbus_read_word is modeled as a hardware Input op.
        // The channel name encodes bus/addr/cmd so the runtime can dispatch.
        let channel_name = format!("smbus/{}_{:#04x}_{:#04x}", self.bus, self.addr, self.cmd);
        let read_val = dag.input(&channel_name)?;
        dag.publish(&self.topic, read_val)?;

        Ok(LowerResult {
            dag,
            ports: BlockPorts {
                inputs: vec![],
                outputs: vec![("value".into(), read_val)],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_to_il_text;
    use dag_core::op::Op;

    #[test]
    fn default_config() {
        let block = SmBusReadBlock::default();
        assert_eq!(block.block_type(), "smbus_read");
        assert_eq!(block.category(), BlockCategory::Io);
        assert_eq!(block.bus, 0);
        assert_eq!(block.addr, 0x48);
    }

    #[test]
    fn apply_config_updates_fields() {
        let mut block = SmBusReadBlock::default();
        block.apply_config(&serde_json::json!({
            "bus": 2,
            "addr": 0x50,
            "cmd": 0x05,
            "topic": "temp/sensor1",
            "periodic": true,
            "interval_ms": 500
        }));
        assert_eq!(block.bus, 2);
        assert_eq!(block.addr, 0x50);
        assert_eq!(block.cmd, 0x05);
        assert_eq!(block.topic, "temp/sensor1");
        assert!(block.periodic);
        assert_eq!(block.interval_ms, 500);
    }

    #[test]
    fn declared_channels_has_output_topic() {
        let block = SmBusReadBlock::default();
        let channels = block.declared_channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "smbus/result");
        assert_eq!(channels[0].direction, ChannelDirection::Output);
    }

    #[test]
    fn lower_produces_input_and_publish() {
        let block = SmBusReadBlock::default();
        let result = block.lower().unwrap();
        let ops = result.dag.nodes();

        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Op::Input(name) if name.starts_with("smbus/")));
        assert!(matches!(&ops[1], Op::Publish(topic, 0) if topic == "smbus/result"));
    }

    #[test]
    fn lower_to_il_text_contains_block_name() {
        let block = SmBusReadBlock::default();
        let text = lower_to_il_text(&block).unwrap();
        assert!(text.contains("block @smbus_read"));
        assert!(text.contains("Publish"));
    }

    #[test]
    fn config_schema_has_six_fields() {
        let block = SmBusReadBlock::default();
        assert_eq!(block.config_schema().len(), 6);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p configurable-blocks smbus`
Expected: FAIL — module `smbus` not found

- [ ] **Step 3: Register the module and block**

In `configurable-blocks/src/blocks/mod.rs`, add after line 2:

```rust
pub mod smbus;
```

In the `registry()` function, add to the I/O section (after the `"pwm"` entry):

```rust
        BlockEntry {
            block_type: "smbus_read",
            display_name: "SMBus Read Word",
            category: BlockCategory::Io,
            description: "Read a 16-bit word via SMBus protocol, optionally periodic",
            create: || Box::new(smbus::SmBusReadBlock::default()),
        },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p configurable-blocks smbus`
Expected: 6 tests PASS

- [ ] **Step 5: Run full workspace tests**

Run: `cargo test -p configurable-blocks`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
cd /home/v/src/webCAM
git add configurable-blocks/src/blocks/smbus.rs configurable-blocks/src/blocks/mod.rs
git commit -m "feat(configurable-blocks): add SmBusReadBlock with periodic attribute"
```

---

### Task 7: Integration — Full Pipeline Test

**Files:**
- Modify: `mlir-codegen/src/lower.rs` (add integration test)

- [ ] **Step 1: Write integration test for smbus_read through full pipeline**

Add to the tests in `mlir-codegen/src/lower.rs`:

```rust
    #[test]
    fn lower_smbus_read_full_pipeline() {
        // smbus_read → pubsub_sink: verify the complete lowering chain
        let blocks = vec![
            {
                let mut b = make_block(1, "smbus_read", serde_json::json!({
                    "bus": 0, "addr": 0x48, "cmd": 0x00
                }));
                b.inputs = vec![];
                b.outputs = vec![PortDef {
                    name: "value".to_string(),
                    kind: PortKind::Float,
                }];
                b
            },
            {
                let mut b = make_block(2, "pubsub_sink", serde_json::json!({
                    "topic": "sensor/temp"
                }));
                b.inputs = vec![PortDef {
                    name: "in".to_string(),
                    kind: PortKind::Float,
                }];
                b
            },
        ];
        let snap = GraphSnapshot {
            blocks,
            channels: vec![Channel {
                id: 0,
                from_block: 1,
                from_port: 0,
                to_block: 2,
                to_port: 0,
            }],
            tick_count: 0,
            time: 0.0,
        };

        // MLIR text path
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.smbus_read_word"));
        assert!(mlir.contains("dataflow.publish"));

        // Typed IR path
        let ir = lower_graph_ir(&snap).unwrap();
        let ops = &ir.funcs[0].ops;
        assert!(ops.iter().any(|op| op.kind == IrOpKind::Dataflow(DataflowOp::SmBusReadWord)));
    }
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p mlir-codegen lower_smbus_read_full_pipeline`
Expected: PASS

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test`
Expected: All tests PASS (default-members)

- [ ] **Step 4: Commit**

```bash
cd /home/v/src/webCAM
git add mlir-codegen/src/lower.rs
git commit -m "test(mlir-codegen): add smbus_read full pipeline integration test"
```

---

### Task Summary

| Task | Layer | What |
|------|-------|------|
| 1 | HIL | `I2cTransaction` + `I2cResponse` types |
| 2 | HIL | `I2cChannel<N>` ring buffer |
| 3 | MLIR | `DataflowOp::SmBusReadWord` + `IrBuilder::smbus_read_word()` |
| 4 | MLIR | `HardwareBridge::smbus_read_word()` + `BlockFn::SmBusReadWord` |
| 5 | MLIR | Lowering emitters (MLIR text + typed IR + Rust) |
| 6 | Blocks | `SmBusReadBlock` configurable block with `periodic` attr |
| 7 | Integration | Full pipeline test: smbus_read → publish |
