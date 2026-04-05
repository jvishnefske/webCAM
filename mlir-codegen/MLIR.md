# MLIR Dialect Reference

Typed IR dialect system for the RustCAM dataflow codegen pipeline.

## Architecture

Three parallel IR tiers are generated from the same `GraphSnapshot` input:

```
GraphSnapshot (JSON)
  │
  ├─ Tier 1: String-based MLIR (lower.rs → dialect.rs)
  │    Text concatenation → .mlir file → mlir-opt → mlir-translate → .c/.h
  │    Uses "dataflow.*" string constants for all ops.
  │    Unchanged by the typed IR refactor.
  │
  ├─ Tier 2: Typed IR (ir.rs → emit_rust.rs / printer.rs)
  │    IrBuilder → IrModule { IrFunc { IrOp { kind: IrOpKind, ... } } }
  │    Enum-based dispatch, compile-time exhaustiveness checking.
  │    Emits: safe Rust source code or MLIR text.
  │
  └─ Tier 3: Runtime (runtime.rs)
       BlockFn enum (curried config) → DagRuntime → tick(hw)
       Flat Vec<f64> state buffer, topologically-sorted execution.
       Used for in-MCU and in-browser DAG evaluation.
```

## Dialects

### `arith` — Standard MLIR Arithmetic

Enum: `ArithOp` in `ir.rs`

| Variant | MLIR Name | Operands | Attrs | Results | Description |
|---------|-----------|----------|-------|---------|-------------|
| `Constant` | `arith.constant` | 0 | `value: f64` | 1 (f64) | Compile-time constant |
| `Addf` | `arith.addf` | 2 (f64, f64) | — | 1 (f64) | Floating-point addition |
| `Mulf` | `arith.mulf` | 2 (f64, f64) | — | 1 (f64) | Floating-point multiplication |
| `Subf` | `arith.subf` | 2 (f64, f64) | — | 1 (f64) | Floating-point subtraction |
| `Select` | `arith.select` | 3 (i1, f64, f64) | — | 1 (f64) | Conditional value selection (future) |

These align directly with MLIR's [arith dialect](https://mlir.llvm.org/docs/Dialects/ArithOps/).

### `func` — Standard MLIR Function Calls

Enum: `FuncOp` in `ir.rs`

| Variant | MLIR Name | Operands | Attrs | Results | Description |
|---------|-----------|----------|-------|---------|-------------|
| `Call { callee: "subscribe" }` | `func.call @subscribe` | 0 | `topic: str` | 1 (f64) | Read value from pub/sub topic |
| `Call { callee: "publish" }` | `func.call @publish` | 1 (f64) | `topic: str` | 0 | Write value to pub/sub topic |

Pub/sub is modeled as function symbol invocation rather than custom dialect ops. This aligns with MLIR's `func.call @symbol` pattern and treats pub/sub channels as named external functions with side effects.

**Why function calls?** Subscribe and publish are inter-node communication primitives, not hardware peripheral operations. Modeling them as `func.call` makes the IR composable — a future optimizer can inline, dead-code-eliminate, or replace these calls without special dialect knowledge.

### `dataflow` — Custom Hardware I/O

Enum: `DataflowOp` in `ir.rs`

| Variant | MLIR Name | Operands | Attrs | Results | Description |
|---------|-----------|----------|-------|---------|-------------|
| `Clamp` | `dataflow.clamp` | 1 (f64) | `lo: f64`, `hi: f64` | 1 (f64) | Clamp value to [lo, hi] |
| `AdcRead` | `dataflow.adc_read` | 0 | `channel: i64` | 1 (f64) | Read ADC channel |
| `PwmWrite` | `dataflow.pwm_write` | 1 (f64) | `channel: i64` | 0 | Set PWM duty cycle |
| `GpioRead` | `dataflow.gpio_read` | 0 | `pin: i64` | 1 (f64) | Read GPIO pin |
| `GpioWrite` | `dataflow.gpio_write` | 1 (f64) | `pin: i64` | 0 | Write GPIO pin |
| `UartRx` | `dataflow.uart_rx` | 0 | `port: i64` | 1 (f64) | UART receive |
| `UartTx` | `dataflow.uart_tx` | 1 (f64) | `port: i64` | 0 | UART transmit |
| `EncoderRead` | `dataflow.encoder_read` | 0 | `channel: i64` | 2 (f64, f64) | Read encoder (position, velocity) |

These have no standard MLIR equivalent — they represent direct hardware peripheral access on embedded targets.

### Operations NOT in the IR

The following block types exist in the runtime (`BlockFn` enum) but are **intentionally excluded** from the typed IR:

| Block | Runtime Variant | Why Excluded |
|-------|----------------|-------------|
| Stepper motor | `BlockFn::Stepper(port)` | Direction/pulse is a custom message struct over a channel, not a language-level op |
| StallGuard | `BlockFn::StallGuard { port, addr, threshold }` | Sensor reading over TMC UART — a protocol message, not an IR primitive |
| Display | `BlockFn::DisplayWrite(bus, addr)` | I2C display update — a structured message, not a scalar op |
| State machine | `BlockFn::StateMachine { ... }` | Region-based control flow handled separately in `state_machine.rs` |

These are better modeled as typed messages sent over channels (pub/sub or direct), where the message struct carries direction, position, enable flags, etc. The IR should not need to know about TMC2209 register layouts.

## Type System

| `IrType` | MLIR Text | Size | Description |
|----------|-----------|------|-------------|
| `F64` | `f64` | 8 bytes | All signal values (current default) |
| `I32` | `i32` | 4 bytes | Integer config/counters |
| `I64` | `i64` | 8 bytes | Integer config/addresses |
| `Bool` | `i1` | 1 bit | Guard conditions, enables |
| `Index` | `index` | platform | Loop indices (future) |

Currently all op results are `f64` — the type system is prepared for mixed-type signals but not yet exercised.

## IrBuilder API

```rust
let mut b = IrBuilder::new();
b.begin_func("tick", &[], &[]);

// Arithmetic (arith dialect)
let c = b.constant_f64(5.0);        // ArithOp::Constant
let sum = b.addf(c, c);             // ArithOp::Addf
let prod = b.mulf(c, sum);          // ArithOp::Mulf
let diff = b.subf(prod, c);         // ArithOp::Subf

// Hardware I/O (dataflow dialect)
let adc = b.adc_read(3);            // DataflowOp::AdcRead
b.pwm_write(1, adc);                // DataflowOp::PwmWrite
let pin = b.gpio_read(5);           // DataflowOp::GpioRead
b.gpio_write(7, pin);               // DataflowOp::GpioWrite
let clamped = b.clamp(adc, 0.0, 1.0); // DataflowOp::Clamp

// Pub/Sub (func dialect — function calls)
let val = b.subscribe("sensor/temp");   // FuncOp::Call @subscribe
b.publish("actuator/fan", val);         // FuncOp::Call @publish

// Custom ops (escape hatch)
b.custom_op("my.experimental_op", &[val], &[], 1);  // IrOpKind::Custom

let module = b.build();
```

## Output Formats

### MLIR Text (printer.rs)

```mlir
module {
  func.func @tick() {
    %0 = "arith.constant"() {value = 5.0 : f64} : () -> f64
    %1 = "arith.addf"(%0, %0) : (f64, f64) -> f64
    %2 = "dataflow.adc_read"() {channel = 3 : i64} : () -> f64
    %3 = "func.call @subscribe"() {topic = "sensor/temp"} : () -> f64
    "func.call @publish"(%3) {topic = "actuator/fan"} : (f64) -> ()
    return
  }
}
```

### Safe Rust (emit_rust.rs)

```rust
#![forbid(unsafe_code)]

pub trait HardwareBridge {
    fn adc_read(&self, channel: u8) -> f64 { 0.0 }
    fn pwm_write(&mut self, channel: u8, duty: f64) {}
    fn subscribe(&self, topic: &str) -> f64 { 0.0 }
    fn publish(&mut self, topic: &str, value: f64) {}
    // ... gpio, uart, encoder
}

#[derive(Default)]
pub struct State {
    pub v0: f64,
    pub v1: f64,
    // one field per SSA value
}

pub fn tick(state: &mut State, hw: &mut dyn HardwareBridge) {
    // Op 0: arith.constant {value = 5.0}
    state.v0 = 5.0_f64;
    // Op 1: arith.addf(%0, %0)
    state.v1 = state.v0 + state.v0;
    // ...
}
```

## File Map

| File | Purpose |
|------|---------|
| `ir.rs` | `IrOpKind`, `ArithOp`, `FuncOp`, `DataflowOp` enums; `IrOp`, `IrFunc`, `IrModule` structs; `IrBuilder` |
| `emit_rust.rs` | `IrModule` → safe Rust source (matches on `op.kind`) |
| `printer.rs` | `IrModule` → MLIR text (uses `op.kind.mlir_name()`) |
| `lower.rs` | `GraphSnapshot` → `IrModule` (typed IR) and `.mlir` text (tier 1) |
| `runtime.rs` | `BlockFn` enum, `DagRuntime`, `HardwareBridge` trait |
| `dialect.rs` | String constants for tier 1 text MLIR (unchanged by refactor) |
| `state_machine.rs` | FSM blocks → MLIR region-based control flow |
| `pipeline.rs` | Orchestrate mlir-opt → mlir-translate |
| `peripherals.rs` | Generate safe Rust `State` struct |
