# Flow DSL PEG Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `dsl/` crate that parses `.flow` text into typed Rust AST enums and pretty-prints AST back to text, with round-trip fidelity.

**Architecture:** Standalone `dsl/` workspace crate using `rust-peg` for build-time parser generation. AST types are native Rust enums/structs (`Send + Sync`). A bridge module in the main crate converts between DSL AST and `GraphSnapshot`/`Channel` types.

**Tech Stack:** Rust, `peg` crate (proc-macro PEG parser generator)

**Spec:** `docs/superpowers/specs/2026-03-25-flow-dsl-peg-parser-design.md`

---

## File Structure

```
dsl/                          # New workspace crate
  Cargo.toml                  # deps: peg; optional: serde
  src/
    lib.rs                    # Public API: parse(), serialize()
    ast.rs                    # Value, Config, BlockDecl, Connection, Annotation, Graph
    parser.rs                 # peg::parser!{} grammar producing AST types
    printer.rs                # Display impls for AST → .flow text
    error.rs                  # ParseError with line/column info

src/dataflow/dsl_bridge.rs    # AST ↔ GraphSnapshot conversion (in main crate)
src/dataflow/mod.rs           # Modified: add `pub mod dsl_bridge;`
Cargo.toml                    # Modified: add dsl to workspace members + dependency
```

---

### Task 1: Scaffold `dsl/` Crate and AST Types

**Files:**
- Create: `dsl/Cargo.toml`
- Create: `dsl/src/lib.rs`
- Create: `dsl/src/ast.rs`
- Create: `dsl/src/error.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Write tests for AST types**

Create `dsl/src/ast.rs` with the types and a test module:

```rust
// dsl/src/ast.rs

/// A value in the DSL.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Ident(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

/// An annotation like @target(rp2040).
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Value>,
}

/// Block configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum Config {
    Empty,
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
    Structured(Vec<(String, Value)>),
}

/// A block declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDecl {
    pub id: String,
    pub block_type: String,
    pub config: Config,
    pub annotations: Vec<Annotation>,
}

/// A connection between ports.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub from_block: String,
    pub from_port: String,
    pub to_block: String,
    pub to_port: String,
}

/// A complete dataflow graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub blocks: Vec<BlockDecl>,
    pub connections: Vec<Connection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ast_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Value>();
        assert_send_sync::<Annotation>();
        assert_send_sync::<Config>();
        assert_send_sync::<BlockDecl>();
        assert_send_sync::<Connection>();
        assert_send_sync::<Graph>();
    }

    #[test]
    fn value_equality() {
        assert_eq!(Value::Int(42), Value::Int(42));
        assert_ne!(Value::Int(42), Value::Float(42.0));
        assert_eq!(
            Value::List(vec![Value::Int(1), Value::Int(2)]),
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn config_variants() {
        let empty = Config::Empty;
        let pos = Config::Positional(vec![Value::Float(42.0)]);
        let named = Config::Named(vec![("channel".into(), Value::Int(0))]);
        let structured = Config::Structured(vec![("initial".into(), Value::Ident("idle".into()))]);
        assert_ne!(empty, pos);
        assert_ne!(named, structured);
    }

    #[test]
    fn graph_construction() {
        let g = Graph {
            blocks: vec![BlockDecl {
                id: "c".into(),
                block_type: "constant".into(),
                config: Config::Positional(vec![Value::Float(42.0)]),
                annotations: vec![],
            }],
            connections: vec![Connection {
                from_block: "c".into(),
                from_port: "out".into(),
                to_block: "p".into(),
                to_port: "input".into(),
            }],
        };
        assert_eq!(g.blocks.len(), 1);
        assert_eq!(g.connections.len(), 1);
    }
}
```

- [ ] **Step 2: Create `dsl/Cargo.toml`**

```toml
[package]
name = "dsl"
version = "0.1.0"
edition = "2021"

[features]
default = []
serde = ["dep:serde"]

[dependencies]
peg = "0.8"
serde = { version = "1", features = ["derive"], optional = true }
```

- [ ] **Step 3: Create `dsl/src/error.rs`**

```rust
// dsl/src/error.rs

use std::fmt;

/// A parse error with location information.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub expected: Vec<String>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at line {}:{}: expected {}",
            self.line,
            self.column,
            self.expected.join(" or ")
        )
    }
}

impl std::error::Error for ParseError {}

/// Convert a byte offset in `source` to (line, column), both 1-based.
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_to_line_col_first_char() {
        assert_eq!(offset_to_line_col("hello", 0), (1, 1));
    }

    #[test]
    fn offset_to_line_col_second_line() {
        assert_eq!(offset_to_line_col("ab\ncd", 3), (2, 1));
        assert_eq!(offset_to_line_col("ab\ncd", 4), (2, 2));
    }

    #[test]
    fn parse_error_display() {
        let e = ParseError {
            line: 3,
            column: 5,
            expected: vec!["\"block\"".into(), "identifier".into()],
        };
        assert_eq!(e.to_string(), "parse error at line 3:5: expected \"block\" or identifier");
    }
}
```

- [ ] **Step 4: Create `dsl/src/lib.rs` (stub)**

```rust
// dsl/src/lib.rs

