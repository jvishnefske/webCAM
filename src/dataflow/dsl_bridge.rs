//! Bridge between the DSL parser AST and the dataflow engine's graph types.
//!
//! Converts `parser::ast::Graph` into a `GraphSnapshot` suitable for
//! hydrating a `DataflowGraph` or feeding into codegen.

use std::collections::HashMap;

use crate::dataflow::block::BlockId;
use crate::dataflow::blocks::create_block;
use crate::dataflow::channel::{Channel, ChannelId};
use crate::dataflow::codegen::target::TargetFamily;
use crate::dataflow::graph::{BlockSnapshot, GraphSnapshot};

/// Convert a DSL AST graph into a `GraphSnapshot`.
pub fn ast_to_snapshot(graph: &parser::ast::Graph) -> Result<GraphSnapshot, String> {
    let mut blocks = Vec::new();
    let mut name_to_id: HashMap<String, BlockId> = HashMap::new();
    let mut next_id: u32 = 1;

    // Pass 1: create blocks and build the name -> id map.
    for decl in &graph.blocks {
        let id = BlockId(next_id);
        next_id += 1;
        name_to_id.insert(decl.id.clone(), id);

        let config_json = config_to_json(&decl.block_type, &decl.config);
        let module = create_block(&decl.block_type, &config_json)?;

        // Resolve @target annotation if present.
        let target = decl
            .annotations
            .iter()
            .find(|a| a.name == "target")
            .and_then(|a| a.args.first())
            .and_then(|v| match v {
                parser::ast::Value::Ident(name) => parse_target_family(name),
                parser::ast::Value::Text(name) => parse_target_family(name),
                _ => None,
            });

        let config: serde_json::Value =
            serde_json::from_str(&config_json).unwrap_or(serde_json::Value::Null);

        let custom_codegen = module.as_codegen().and_then(|cg| cg.emit_rust("host").ok());

        blocks.push(BlockSnapshot {
            id: id.0,
            block_type: module.block_type().to_string(),
            name: module.name().to_string(),
            inputs: module.input_ports(),
            outputs: module.output_ports(),
            config,
            output_values: vec![None; module.output_ports().len()],
            target,
            custom_codegen,
        });
    }

    // Pass 2: resolve connections.
    let mut channels = Vec::new();
    let mut next_ch_id: u32 = 1;

    for conn in &graph.connections {
        let from_block = *name_to_id
            .get(&conn.from_block)
            .ok_or_else(|| format!("unknown block '{}'", conn.from_block))?;
        let to_block = *name_to_id
            .get(&conn.to_block)
            .ok_or_else(|| format!("unknown block '{}'", conn.to_block))?;

        let from_snap = blocks
            .iter()
            .find(|b| b.id == from_block.0)
            .ok_or_else(|| format!("internal error: missing block {}", from_block.0))?;
        let to_snap = blocks
            .iter()
            .find(|b| b.id == to_block.0)
            .ok_or_else(|| format!("internal error: missing block {}", to_block.0))?;

        let from_port = from_snap
            .outputs
            .iter()
            .position(|p| p.name == conn.from_port)
            .ok_or_else(|| {
                format!(
                    "no output port '{}' on block '{}'",
                    conn.from_port, conn.from_block
                )
            })?;
        let to_port = to_snap
            .inputs
            .iter()
            .position(|p| p.name == conn.to_port)
            .ok_or_else(|| {
                format!(
                    "no input port '{}' on block '{}'",
                    conn.to_port, conn.to_block
                )
            })?;

        channels.push(Channel {
            id: ChannelId(next_ch_id),
            from_block,
            from_port,
            to_block,
            to_port,
        });
        next_ch_id += 1;
    }

    Ok(GraphSnapshot {
        blocks,
        channels,
        tick_count: 0,
        time: 0.0,
    })
}

/// Map a lowercase DSL target name to a `TargetFamily`.
fn parse_target_family(name: &str) -> Option<TargetFamily> {
    match name {
        "host" => Some(TargetFamily::Host),
        "rp2040" => Some(TargetFamily::Rp2040),
        "stm32f4" => Some(TargetFamily::Stm32f4),
        "esp32c3" => Some(TargetFamily::Esp32c3),
        "stm32g0b1" => Some(TargetFamily::Stm32g0b1),
        _ => None,
    }
}

