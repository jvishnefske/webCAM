# Flow DSL — PEG Parser Generator for Dataflow Graphs

## Summary

A textual DSL (`.flow` files) for defining RustCAM dataflow graphs, powered by a build-time PEG parser generator (`rust-peg` crate). The parser produces native Rust enum/struct AST types directly — no JSON in the critical path. The DSL is round-trippable: parse text → AST → print text yields identical output.

The block editor remains the primary editing experience. The DSL serves as a readable serialization format with import/export capability.

## Requirements

- **Audience**: Both embedded/controls engineers and developers; also suitable for machine-generated output (Python API, Jupyter)
- **Round-trip**: Parse `.flow` text → typed Rust AST, and serialize AST → `.flow` text, yielding semantically equivalent output (normalized formatting; floats use Rust's `{:?}` round-trippable representation)
- **Progressive expressiveness**: Terse one-liners for simple blocks, structured config blocks for complex types
- **Native Rust types**: Parser actions produce Rust enums/structs directly; AST values can be sent over channels, pattern-matched, passed to codegen without serialization
- **Build-time grammar**: PEG grammar compiled via `peg::parser!{}` proc macro — no runtime grammar interpretation
- **Independence**: DSL crate has no dependency on the dataflow engine; optional bridge is feature-gated

## DSL Syntax

### Block declarations

```
# Simple: positional argument
block sensor: constant(42.0)

# Named parameters (use = separator)
block motor: pwm_sink(channel = 0, frequency = 1000)

# Structured config block (use : separator, brace-delimited)
block ctrl: state_machine {
  initial: idle
  states: [idle, running, error]
  transitions: [
    { from: idle, to: running, guard: 0 },
    { from: running, to: error, guard: 1 },
    { from: error, to: idle }
  ]
}
```

**Syntax note:** Named config uses `type(key = val, ...)` with `=` separators inside parentheses. Structured config uses `type { key: val ... }` with `:` separators inside braces. Both forms support nested lists and maps. The parser accepts single-line or multi-line structured blocks; the printer always normalizes to multi-line.

### Connections

```
sensor.out -> amp.input
amp.out -> display.input
```

### Annotations

```
@target(rp2040)
block sensor: adc(channel = 0)

@target(host)
block viz: plot("Filtered Signal")
```

### Comments

```
# Line comments with hash
```

### Full example

```
@target(rp2040)
block sensor: adc(channel = 0)
block amp: gain(2.5)

@target(host)
block display: plot("Sensor Output")

sensor.out -> amp.input
amp.out -> display.input
```

## Architecture

### Crate structure

```
dsl/
  src/
    lib.rs          # Public API: parse(), serialize()
    ast.rs          # Typed Rust enums/structs
    parser.rs       # peg::parser!{} grammar → AST
    printer.rs      # AST → .flow text (pretty-printer)
    error.rs        # Parse errors with span/line/column info
  Cargo.toml        # deps: peg; optional: serde

# Bridge lives in main crate to avoid circular dependency:
src/dataflow/dsl_bridge.rs  # AST ↔ GraphSnapshot conversion
```

### Data flow

```
                 parse()
  .flow text  ────────────►  Rust AST enums  ──► channel / direct consumption
                                  │
  .flow text  ◄────────────       │  (optional, in main crate)
                print()           ▼
                            dsl_bridge.rs (GraphSnapshot ↔ AST)
```

### AST types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Ident(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Config {
    Empty,
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),       // type(key = val, ...) — parenthesized, = separator
    Structured(Vec<(String, Value)>),  // type { key: val ... } — braced, : separator
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDecl {
    pub id: String,
    pub block_type: String,
    pub config: Config,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub from_block: String,
    pub from_port: String,
    pub to_block: String,
    pub to_port: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub blocks: Vec<BlockDecl>,
    pub connections: Vec<Connection>,
}
```

All AST types derive `Debug`, `Clone`, `PartialEq`. Serde derives are feature-gated behind `serde` feature. Types are `Send + Sync` for channel usage.

**Number handling:** `Value::Int(i64)` for numbers without a decimal point (`0`, `1000`), `Value::Float(f64)` for numbers with a decimal point (`42.0`, `2.5`). The printer outputs integers without a decimal and floats using Rust's `{:?}` format for round-trip fidelity.

**String escapes:** String literals support `\"`, `\\`, `\n`, `\t` escape sequences.

**Value mapping to engine types (in `dsl_bridge.rs`):**
- `Value::Int` / `Value::Float` → engine `Value::Float(f64)`
- `Value::Text` / `Value::Ident` → engine `Value::Text(String)` (idents treated as bare strings)
- `Value::List` / `Value::Map` → serialized to `serde_json::Value` for `BlockSnapshot.config`

