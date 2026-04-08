# I2C Channel + SMBus Read Block

## Overview

Two-layer addition: a `no_std` I2C channel shim in `hil/i2c-hil-sim` that queues `embedded_hal::i2c::I2c` transactions through a fixed-size array, and a single `SmBusRead` configurable block in `configurable-blocks` that lowers to an `IrOpKind::SmBusReadWord` operation. The block has a `periodic` attribute — when set, it re-executes on a tick interval and publishes the result as an integer.

## Layer 1: I2C Channel (hil/i2c-hil-sim)

### Transaction Type

```rust
/// A single I2C transaction request.
#[derive(Clone, Copy)]
pub struct I2cTransaction {
    pub addr: u8,
    pub write_len: u8,
    pub write_buf: [u8; 4],   // SMBus commands are short
    pub read_len: u8,
}

/// Response from executing a transaction.
#[derive(Clone, Copy, Default)]
pub struct I2cResponse {
    pub data: [u8; 4],
    pub len: u8,
    pub ok: bool,
}
```

### Channel

Fixed-size ring buffer. Producer implements `embedded_hal::i2c::I2c` (blocking: enqueue + spin on response). Consumer drains and executes against any `I2c` impl.

```rust
/// Array-backed I2C transaction channel.
///
/// `N` is the queue depth (number of in-flight transactions).
pub struct I2cChannel<const N: usize> {
    requests: [I2cTransaction; N],
    responses: [I2cResponse; N],
    head: usize,   // next write slot (producer)
    tail: usize,   // next read slot (consumer)
    ready: [bool; N],  // response-ready flags
}
```

**Producer API** — implements `embedded_hal::i2c::I2c`:
- `write(addr, data)` → enqueue transaction, wait for response
- `write_read(addr, write, read)` → enqueue, wait, copy response into `read`
- `read(addr, buf)` → enqueue with empty write, wait

**Consumer API**:
- `dequeue(&mut self) -> Option<(usize, I2cTransaction)>` — take next pending transaction
- `complete(&mut self, idx: usize, response: I2cResponse)` — post response for producer

### Where it goes

New module `i2c-hil-sim/src/channel.rs`. Exported from `i2c-hil-sim/src/lib.rs`.

## Layer 2: SMBus Read Block (configurable-blocks)

### Block: `smbus_read`

Single configurable block that reads a 16-bit word via SMBus protocol.

**Config fields:**

| Key | Kind | Default | Description |
|-----|------|---------|-------------|
| `bus` | Int | 0 | I2C bus index |
| `addr` | Int | 0x48 | 7-bit device address |
| `cmd` | Int | 0x00 | SMBus command byte (register pointer) |
| `topic` | Text | `"smbus/result"` | Publish topic for the read value |
| `periodic` | Bool | false | Re-execute on tick interval |
| `interval_ms` | Int | 1000 | Tick interval when periodic=true |

**Category:** I/O

**Declared channels:**
- Output: `{topic}` (PubSub, direction=Output)

**Lowering to DAG:**

```
%0 = SmBusReadWord { bus, addr, cmd }   // HwBridge call, returns u16 as f64
%1 = Publish("{topic}", %0)
```

When `periodic=true`, the block subscribes to slot 0 (tick) and a tick counter + modulo check gates execution to the configured interval.

### IrOpKind Extension

In `mlir-codegen/src/ir.rs`:

```rust
SmBusReadWord { bus: u8, addr: u8, cmd: u8 }
```

### HwBridge Extension

In `mlir-codegen/src/runtime.rs` (the event-driven runtime from the latest spec):

```rust
pub trait HwBridge {
    // ... existing methods ...
    fn smbus_read_word(&self, bus: u8, addr: u8, cmd: u8) -> u16 { 0 }
}
```

### execute_op Handler

```rust
IrOpKind::SmBusReadWord { bus, addr, cmd } => {
    out[0] = hw.smbus_read_word(bus, addr, cmd) as f64;
}
```

### Periodic Attribute in IrOpKind

`periodic` is not a separate IR op. It is handled at the configurable-block level during lowering: the block emits a tick-counter pattern that gates the SMBus read to fire every N ticks.

```
%0 = Subscribe(slot 0)              // tick event
%1 = Const(interval_ticks)          // interval_ms / tick_period
%2 = state_slot                     // tick counter (persisted)
%3 = Add(%2, Const(1.0))            // increment
%4 = Mod(%3, %1)                    // modulo
%5 = store %4 → state_slot          // persist counter
%6 = Eq(%4, Const(0.0))             // fire condition
%7 = Select(%6, SmBusReadWord, previous_value)
%8 = Publish(topic, %7)
```

When `periodic=false`, the block omits the tick/counter/select gating and just does `SmBusReadWord → Publish`.

## Firmware Integration

On the Pico2 (`board-support-pico2`), the `HwBridge` implementation for `smbus_read_word`:

```rust
fn smbus_read_word(&self, bus: u8, addr: u8, cmd: u8) -> u16 {
    // Uses I2cChannel to enqueue: write [cmd], read 2 bytes
    // Returns (buf[0] as u16) << 8 | buf[1] as u16
    // For simulated buses: passes through to RuntimeBus
    // For physical I2C (phase 2): passes through to hardware I2C peripheral
}
```

## File Changes

| File | Change |
|------|--------|
| `hil/i2c-hil-sim/src/channel.rs` | New: I2cTransaction, I2cResponse, I2cChannel |
| `hil/i2c-hil-sim/src/lib.rs` | Add `pub mod channel;` |
| `configurable-blocks/src/blocks/smbus.rs` | New: SmBusReadBlock |
| `configurable-blocks/src/blocks/mod.rs` | Add `pub mod smbus;`, register in palette |
| `mlir-codegen/src/ir.rs` | Add `SmBusReadWord` to IrOpKind |
| `mlir-codegen/src/runtime.rs` | Add `smbus_read_word` to HwBridge, execute_op handler |

## Testing

- `i2c-hil-sim`: channel enqueue/dequeue round-trip, producer blocks until response, full queue behavior
- `configurable-blocks`: SmBusReadBlock lowers to DAG with correct ops, periodic variant includes tick gating, config schema validation
- `mlir-codegen`: SmBusReadWord execute_op returns mock value via NullHardware, periodic fires on correct tick count

## Non-Goals

- General I2C read/write blocks (future)
- Physical I2C pin configuration (phase 2)
- SMBus write operations
- Block read (variable-length) — word only for now
