# Expression DAG Core — Design Spec

**Date**: 2026-03-26
**Branch**: `feature-embedded-targets`
**Status**: Approved

## Overview

Replace the named-block-with-ports dataflow model with an expression DAG as the new core representation. The DAG is a flat array of `Op` nodes — each referencing inputs by index — inspired by micrograd's expression graph style. The existing block system becomes a thin UI template layer that lowers to `Op` subgraphs.

The microcontroller receives CBOR-encoded DAGs at runtime, builds callable evaluation pipelines via combinators, and binds named I/O channels to physical peripherals. A gzipped WASM+HTML bundle stored on MCU flash is served over USB CDC ECM for the browser-based editor.

## Goals

1. Single core IR (`Dag`) usable across WASM, host, and all MCU targets
2. `no_std` + `no_alloc` capable (heapless on MCU, std in WASM)
3. CBOR wire format — compact, fast to deserialize on constrained devices
4. Runtime-interpreted on MCU — no reflashing to change behavior
5. Named channel I/O binding — DAG is hardware-agnostic
6. Pub/Sub nodes for inter-micro and browser-micro communication
7. Existing blocks become DAG templates (backward compat for visual editor)

## Non-Goals

- Autograd / backward pass (micrograd-style gradient computation)
- JIT compilation on MCU
- Dynamic memory allocation on MCU at evaluation time

## Architecture

### Crate: `dag-core` (new, `no_std`)

```
dag-core/
  Cargo.toml          # no_std, optional std feature, minicbor
  src/
    lib.rs            # Re-exports
    op.rs             # Op enum, NodeId, Dag struct
    eval.rs           # Flat evaluator, ChannelReader/Writer traits
    cbor.rs           # minicbor Encode/Decode impls
    builder.rs        # Programmatic DAG construction helpers
```

### Op Enum

```rust
type NodeId = u16;

enum Op {
    // Sources
    Const(f64),
    Input(ChannelName),

    // Sinks
    Output(ChannelName, NodeId),

    // Binary math
    Add(NodeId, NodeId),
    Mul(NodeId, NodeId),
    Sub(NodeId, NodeId),
    Div(NodeId, NodeId),
    Pow(NodeId, NodeId),

    // Unary
    Neg(NodeId),
    Relu(NodeId),

    // Pub/Sub
    Subscribe(Topic),
    Publish(Topic, NodeId),
}
```

Where `ChannelName` and `Topic` are `heapless::String<32>` on `no_std`, `String` on `std`.

### Dag Struct

```rust
struct Dag {
    nodes: Vec<Op>,
    // Invariant: node i only references nodes j where j < i
    // Topological ordering by construction — no runtime sorting
}
```

`Vec` is `heapless::Vec<Op, MAX_NODES>` on MCU, `std::vec::Vec<Op>` with `std` feature.

### Topological Invariant

Enforced at construction time via the builder API:

```rust
impl Dag {
    fn add_op(&mut self, op: Op) -> Result<NodeId, DagError> {
        // Validate all NodeId references point to j < current length
        // Returns the new node's index
    }
}
```

Invalid references (forward edges, out-of-bounds) are rejected. This guarantees single-pass evaluation.

### Flat Evaluator

```rust
impl Dag {
    fn evaluate(
        &self,
        channels: &dyn ChannelReader,
        pubsub: &dyn PubSubReader,
        values: &mut [f64],
    ) -> EvalResult {
        for (i, op) in self.nodes.iter().enumerate() {
            values[i] = match op {
                Op::Const(v) => *v,
                Op::Input(name) => channels.read(name),
                Op::Add(a, b) => values[*a as usize] + values[*b as usize],
                Op::Mul(a, b) => values[*a as usize] * values[*b as usize],
                Op::Sub(a, b) => values[*a as usize] - values[*b as usize],
                Op::Div(a, b) => values[*a as usize] / values[*b as usize],
                Op::Pow(a, b) => libm::pow(values[*a as usize], values[*b as usize]),
                Op::Neg(a) => -values[*a as usize],
                Op::Relu(a) => values[*a as usize].max(0.0),
                Op::Subscribe(topic) => pubsub.read(topic),
                Op::Output(name, src) => {
                    // Collected into EvalResult
                    values[*src as usize]
                }
                Op::Publish(topic, src) => {
                    // Collected into EvalResult
                    values[*src as usize]
                }
            };
        }
        // Return collected outputs and publishes
    }
}
```