/// Convert a DSL `Config` to a JSON string suitable for `create_block()`.
pub fn config_to_json(block_type: &str, config: &parser::ast::Config) -> String {
    match config {
        parser::ast::Config::Empty => "{}".to_string(),
        parser::ast::Config::Positional(args) => positional_to_json(block_type, args),
        parser::ast::Config::Named(pairs) | parser::ast::Config::Structured(pairs) => {
            let mut map: serde_json::Map<String, serde_json::Value> = pairs
                .iter()
                .map(|(k, v)| (k.clone(), dsl_value_to_json(v)))
                .collect();
            // Inject required defaults that the DSL user can omit.
            inject_defaults(block_type, &mut map);
            serde_json::Value::Object(map).to_string()
        }
    }
}

/// Inject required defaults for block configs where the DSL omits them.
fn inject_defaults(block_type: &str, map: &mut serde_json::Map<String, serde_json::Value>) {
    match block_type {
        "gain" | "clamp" => {
            if !map.contains_key("op") {
                let op = match block_type {
                    "gain" => "Gain",
                    "clamp" => "Clamp",
                    _ => return,
                };
                map.insert("op".into(), serde_json::json!(op));
            }
            if !map.contains_key("param1") {
                map.insert("param1".into(), serde_json::json!(0.0));
            }
            if !map.contains_key("param2") {
                map.insert("param2".into(), serde_json::json!(0.0));
            }
        }
        "adc_source" => {
            if !map.contains_key("resolution_bits") {
                map.insert("resolution_bits".into(), serde_json::json!(12));
            }
        }
        "pwm_sink" => {
            if !map.contains_key("frequency_hz") {
                map.insert("frequency_hz".into(), serde_json::json!(1000));
            }
        }
        _ => {}
    }
}

/// Map positional args to the JSON keys expected by `create_block()`.
fn positional_to_json(block_type: &str, args: &[parser::ast::Value]) -> String {
    match block_type {
        "constant" => {
            let v = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(0.0));
            serde_json::json!({ "value": v }).to_string()
        }
        "gain" => {
            let v = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(1.0));
            serde_json::json!({ "op": "Gain", "param1": v, "param2": 0.0 }).to_string()
        }
        "clamp" => {
            let min = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(0.0));
            let max = args
                .get(1)
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(1.0));
            serde_json::json!({ "op": "Clamp", "param1": min, "param2": max }).to_string()
        }
        "plot" => {
            let ms = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(1000));
            serde_json::json!({ "max_samples": ms }).to_string()
        }
        "adc_source" => {
            let ch = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(0));
            serde_json::json!({ "channel": ch, "resolution_bits": 12 }).to_string()
        }
        "pwm_sink" => {
            let ch = args
                .first()
                .map(dsl_value_to_json)
                .unwrap_or(serde_json::json!(0));
            serde_json::json!({ "channel": ch, "frequency_hz": 1000 }).to_string()
        }
        _ => {
            // Generic: wrap as arg0, arg1, ...
            let map: serde_json::Map<String, serde_json::Value> = args
                .iter()
                .enumerate()
                .map(|(i, v)| (format!("arg{i}"), dsl_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map).to_string()
        }
    }
}