### Parser grammar shape

```rust
peg::parser! {
    pub grammar flow_parser() for str {
        rule ident() -> String
        rule float() -> f64
        rule string() -> String
        rule comment() = "#" [^'\n']*

        rule value() -> Value
            = f:float()       { Value::Float(f) }  // must try before int (42.0 vs 42)
            / n:int()         { Value::Int(n) }
            / s:string()      { Value::Text(s) }
            / i:ident()       { Value::Ident(i) }
            / "[" vs:value() ** "," "]" { Value::List(vs) }
            / "{" entries:(ident() ":" value()) ** "," "}" { Value::Map(entries) }

        rule positional() -> Config
        rule named() -> Config
        rule structured() -> Config

        rule annotation() -> Annotation
            = "@" name:ident() "(" args:value() ** "," ")"

        rule block_decl() -> BlockDecl
            = anns:annotation()* "block" id:ident() ":" ty:ident() config:config()?

        rule connection() -> Connection
            = from_b:ident() "." from_p:ident() "->"
              to_b:ident() "." to_p:ident()

        pub rule graph() -> Graph
    }
}
```

### Pretty-printer rules

- One block declaration per line
- Connections grouped after blocks
- Blank line between `@target` groups
- Named params in alphabetical order
- Deterministic: same AST always produces same text
- Implemented as `Display` trait on AST types

## Crate dependencies

- `peg` — build-time parser generator (only required dependency)
- `serde`, `serde_json` — feature-gated behind `serde` feature (for `convert.rs` bridge)
- Dev: `insta` for snapshot tests

### Dependency direction

- `dsl/` depends on: `peg` only (plus optional `serde`)
- `dsl/` does NOT depend on `src/dataflow/` or the main crate
- Main crate depends on `dsl/` (workspace dependency)
- Bridge (`src/dataflow/dsl_bridge.rs`) lives in the main crate, imports both `dsl::ast` types and `dataflow` types — no circular dependency

### Bridge: name-to-index resolution

The `dsl_bridge.rs` module converts between DSL AST and `GraphSnapshot`. Key conversions:
- **Block names → BlockIds**: Allocated sequentially during conversion; a name→id map is maintained
- **Port names → port indices**: Resolved by instantiating each block type and querying `input_ports()` / `output_ports()` to find the index for a given port name
- **DSL uses engine type strings**: Block type names in `.flow` files match the engine's `create_block()` registry exactly (e.g., `adc_source`, `pwm_sink`, `gpio_in`, `uart_rx`)

## Testing strategy

### Layer 1 — Parser unit tests
- Atom parsing: floats, strings, identifiers
- Each config form: positional, named, structured
- Block declarations with/without annotations
- Connections
- Comments and whitespace handling
- Error cases: malformed syntax, missing ports, unclosed braces

### Layer 2 — Round-trip tests
- Parse text → AST → print → re-parse → assert ASTs equal
- Hand-written examples covering all syntax forms

### Layer 3 — AST construction tests
- Verify correct Rust enum variants from parsed input
- Pattern match on `Value::Float`, `Config::Named`, etc.
- Confirm AST types are `Send + Sync`

### Layer 4 — Integration tests
- Parse full graphs, verify block/connection counts
- Optional: AST ↔ `GraphSnapshot` round-trip via `dsl_bridge.rs`
- Snapshot tests with `insta` for printer output

## Scope

### In scope (v1)
- `dsl/` crate with parser, AST types, printer
- Round-trip: parse ↔ print
- All 17+ current block types parseable
- `@target` annotations
- Feature-gated serde derives
- `src/dataflow/dsl_bridge.rs` for AST ↔ GraphSnapshot conversion

### Out of scope (v1)
- Block editor UI integration (import/export buttons)
- WASM exports from main crate
- Python API integration
- Custom block type definitions in the DSL
- Include/import system for composing multiple `.flow` files
- Semantic validation (port existence, type checking)

## Supported block types

All current engine block types: constant, gain, add, multiply, clamp, plot, adc_source, pwm_sink, gpio_in, gpio_out, uart_tx, uart_rx, udp_source, udp_sink, encoder, ssd1306_display, tmc2209_stepper, tmc2209_stallguard, state_machine, pubsub_source, pubsub_sink, json_encode, json_decode, function.

The parser is block-type agnostic — it parses any `block <id>: <type>(...)` declaration. DSL type names must match the engine's `create_block()` registry strings exactly. Block-type-specific validation is out of scope for v1.

## Declaration ordering

The parser accepts blocks and connections in any order (interleaved is fine). The printer normalizes output to: all block declarations first (preserving declaration order), then all connections (ordered by source block, then source port). A blank line separates `@target` groups.