pub mod ast;
pub mod error;
```

- [ ] **Step 5: Add `dsl` to workspace in root `Cargo.toml`**

Append `"dsl"` to the existing `members` list. The current list is `[".", "dataflow-rt", "pubsub"]` — add `"dsl"` to the end:

```toml
[workspace]
members = [".", "dataflow-rt", "pubsub", "dsl"]
```

(If the workspace has additional members like `hil/*` crates, preserve them — just append `"dsl"`.)

- [ ] **Step 6: Run tests to verify**

Run: `cargo test -p dsl`
Expected: All tests pass (ast types, error module)

- [ ] **Step 7: Commit**

```bash
git add dsl/ Cargo.toml
git commit -m "feat(dsl): scaffold crate with AST types and error module"
```

---

### Task 2: Parser — Atoms (Idents, Numbers, Strings)

**Files:**
- Create: `dsl/src/parser.rs`
- Modify: `dsl/src/lib.rs`

- [ ] **Step 1: Write failing tests for atom parsing**

Create `dsl/src/parser.rs` with the grammar and test module. Start with just atom rules:

```rust
// dsl/src/parser.rs

use crate::ast::*;

peg::parser! {
    pub grammar flow_parser() for str {
        rule _() = quiet!{[' ' | '\t']*}
        rule __() = quiet!{[' ' | '\t' | '\n' | '\r']*}

        pub rule ident() -> String
            = s:$(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*) { s.to_string() }

        pub rule float() -> f64
            = s:$(['-']?['0'..='9']+ "." ['0'..='9']+) { s.parse().unwrap() }

        pub rule int() -> i64
            = s:$(['-']?['0'..='9']+) { s.parse().unwrap() }

        pub rule string() -> String
            = "\"" s:string_char()* "\"" { s.into_iter().collect() }

        rule string_char() -> char
            = "\\\"" { '"' }
            / "\\\\" { '\\' }
            / "\\n" { '\n' }
            / "\\t" { '\t' }
            / c:$([^ '"' | '\\']) { c.chars().next().unwrap() }
    }
}

#[cfg(test)]
mod tests {
    use super::flow_parser;

    #[test]
    fn parse_ident() {
        assert_eq!(flow_parser::ident("hello"), Ok("hello".to_string()));
        assert_eq!(flow_parser::ident("rp2040"), Ok("rp2040".to_string()));
        assert_eq!(flow_parser::ident("adc_source"), Ok("adc_source".to_string()));
        assert!(flow_parser::ident("123").is_err());
    }

    #[test]
    fn parse_float() {
        assert_eq!(flow_parser::float("42.0"), Ok(42.0));
        assert_eq!(flow_parser::float("2.5"), Ok(2.5));
        assert_eq!(flow_parser::float("-1.5"), Ok(-1.5));
        assert!(flow_parser::float("42").is_err());
    }

    #[test]
    fn parse_int() {
        assert_eq!(flow_parser::int("42"), Ok(42));
        assert_eq!(flow_parser::int("0"), Ok(0));
        assert_eq!(flow_parser::int("-7"), Ok(-7));
        assert_eq!(flow_parser::int("1000"), Ok(1000));
    }

    #[test]
    fn parse_string() {
        assert_eq!(flow_parser::string("\"hello\""), Ok("hello".to_string()));
        assert_eq!(flow_parser::string("\"Sensor Output\""), Ok("Sensor Output".to_string()));
        assert_eq!(flow_parser::string("\"a\\\"b\""), Ok("a\"b".to_string()));
        assert_eq!(flow_parser::string("\"a\\\\b\""), Ok("a\\b".to_string()));
        assert_eq!(flow_parser::string("\"a\\nb\""), Ok("a\nb".to_string()));
    }
}
```

- [ ] **Step 2: Update `dsl/src/lib.rs`**

```rust
pub mod ast;
pub mod error;
pub mod parser;
```

- [ ] **Step 3: Run tests to verify atoms parse correctly**

Run: `cargo test -p dsl parser::tests`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add dsl/src/parser.rs dsl/src/lib.rs
git commit -m "feat(dsl): add PEG parser atoms — ident, float, int, string"
```

---

### Task 3: Parser — Values and Config Forms

**Files:**
- Modify: `dsl/src/parser.rs`

- [ ] **Step 1: Write failing tests for value and config parsing**

Add to the test module in `dsl/src/parser.rs`:

```rust
    #[test]
    fn parse_value_float() {
        assert_eq!(flow_parser::value("42.0"), Ok(Value::Float(42.0)));
    }

    #[test]
    fn parse_value_int() {
        assert_eq!(flow_parser::value("42"), Ok(Value::Int(42)));
    }

    #[test]
    fn parse_value_string() {
        assert_eq!(flow_parser::value("\"hi\""), Ok(Value::Text("hi".to_string())));
    }

    #[test]
    fn parse_value_ident() {
        assert_eq!(flow_parser::value("idle"), Ok(Value::Ident("idle".to_string())));
    }

    #[test]
    fn parse_value_list() {
        assert_eq!(
            flow_parser::value("[1, 2, 3]"),
            Ok(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]))
        );
    }

    #[test]
    fn parse_value_map() {
        assert_eq!(
            flow_parser::value("{ from: idle, to: running }"),
            Ok(Value::Map(vec![
                ("from".into(), Value::Ident("idle".into())),
                ("to".into(), Value::Ident("running".into())),
            ]))
        );
    }

    #[test]
    fn parse_value_nested_list_in_map() {
        assert_eq!(
            flow_parser::value("{ states: [idle, running] }"),
            Ok(Value::Map(vec![
                ("states".into(), Value::List(vec![
                    Value::Ident("idle".into()),
                    Value::Ident("running".into()),
                ])),
            ]))
        );
    }

    #[test]
    fn parse_config_positional() {
        assert_eq!(
            flow_parser::config("(42.0)"),
            Ok(Config::Positional(vec![Value::Float(42.0)]))
        );
        assert_eq!(
            flow_parser::config("(42.0, \"hello\")"),
            Ok(Config::Positional(vec![Value::Float(42.0), Value::Text("hello".into())]))
        );
    }

    #[test]
    fn parse_config_named() {
        assert_eq!(
            flow_parser::config("(channel = 0, frequency = 1000)"),
            Ok(Config::Named(vec![
                ("channel".into(), Value::Int(0)),
                ("frequency".into(), Value::Int(1000)),
            ]))
        );
    }

    #[test]
    fn parse_config_structured() {
        let input = "{\n  initial: idle\n  states: [idle, running]\n}";
        let result = flow_parser::config(input);
        assert_eq!(
            result,
            Ok(Config::Structured(vec![
                ("initial".into(), Value::Ident("idle".into())),
                ("states".into(), Value::List(vec![
                    Value::Ident("idle".into()),
                    Value::Ident("running".into()),
                ])),
            ]))
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl parser::tests`
Expected: FAIL — `value` and `config` rules don't exist yet

- [ ] **Step 3: Implement value and config grammar rules**

Add to the `flow_parser` grammar in `dsl/src/parser.rs`:

```rust
        pub rule value() -> Value
            = f:float()   { Value::Float(f) }
            / n:int()     { Value::Int(n) }
            / s:string()  { Value::Text(s) }
            / "[" _ vs:(value() ** (_ "," _)) _ "]" { Value::List(vs) }
            / "{" __ entries:(ident() _ ":" _ value()) ++ (_ "," _ / __) __ "}" {
                Value::Map(entries)
            }
            / i:ident()   { Value::Ident(i) }

        rule named_param() -> (String, Value)
            = k:ident() _ "=" _ v:value() { (k, v) }

        rule positional_args() -> Config
            = "(" _ vs:(value() ** (_ "," _)) _ ")" { Config::Positional(vs) }

        rule named_args() -> Config
            = "(" _ ps:(named_param() ** (_ "," _)) _ ")" { Config::Named(ps) }

        rule structured_body() -> Config
            = "{" __ entries:(ident() _ ":" _ value()) ++ (_ "," _ / __) __ "}" {
                Config::Structured(entries)
            }

        pub rule config() -> Config
            = _ c:named_args() { c }
            / _ c:positional_args() { c }
            / _ c:structured_body() { c }
```

Note: `named_args` must be tried before `positional_args` because `(key = val)` would partially match as positional. PEG ordered choice handles this — if `named_args` fails, it backtracks to try `positional_args`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dsl parser::tests`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add dsl/src/parser.rs
git commit -m "feat(dsl): add value and config parsing rules"
```

---

### Task 4: Parser — Block Declarations, Connections, Annotations, Graph

**Files:**
- Modify: `dsl/src/parser.rs`

- [ ] **Step 1: Write failing tests for blocks, connections, annotations, and full graphs**

Add to test module:

```rust
    use crate::ast::*;

    #[test]
    fn parse_connection() {
        assert_eq!(
            flow_parser::connection("sensor.out -> amp.input"),
            Ok(Connection {
                from_block: "sensor".into(),
                from_port: "out".into(),
                to_block: "amp".into(),
                to_port: "input".into(),
            })
        );
    }

    #[test]
    fn parse_annotation() {
        assert_eq!(
            flow_parser::annotation("@target(rp2040)"),
            Ok(Annotation {
                name: "target".into(),
                args: vec![Value::Ident("rp2040".into())],
            })
        );
    }

    #[test]
    fn parse_annotation_multiple_args() {
        assert_eq!(
            flow_parser::annotation("@target(rp2040, 1)"),
            Ok(Annotation {
                name: "target".into(),
                args: vec![Value::Ident("rp2040".into()), Value::Int(1)],
            })
        );
    }

    #[test]
    fn parse_block_decl_simple() {
        assert_eq!(
            flow_parser::block_decl("block sensor: constant(42.0)"),
            Ok(BlockDecl {
                id: "sensor".into(),
                block_type: "constant".into(),
                config: Config::Positional(vec![Value::Float(42.0)]),
                annotations: vec![],
            })
        );
    }

    #[test]
    fn parse_block_decl_no_config() {
        assert_eq!(
            flow_parser::block_decl("block sum: add"),
            Ok(BlockDecl {
                id: "sum".into(),
                block_type: "add".into(),
                config: Config::Empty,
                annotations: vec![],
            })
        );
    }

    #[test]
    fn parse_block_decl_with_annotation() {
        let input = "@target(rp2040)\nblock sensor: adc_source(channel = 0)";
        let result = flow_parser::block_decl(input);
        assert_eq!(
            result,
            Ok(BlockDecl {
                id: "sensor".into(),
                block_type: "adc_source".into(),
                config: Config::Named(vec![("channel".into(), Value::Int(0))]),
                annotations: vec![Annotation {
                    name: "target".into(),
                    args: vec![Value::Ident("rp2040".into())],
                }],
            })
        );
    }

    #[test]
    fn parse_block_decl_structured() {
        let input = "block ctrl: state_machine {\n  initial: idle\n  states: [idle, running]\n}";
        let result = flow_parser::block_decl(input);
        assert!(result.is_ok());
        let b = result.unwrap();
        assert_eq!(b.block_type, "state_machine");
        assert!(matches!(b.config, Config::Structured(_)));
    }

    #[test]
    fn parse_full_graph() {
        let input = "\
@target(rp2040)
block sensor: adc_source(channel = 0)
block amp: gain(2.5)

@target(host)
block display: plot(\"Sensor Output\")

sensor.out -> amp.input
amp.out -> display.input
";
        let result = flow_parser::graph(input);
        assert!(result.is_ok());
        let g = result.unwrap();
        assert_eq!(g.blocks.len(), 3);
        assert_eq!(g.connections.len(), 2);
        assert_eq!(g.blocks[0].annotations.len(), 1);
        assert_eq!(g.blocks[0].annotations[0].name, "target");
    }

    #[test]
    fn parse_graph_interleaved() {
        let input = "\
block a: constant(1.0)
a.out -> b.input
block b: gain(2.0)
";
        let g = flow_parser::graph(input).unwrap();
        assert_eq!(g.blocks.len(), 2);
        assert_eq!(g.connections.len(), 1);
    }

    #[test]
    fn parse_graph_with_comments() {
        let input = "\
# This is a comment
block a: constant(1.0)
# Another comment
a.out -> b.input
block b: gain(2.0)
";
        let g = flow_parser::graph(input).unwrap();
        assert_eq!(g.blocks.len(), 2);
        assert_eq!(g.connections.len(), 1);
    }

    #[test]
    fn parse_error_invalid_syntax() {
        assert!(flow_parser::graph("block : missing_id").is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl parser::tests`
Expected: FAIL — `connection`, `annotation`, `block_decl`, `graph` rules don't exist yet

- [ ] **Step 3: Implement remaining grammar rules**

Add to the `flow_parser` grammar:

```rust
        rule comment() = "#" [^'\n']* ("\n" / ![_])

        rule eol() = _ ("\n" / ![_])

        rule line_sep() = (eol() / _ comment()) ++ ()

        pub rule annotation() -> Annotation
            = "@" name:ident() "(" _ args:(value() ** (_ "," _)) _ ")" {
                Annotation { name, args }
            }

        pub rule connection() -> Connection
            = from_b:ident() "." from_p:ident() _ "->" _ to_b:ident() "." to_p:ident() {
                Connection {
                    from_block: from_b,
                    from_port: from_p,
                    to_block: to_b,
                    to_port: to_p,
                }
            }

        pub rule block_decl() -> BlockDecl
            = anns:(annotation() __)*  "block" _ id:ident() _ ":" _ ty:ident() c:config()? {
                BlockDecl {
                    id,
                    block_type: ty,
                    config: c.unwrap_or(Config::Empty),
                    annotations: anns,
                }
            }

        rule statement() -> Option<(Option<BlockDecl>, Option<Connection>)>
            = b:block_decl() { Some((Some(b), None)) }
            / c:connection() { Some((None, Some(c))) }
            / comment() { None }
            / eol() { None }

        pub rule graph() -> Graph
            = __ stmts:statement() ** __ __ {
                let mut blocks = Vec::new();
                let mut connections = Vec::new();
                for s in stmts.into_iter().flatten() {
                    if let (Some(b), _) = &s { blocks.push(b.clone()); }
                    if let (_, Some(c)) = &s { connections.push(c.clone()); }
                }
                Graph { blocks, connections }
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dsl parser::tests`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add dsl/src/parser.rs
git commit -m "feat(dsl): add block, connection, annotation, and graph parsing"
```

---

### Task 5: Pretty-Printer

**Files:**
- Create: `dsl/src/printer.rs`
- Modify: `dsl/src/lib.rs`

- [ ] **Step 1: Write failing tests for pretty-printing**

Create `dsl/src/printer.rs`:

```rust
// dsl/src/printer.rs

use std::fmt;
use crate::ast::*;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for BlockDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::*;

    #[test]
    fn print_value_int() {
        assert_eq!(Value::Int(42).to_string(), "42");
    }

    #[test]
    fn print_value_float() {
        assert_eq!(Value::Float(42.0).to_string(), "42.0");
        assert_eq!(Value::Float(2.5).to_string(), "2.5");
    }

    #[test]
    fn print_value_text() {
        assert_eq!(Value::Text("hello".into()).to_string(), "\"hello\"");
    }

    #[test]
    fn print_value_text_escapes() {
        assert_eq!(Value::Text("a\"b".into()).to_string(), "\"a\\\"b\"");
        assert_eq!(Value::Text("a\\b".into()).to_string(), "\"a\\\\b\"");
    }

    #[test]
    fn print_value_ident() {
        assert_eq!(Value::Ident("idle".into()).to_string(), "idle");
    }

    #[test]
    fn print_value_list() {
        let v = Value::List(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(v.to_string(), "[1, 2]");
    }

    #[test]
    fn print_value_map() {
        let v = Value::Map(vec![
            ("from".into(), Value::Ident("idle".into())),
            ("to".into(), Value::Ident("running".into())),
        ]);
        assert_eq!(v.to_string(), "{ from: idle, to: running }");
    }

    #[test]
    fn print_connection() {
        let c = Connection {
            from_block: "a".into(),
            from_port: "out".into(),
            to_block: "b".into(),
            to_port: "input".into(),
        };
        assert_eq!(c.to_string(), "a.out -> b.input");
    }

    #[test]
    fn print_block_simple() {
        let b = BlockDecl {
            id: "c".into(),
            block_type: "constant".into(),
            config: Config::Positional(vec![Value::Float(42.0)]),
            annotations: vec![],
        };
        assert_eq!(b.to_string(), "block c: constant(42.0)");
    }

    #[test]
    fn print_block_no_config() {
        let b = BlockDecl {
            id: "sum".into(),
            block_type: "add".into(),
            config: Config::Empty,
            annotations: vec![],
        };
        assert_eq!(b.to_string(), "block sum: add");
    }

    #[test]
    fn print_block_named() {
        let b = BlockDecl {
            id: "m".into(),
            block_type: "pwm_sink".into(),
            config: Config::Named(vec![
                ("channel".into(), Value::Int(0)),
                ("frequency".into(), Value::Int(1000)),
            ]),
            annotations: vec![],
        };
        // Named params are sorted alphabetically
        assert_eq!(b.to_string(), "block m: pwm_sink(channel = 0, frequency = 1000)");
    }

    #[test]
    fn print_block_with_annotation() {
        let b = BlockDecl {
            id: "s".into(),
            block_type: "adc_source".into(),
            config: Config::Named(vec![("channel".into(), Value::Int(0))]),
            annotations: vec![Annotation {
                name: "target".into(),
                args: vec![Value::Ident("rp2040".into())],
            }],
        };
        assert_eq!(b.to_string(), "@target(rp2040)\nblock s: adc_source(channel = 0)");
    }

    #[test]
    fn print_block_structured() {
        let b = BlockDecl {
            id: "ctrl".into(),
            block_type: "state_machine".into(),
            config: Config::Structured(vec![
                ("initial".into(), Value::Ident("idle".into())),
                ("states".into(), Value::List(vec![
                    Value::Ident("idle".into()),
                    Value::Ident("running".into()),
                ])),
            ]),
            annotations: vec![],
        };
        let expected = "block ctrl: state_machine {\n  initial: idle\n  states: [idle, running]\n}";
        assert_eq!(b.to_string(), expected);
    }

    #[test]
    fn print_graph() {
        let g = Graph {
            blocks: vec![
                BlockDecl {
                    id: "a".into(),
                    block_type: "constant".into(),
                    config: Config::Positional(vec![Value::Float(1.0)]),
                    annotations: vec![],
                },
                BlockDecl {
                    id: "b".into(),
                    block_type: "gain".into(),
                    config: Config::Positional(vec![Value::Float(2.0)]),
                    annotations: vec![],
                },
            ],
            connections: vec![Connection {
                from_block: "a".into(),
                from_port: "out".into(),
                to_block: "b".into(),
                to_port: "input".into(),
            }],
        };
        let expected = "block a: constant(1.0)\nblock b: gain(2.0)\n\na.out -> b.input\n";
        assert_eq!(g.to_string(), expected);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dsl printer::tests`
Expected: FAIL — all `todo!()` panics

- [ ] **Step 3: Implement Display traits**

Replace the `todo!()` stubs in `dsl/src/printer.rs`:

```rust
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => {
                // Use Debug format for round-trip fidelity of f64
                write!(f, "{v:?}")
            }
            Value::Text(s) => {
                write!(f, "\"")?;
                for ch in s.chars() {
                    match ch {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        c => write!(f, "{c}")?,
                    }
                }
                write!(f, "\"")
            }
            Value::Ident(s) => write!(f, "{s}"),
            Value::List(vs) => {
                write!(f, "[")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Map(entries) => {
                write!(f, "{{ ")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Config::Empty => Ok(()),
            Config::Positional(vs) => {
                write!(f, "(")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Config::Named(ps) => {
                write!(f, "(")?;
                let mut sorted: Vec<_> = ps.iter().collect();
                sorted.sort_by_key(|(k, _)| k.as_str());
                for (i, (k, v)) in sorted.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{k} = {v}")?;
                }
                write!(f, ")")
            }
            Config::Structured(entries) => {
                writeln!(f, " {{")?;
                for (k, v) in entries {
                    writeln!(f, "  {k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.name)?;
        write!(f, "(")?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{arg}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{} -> {}.{}", self.from_block, self.from_port, self.to_block, self.to_port)
    }
}

impl fmt::Display for BlockDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ann in &self.annotations {
            writeln!(f, "{ann}")?;
        }
        write!(f, "block {}: {}{}", self.id, self.block_type, self.config)
    }
}

impl fmt::Display for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Group blocks: insert blank line when @target annotation changes
        let mut prev_target: Option<String> = None;
        for (i, b) in self.blocks.iter().enumerate() {
            let cur_target = b.annotations.iter()
                .find(|a| a.name == "target")
                .map(|a| format!("{a}"));
            if i > 0 && cur_target != prev_target {
                writeln!(f)?; // blank line between target groups
            }
            writeln!(f, "{b}")?;
            prev_target = cur_target;
        }
        if !self.connections.is_empty() {
            writeln!(f)?;
            for c in &self.connections {
                writeln!(f, "{c}")?;
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Update `dsl/src/lib.rs`**

```rust
pub mod ast;
pub mod error;
pub mod parser;
pub mod printer;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p dsl printer::tests`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add dsl/src/printer.rs dsl/src/lib.rs
git commit -m "feat(dsl): add pretty-printer with Display impls for all AST types"
```

---

### Task 6: Public API and Round-Trip Tests

**Files:**
- Modify: `dsl/src/lib.rs`

- [ ] **Step 1: Write failing round-trip tests**

Add to `dsl/src/lib.rs`:

```rust
pub mod ast;
pub mod error;
pub mod parser;
pub mod printer;

use ast::Graph;
use error::ParseError;

/// Parse a `.flow` text into a Graph AST.
pub fn parse(source: &str) -> Result<Graph, ParseError> {
    parser::flow_parser::graph(source).map_err(|e| {
        let (line, column) = error::offset_to_line_col(source, e.location.offset);
        ParseError {
            line,
            column,
            expected: e.expected.tokens().map(|t| t.to_string()).collect(),
        }
    })
}

/// Serialize a Graph AST to `.flow` text.
pub fn serialize(graph: &Graph) -> String {
    graph.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple_graph() {
        let input = "\
block a: constant(1.0)
block b: gain(2.5)

a.out -> b.input
";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_named_config() {
        let input = "\
block m: pwm_sink(channel = 0, frequency = 1000)
";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_annotations() {
        let input = "\
@target(rp2040)
block s: adc_source(channel = 0)

@target(host)
block p: plot(\"Signal\")

s.out -> p.input
";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_structured_config() {
        let input = "\
block ctrl: state_machine {
  initial: idle
  states: [idle, running]
}
";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_no_config_block() {
        let input = "\
block sum: add
";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn parse_error_has_line_info() {
        let result = parse("block : bad");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.column > 0);
    }

    #[test]
    fn parse_empty_graph() {
        let g = parse("").unwrap();
        assert!(g.blocks.is_empty());
        assert!(g.connections.is_empty());
    }

    #[test]
    fn parse_comments_only() {
        let g = parse("# just a comment\n# another\n").unwrap();
        assert!(g.blocks.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p dsl`
Expected: All pass (if parser + printer are correct). If any round-trip tests fail, debug and fix the parser/printer.

- [ ] **Step 3: Commit**

```bash
git add dsl/src/lib.rs
git commit -m "feat(dsl): add public parse/serialize API with round-trip tests"
```

---

### Task 7: DSL Bridge in Main Crate

**Files:**
- Create: `src/dataflow/dsl_bridge.rs`
- Modify: `src/dataflow/mod.rs`
- Modify: `Cargo.toml` (root, add `dsl` dependency)

- [ ] **Step 1: Add `dsl` dependency to root `Cargo.toml`**

Add under `[dependencies]`:
```toml
dsl = { path = "dsl" }
```

- [ ] **Step 2: Write failing tests for the bridge**

Create `src/dataflow/dsl_bridge.rs`:

```rust
//! Converts between DSL AST types and dataflow GraphSnapshot/Channel types.

use super::block::{BlockId, PortDef, Value as EngineValue};
use super::blocks::create_block;
use super::channel::{Channel, ChannelId};
use super::graph::{BlockSnapshot, GraphSnapshot};

/// Convert a DSL Graph AST to a GraphSnapshot.
pub fn ast_to_snapshot(graph: &dsl::ast::Graph) -> Result<GraphSnapshot, String> {
    let mut blocks = Vec::new();
    let mut name_to_id: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut next_id = 0u32;

    for decl in &graph.blocks {
        let id = next_id;
        next_id += 1;
        name_to_id.insert(decl.id.clone(), id);

        let config_json = dsl_config_to_json(&decl.block_type, &decl.config);
        let block = create_block(&decl.block_type, &config_json)
            .map_err(|e| format!("block '{}': {}", decl.id, e))?;

        // TargetFamily doesn't implement FromStr, so deserialize via serde
        let target = decl.annotations.iter()
            .find(|a| a.name == "target")
            .and_then(|a| a.args.first())
            .and_then(|v| match v {
                dsl::ast::Value::Ident(s) => {
                    serde_json::from_value(serde_json::Value::String(s.clone())).ok()
                }
                _ => None,
            });

        blocks.push(BlockSnapshot {
            id,
            block_type: decl.block_type.clone(),
            name: decl.id.clone(),
            inputs: block.input_ports(),
            outputs: block.output_ports(),
            config: serde_json::from_str(&config_json).unwrap_or_default(),
            output_values: vec![None; block.output_ports().len()],
            target,
        });
    }

    let mut channels = Vec::new();
    let mut ch_id = 0u32;
    for conn in &graph.connections {
        let from_block_id = *name_to_id.get(&conn.from_block)
            .ok_or_else(|| format!("unknown block '{}'", conn.from_block))?;
        let to_block_id = *name_to_id.get(&conn.to_block)
            .ok_or_else(|| format!("unknown block '{}'", conn.to_block))?;

        let from_snap = blocks.iter().find(|b| b.id == from_block_id).unwrap();
        let from_port = from_snap.outputs.iter().position(|p| p.name == conn.from_port)
            .ok_or_else(|| format!("block '{}' has no output port '{}'", conn.from_block, conn.from_port))?;

        let to_snap = blocks.iter().find(|b| b.id == to_block_id).unwrap();
        let to_port = to_snap.inputs.iter().position(|p| p.name == conn.to_port)
            .ok_or_else(|| format!("block '{}' has no input port '{}'", conn.to_block, conn.to_port))?;

        channels.push(Channel {
            id: ChannelId(ch_id),
            from_block: BlockId(from_block_id),
            from_port,
            to_block: BlockId(to_block_id),
            to_port,
        });
        ch_id += 1;
    }

    Ok(GraphSnapshot {
        blocks,
        channels,
        tick_count: 0,
        time: 0.0,
    })
}

/// Convert a DSL Config to a JSON string for create_block().
/// For positional configs, uses the block_type to determine the correct JSON key.
fn dsl_config_to_json(block_type: &str, config: &dsl::ast::Config) -> String {
    match config {
        dsl::ast::Config::Empty => "{}".to_string(),
        dsl::ast::Config::Positional(vals) => {
            positional_to_json(block_type, vals)
        }
        dsl::ast::Config::Named(pairs) | dsl::ast::Config::Structured(pairs) => {
            let map: serde_json::Map<String, serde_json::Value> = pairs.iter()
                .map(|(k, v)| (k.clone(), dsl_value_to_json(v)))
                .collect();
            serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
        }
    }
}

/// Map positional args to JSON keys based on block type.
/// Each block type defines which JSON keys its positional args map to.
fn positional_to_json(block_type: &str, vals: &[dsl::ast::Value]) -> String {
    // Mapping of block_type -> ordered list of JSON key names for positional args
    let keys: &[&str] = match block_type {
        "constant" => &["value"],
        "gain" => &["factor"],
        "clamp" => &["min", "max"],
        "plot" => &["label"],
        "adc_source" => &["channel"],
        "pwm_sink" => &["channel"],
        "gpio_in" | "gpio_out" => &["pin"],
        "uart_tx" | "uart_rx" => &["baud"],
        "udp_source" | "udp_sink" => &["port"],
        "pubsub_source" | "pubsub_sink" => &["topic"],
        _ => &[],
    };

    let mut map = serde_json::Map::new();
    for (i, val) in vals.iter().enumerate() {
        let key = if i < keys.len() {
            keys[i].to_string()
        } else {
            format!("arg{i}")
        };
        map.insert(key, dsl_value_to_json(val));
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

fn dsl_value_to_json(val: &dsl::ast::Value) -> serde_json::Value {
    match val {
        dsl::ast::Value::Int(n) => serde_json::Value::Number((*n).into()),
        dsl::ast::Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        dsl::ast::Value::Text(s) | dsl::ast::Value::Ident(s) => {
            serde_json::Value::String(s.clone())
        }
        dsl::ast::Value::List(vs) => {
            serde_json::Value::Array(vs.iter().map(dsl_value_to_json).collect())
        }
        dsl::ast::Value::Map(entries) => {
            let map: serde_json::Map<String, serde_json::Value> = entries.iter()
                .map(|(k, v)| (k.clone(), dsl_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_simple_graph() {
        let graph = dsl::parse("block c: constant(42.0)\nblock g: gain(2.5)\nc.out -> g.input\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert_eq!(snapshot.blocks.len(), 2);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.blocks[0].block_type, "constant");
        assert_eq!(snapshot.blocks[1].block_type, "gain");
    }

    #[test]
    fn bridge_resolves_port_names() {
        let graph = dsl::parse("block c: constant(1.0)\nblock g: gain(2.0)\nc.out -> g.input\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        let ch = &snapshot.channels[0];
        assert_eq!(ch.from_port, 0); // constant has one output: "out" at index 0
        assert_eq!(ch.to_port, 0);   // gain has one input: "input" at index 0
    }

    #[test]
    fn bridge_unknown_block_name_errors() {
        let graph = dsl::parse("block c: constant(1.0)\nx.out -> c.input\n").unwrap();
        let result = ast_to_snapshot(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown block 'x'"));
    }

    #[test]
    fn bridge_unknown_port_name_errors() {
        let graph = dsl::parse("block c: constant(1.0)\nblock g: gain(2.0)\nc.nonexistent -> g.input\n").unwrap();
        let result = ast_to_snapshot(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no output port"));
    }

    #[test]
    fn bridge_with_target_annotation() {
        let input = "@target(rp2040)\nblock s: adc_source(channel = 0)\n";
        let graph = dsl::parse(input).unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert!(snapshot.blocks[0].target.is_some());
    }

    #[test]
    fn bridge_no_config_block() {
        let graph = dsl::parse("block sum: add\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert_eq!(snapshot.blocks[0].block_type, "add");
    }

    #[test]
    fn dsl_config_to_json_named() {
        let config = dsl::ast::Config::Named(vec![
            ("channel".into(), dsl::ast::Value::Int(0)),
            ("frequency".into(), dsl::ast::Value::Int(1000)),
        ]);
        let json: serde_json::Value = serde_json::from_str(&dsl_config_to_json(&config)).unwrap();
        assert_eq!(json["channel"], 0);
        assert_eq!(json["frequency"], 1000);
    }
}
```

- [ ] **Step 3: Add `pub mod dsl_bridge;` to `src/dataflow/mod.rs`**

Find the module declarations in `src/dataflow/mod.rs` and add:
```rust
pub mod dsl_bridge;
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test dsl_bridge`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/dataflow/dsl_bridge.rs src/dataflow/mod.rs Cargo.toml
git commit -m "feat(dsl): add bridge converting DSL AST to GraphSnapshot"
```

---

### Task 8: Final Integration Test — Full Pipeline

**Files:**
- Create: `dsl/tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
// dsl/tests/integration.rs

#[test]
fn parse_serialize_full_example() {
    let input = "\
@target(rp2040)
block sensor: adc_source(channel = 0)
block amp: gain(2.5)

@target(host)
block display: plot(\"Sensor Output\")

sensor.out -> amp.input
amp.out -> display.input
";
    let graph = dsl::parse(input).unwrap();

    assert_eq!(graph.blocks.len(), 3);
    assert_eq!(graph.connections.len(), 2);

    // Verify block details
    assert_eq!(graph.blocks[0].id, "sensor");
    assert_eq!(graph.blocks[0].block_type, "adc_source");
    assert_eq!(graph.blocks[0].annotations.len(), 1);
    assert_eq!(graph.blocks[0].annotations[0].name, "target");

    assert_eq!(graph.blocks[2].id, "display");
    assert_eq!(graph.blocks[2].block_type, "plot");

    // Verify connections
    assert_eq!(graph.connections[0].from_block, "sensor");
    assert_eq!(graph.connections[0].to_block, "amp");

    // Round-trip
    let output = dsl::serialize(&graph);
    let graph2 = dsl::parse(&output).unwrap();
    assert_eq!(graph, graph2);
}

#[test]
fn parse_state_machine_structured_config() {
    let input = "\
block ctrl: state_machine {
  initial: idle
  states: [idle, running, error]
  transitions: [{ from: idle, to: running, guard: 0 }, { from: running, to: error, guard: 1 }]
}
";
    let graph = dsl::parse(input).unwrap();
    assert_eq!(graph.blocks.len(), 1);

    let config = &graph.blocks[0].config;
    match config {
        dsl::ast::Config::Structured(entries) => {
            assert_eq!(entries[0].0, "initial");
            assert_eq!(entries[0].1, dsl::ast::Value::Ident("idle".into()));
            assert_eq!(entries.len(), 3);
        }
        _ => panic!("expected structured config"),
    }
}

#[test]
fn parse_all_config_forms() {
    let input = "\
block a: constant(42.0)
block b: pwm_sink(channel = 0, frequency = 1000)
block c: add
block d: state_machine {
  initial: idle
}
";
    let graph = dsl::parse(input).unwrap();
    assert!(matches!(graph.blocks[0].config, dsl::ast::Config::Positional(_)));
    assert!(matches!(graph.blocks[1].config, dsl::ast::Config::Named(_)));
    assert!(matches!(graph.blocks[2].config, dsl::ast::Config::Empty));
    assert!(matches!(graph.blocks[3].config, dsl::ast::Config::Structured(_)));
}

#[test]
fn parse_error_gives_location() {
    let err = dsl::parse("block :\n").unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.column > 0);
    assert!(!err.expected.is_empty());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p dsl --test integration`
Expected: All pass

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass (dsl crate + main crate including dsl_bridge)

- [ ] **Step 4: Commit**

```bash
git add dsl/tests/integration.rs
git commit -m "test(dsl): add integration tests for full parse-serialize pipeline"
```