/// Convert a DSL `Value` to a `serde_json::Value`.
fn dsl_value_to_json(v: &parser::ast::Value) -> serde_json::Value {
    match v {
        parser::ast::Value::Int(n) => serde_json::json!(n),
        parser::ast::Value::Float(f) => serde_json::json!(f),
        parser::ast::Value::Text(s) => serde_json::json!(s),
        parser::ast::Value::Ident(s) => serde_json::json!(s),
        parser::ast::Value::List(items) => {
            serde_json::Value::Array(items.iter().map(dsl_value_to_json).collect())
        }
        parser::ast::Value::Map(pairs) => {
            let map: serde_json::Map<String, serde_json::Value> = pairs
                .iter()
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
        let graph =
            parser::parse("block c: constant(42.0)\nblock g: gain(2.5)\nc.out -> g.in\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert_eq!(snapshot.blocks.len(), 2);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.blocks[0].block_type, "constant");
        assert_eq!(snapshot.blocks[1].block_type, "gain");
    }

    #[test]
    fn bridge_resolves_port_names() {
        let graph =
            parser::parse("block c: constant(1.0)\nblock g: gain(2.0)\nc.out -> g.in\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        let ch = &snapshot.channels[0];
        assert_eq!(ch.from_port, 0);
        assert_eq!(ch.to_port, 0);
    }

    #[test]
    fn bridge_unknown_block_name_errors() {
        let graph = parser::parse("block c: constant(1.0)\nx.out -> c.in\n").unwrap();
        let result = ast_to_snapshot(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown block"));
    }

    #[test]
    fn bridge_unknown_port_name_errors() {
        let graph =
            parser::parse("block c: constant(1.0)\nblock g: gain(2.0)\nc.nonexistent -> g.in\n")
                .unwrap();
        let result = ast_to_snapshot(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no output port"));
    }

    #[test]
    fn bridge_with_target_annotation() {
        let input = "@target(rp2040)\nblock s: adc_source(channel = 0)\n";
        let graph = parser::parse(input).unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert!(snapshot.blocks[0].target.is_some());
    }

    #[test]
    fn bridge_no_config_block() {
        let graph = parser::parse("block sum: add\n").unwrap();
        let snapshot = ast_to_snapshot(&graph).unwrap();
        assert_eq!(snapshot.blocks[0].block_type, "add");
    }

    #[test]
    fn test_ast_to_snapshot_blocks_and_channels() {
        let src = "block a: constant(1.0)\nblock b: constant(2.0)\nblock s: add\na.out -> s.a\nb.out -> s.b\n";
        let graph = parser::parse(src).unwrap();
        let snap = ast_to_snapshot(&graph).unwrap();
        assert_eq!(snap.blocks.len(), 3);
        assert_eq!(snap.channels.len(), 2);
        assert_eq!(snap.tick_count, 0);
        assert!((snap.time - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dsl_value_to_json_all_variants() {
        use parser::ast::Value;

        // Int
        let v = dsl_value_to_json(&Value::Int(42));
        assert_eq!(v, serde_json::json!(42));

        // Float
        let v = dsl_value_to_json(&Value::Float(3.25));
        assert_eq!(v, serde_json::json!(3.25));

        // Text
        let v = dsl_value_to_json(&Value::Text("hello".into()));
        assert_eq!(v, serde_json::json!("hello"));

        // Ident
        let v = dsl_value_to_json(&Value::Ident("foo".into()));
        assert_eq!(v, serde_json::json!("foo"));

        // List
        let v = dsl_value_to_json(&Value::List(vec![Value::Int(1), Value::Int(2)]));
        assert_eq!(v, serde_json::json!([1, 2]));

        // Map
        let v = dsl_value_to_json(&Value::Map(vec![("k".into(), Value::Int(10))]));
        assert_eq!(v, serde_json::json!({"k": 10}));
    }

    #[test]
    fn test_positional_to_json_constant() {
        let args = vec![parser::ast::Value::Float(99.0)];
        let json: serde_json::Value =
            serde_json::from_str(&positional_to_json("constant", &args)).unwrap();
        assert_eq!(json["value"], 99.0);
    }

    #[test]
    fn test_positional_to_json_gain() {
        let args = vec![parser::ast::Value::Float(2.5)];
        let json: serde_json::Value =
            serde_json::from_str(&positional_to_json("gain", &args)).unwrap();
        assert_eq!(json["param1"], 2.5);
        assert_eq!(json["op"], "Gain");
    }

    #[test]
    fn test_positional_to_json_clamp() {
        let args = vec![
            parser::ast::Value::Float(0.0),
            parser::ast::Value::Float(10.0),
        ];
        let json: serde_json::Value =
            serde_json::from_str(&positional_to_json("clamp", &args)).unwrap();
        assert_eq!(json["param1"], 0.0);
        assert_eq!(json["param2"], 10.0);
    }

    #[test]
    fn test_positional_to_json_generic_fallback() {
        let args = vec![parser::ast::Value::Int(7)];
        let json: serde_json::Value =
            serde_json::from_str(&positional_to_json("unknown_type", &args)).unwrap();
        assert_eq!(json["arg0"], 7);
    }

    #[test]
    fn dsl_config_named_to_json() {
        let config = parser::ast::Config::Named(vec![
            ("channel".into(), parser::ast::Value::Int(0)),
            ("frequency".into(), parser::ast::Value::Int(1000)),
        ]);
        let json: serde_json::Value =
            serde_json::from_str(&config_to_json("test", &config)).unwrap();
        assert_eq!(json["channel"], 0);
        assert_eq!(json["frequency"], 1000);
    }
}