Properties:
- Single pass, O(n)
- No allocation during evaluation (values array pre-allocated)
- Uses `libm` for `pow` on `no_std`

### Channel Traits

```rust
trait ChannelReader {
    fn read(&self, name: &str) -> f64;
}

trait ChannelWriter {
    fn write(&mut self, name: &str, value: f64);
}

trait PubSubReader {
    fn read(&self, topic: &str) -> f64;
}

trait PubSubWriter {
    fn write(&mut self, topic: &str, value: f64);
}
```

On the MCU, these are implemented by the board support crate, mapping names to peripheral HAL calls. On WASM/host, they map to simulated or bridged values.

### CBOR Wire Format

Using `minicbor` (no_std, no_alloc with derives):

```cbor
[                        // Dag: array of ops
  [0, -4.0],             // tag 0 = Const, payload = f64
  [0, 2.0],              // Const(2.0)
  [1, "adc0"],           // tag 1 = Input, payload = channel name
  [3, 0, 1],             // tag 3 = Add(NodeId, NodeId)
  [4, 2, 0],             // tag 4 = Mul(NodeId, NodeId)
  [9, 4],                // tag 9 = Relu(NodeId)
  [2, "pwm0", 5],        // tag 2 = Output(name, NodeId)
]
```

Tag table:
| Tag | Op | Payload |
|-----|-----|---------|
| 0 | Const | f64 |
| 1 | Input | string |
| 2 | Output | string, NodeId |
| 3 | Add | NodeId, NodeId |
| 4 | Mul | NodeId, NodeId |
| 5 | Sub | NodeId, NodeId |
| 6 | Div | NodeId, NodeId |
| 7 | Pow | NodeId, NodeId |
| 8 | Neg | NodeId |
| 9 | Relu | NodeId |
| 10 | Subscribe | string |
| 11 | Publish | string, NodeId |

### Builder API (Combinator Style)

```rust
let mut dag = Dag::new();
let a = dag.constant(-4.0)?;
let b = dag.constant(2.0)?;
let c = dag.add(a, b)?;
let d = dag.mul(a, b)?;
let b_cubed = dag.pow(b, dag.constant(3.0)?)?;
let d = dag.add(d, b_cubed)?;
// ... mirrors the micrograd example
```

Also supports curried/partial application patterns:

```rust
// Create a reusable "scale by factor" combinator
fn scale(dag: &mut Dag, input: NodeId, factor: f64) -> Result<NodeId, DagError> {
    let k = dag.constant(factor)?;
    dag.mul(input, k)
}
```

### Block Compatibility Layer

Existing block types become template functions:

```rust
fn gain_template(dag: &mut Dag, factor: f64) -> BlockTemplate {
    BlockTemplate {
        inputs: vec!["in"],
        outputs: vec!["out"],
        build: |dag, inputs| {
            let k = dag.constant(factor)?;
            let out = dag.mul(inputs["in"], k)?;
            Ok(vec![("out", out)])
        },
    }
}
```

The visual editor works with block templates for UX, lowering to `Op` subgraphs when building the full DAG.

## Micrograd Example — Full Round-Trip

The motivating example:

