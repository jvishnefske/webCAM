# Flow DSL — PEG Parser Generator for Dataflow Graphs

## Summary

A textual DSL (`.flow` files) for defining RustCAM dataflow graphs, powered by a build-time PEG parser generator (`rust-peg` crate). The parser produces native Rust enum/struct AST types directly — no JSON in the critical path. The DSL is round-trippable: parse text → AST → print text yields identical output.

The block editor remains the primary editing experience. The DSL serves as a readable serialization format with import/export capability.

## Requirements

- **Audience**: Both embedded/controls engineers and developers; also suitable for machine-generated output (Python API, Jupyter)
- **Round-trip**: Parse `.flow` text → typed Rust AST, and serialize AST → `.flow` text, yielding identical output
- **Progressive expressiveness**: Terse one-liners for simple blocks, structured config blocks for complex types
- **Native Rust types**: Parser actions produce Rust enums/structs directly; AST values can be sent over channels, pattern-matched, passed to codegen without serialization
- **Build-time grammar**: PEG grammar compiled via `peg::parser!{}` proc macro — no runtime grammar interpretation
- **Independence**: DSL crate has no dependency on the dataflow engine; optional bridge is feature-gated

## DSL Syntax

### Block declarations

```
# Simple: positional argument
block sensor: constant(42.0)

# Named parameters
block motor: pwm(channel = 0, frequency = 1000)

# Structured config block
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
    convert.rs      # Optional: AST ↔ GraphSnapshot bridge (feature-gated)
    error.rs        # Parse errors with span info
  Cargo.toml        # deps: peg; optional: serde
```

### Data flow

```
                 parse()
  .flow text  ────────────►  Rust AST enums  ──► channel / direct consumption
                                  │
  .flow text  ◄────────────       │  (optional, feature-gated)
                print()           ▼
                            GraphSnapshot bridge
```

### AST types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
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
    None,
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
    Structured(Vec<(String, Value)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDecl {
    pub id: String,
    pub block_type: String,
    pub config: Config,
    pub annotation: Option<Annotation>,
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

### Parser grammar shape

```rust
peg::parser! {
    pub grammar flow_parser() for str {
        rule ident() -> String
        rule float() -> f64
        rule string() -> String
        rule comment() = "#" [^'\n']*

        rule value() -> Value
            = f:float()       { Value::Float(f) }
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
            = ann:annotation()? "block" id:ident() ":" ty:ident() config:config()?

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

- `dsl/` depends on: `peg` only
- `dsl/` does NOT depend on `src/dataflow/`
- `convert.rs` bridge depends on dataflow types, gated behind `convert` feature
- Main crate can optionally depend on `dsl/` for future WASM exports

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
- Optional: AST ↔ `GraphSnapshot` round-trip via `convert.rs`
- Snapshot tests with `insta` for printer output

## Scope

### In scope (v1)
- `dsl/` crate with parser, AST types, printer
- Round-trip: parse ↔ print
- All 17+ current block types parseable
- `@target` annotations
- Feature-gated serde derives
- Feature-gated `convert.rs` for AST ↔ GraphSnapshot bridge

### Out of scope (v1)
- Block editor UI integration (import/export buttons)
- WASM exports from main crate
- Python API integration
- Custom block type definitions in the DSL
- Include/import system for composing multiple `.flow` files
- Semantic validation (port existence, type checking)

## Supported block types

All current blocks: constant, gain, add, multiply, clamp, plot, adc, pwm, gpio, uart, udp, state_machine, pubsub_source, pubsub_sink, json_encode, json_decode, function.

The parser is block-type agnostic — it parses any `block <id>: <type>(...)` declaration. Block-type-specific validation is out of scope for v1.