```python
a = Value(-4.0)
b = Value(2.0)
c = a + b
d = a * b + b**3
c += c + 1
c += 1 + c + (-a)
d += d * 2 + (b + a).relu()
d += 3 * d + (b - a).relu()
e = c - d
f = e**2
g = f / 2.0
g += 10.0 / f
print(f'{g.data:.4f}')  # 24.7041
```

Expressed as a DAG via the builder API:

```rust
let mut dag = Dag::new();
let a = dag.constant(-4.0)?;
let b = dag.constant(2.0)?;
let c0 = dag.add(a, b)?;                           // a + b
let ab = dag.mul(a, b)?;                            // a * b
let three = dag.constant(3.0)?;
let b3 = dag.pow(b, three)?;                        // b**3
let d0 = dag.add(ab, b3)?;                          // a*b + b**3
let one = dag.constant(1.0)?;
let c1 = dag.add(c0, one)?;                         // c + 1
let c1 = dag.add(c0, c1)?;                          // c += c + 1
let neg_a = dag.neg(a)?;                            // -a
let c2 = dag.add(one, c1)?;                         // 1 + c
let c2 = dag.add(c2, neg_a)?;                       // 1 + c + (-a)
let c2 = dag.add(c1, c2)?;                          // c += ...
let two = dag.constant(2.0)?;
let d1 = dag.mul(d0, two)?;                         // d * 2
let ba = dag.add(b, a)?;                            // b + a
let ba_relu = dag.relu(ba)?;                        // (b+a).relu()
let d1 = dag.add(d1, ba_relu)?;                     // d*2 + (b+a).relu()
let d1 = dag.add(d0, d1)?;                          // d += ...
let d2 = dag.mul(three, d1)?;                       // 3 * d  (reuse three)
let bsa = dag.sub(b, a)?;                           // b - a
let bsa_relu = dag.relu(bsa)?;                      // (b-a).relu()
let d2 = dag.add(d2, bsa_relu)?;                    // 3*d + (b-a).relu()
let d2 = dag.add(d1, d2)?;                          // d += ...
let e = dag.sub(c2, d2)?;                           // c - d
let f = dag.pow(e, two)?;                           // e**2 (reuse two)
let half = dag.constant(0.5)?;
let g0 = dag.mul(f, half)?;                         // f / 2.0 as f * 0.5
let ten = dag.constant(10.0)?;
let g1 = dag.div(ten, f)?;                          // 10.0 / f
let g = dag.add(g0, g1)?;                           // final result
// dag.evaluate() on node g should produce 24.7041
```

This DAG serializes to approximately 130 bytes of CBOR.

## Web Frontend + CDC Serving

- Refactor existing `www/` editor to build `Dag` via WASM-exported builder API
- WASM bundle includes DAG builder + CBOR encoder + editor UI
- Bundle is gzipped at build time, embedded as `&[u8]` in MCU firmware
- MCU runs minimal HTTP server over USB CDC ECM
- Serves `index.html`, `app.wasm`, `app.js` with `Content-Encoding: gzip`
- Browser edits DAG, encodes to CBOR, POSTs to MCU, MCU deserializes and starts evaluation loop

## Target MCUs

All existing targets: RP2040, STM32F4, STM32G0B1, ESP32-C3. The `dag-core` crate is target-agnostic; board support crates implement `ChannelReader`/`ChannelWriter` mapping names to HAL peripherals.

## MVP Scope

1. `dag-core` crate: `Op`, `Dag`, builder, evaluator, CBOR encode/decode
2. Micrograd example evaluates to 24.7041
3. Named I/O channels (trait-based, mock impl for tests)
4. Pub/Sub nodes (trait-based, mock impl for tests)
5. Block template compatibility layer (gain, add, constant as examples)
6. WASM bindings for DAG builder + CBOR encoder

## Out of Scope (Future Work)

- CDC ECM HTTP server on MCU
- Gzip embedding pipeline
- Frontend editor refactor
- Full block migration (all 22 types)
- Board support crate ChannelReader impls
- Inter-micro pub/sub transport
