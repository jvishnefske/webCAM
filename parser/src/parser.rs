use crate::ast::{Annotation, BlockDecl, Config, Connection, Graph, Value};

peg::parser! {
    pub grammar flow_parser() for str {
        // Horizontal whitespace (spaces and tabs only)
        rule _() = [' ' | '\t']*

        // Any whitespace including newlines and comments
        rule __() = ([' ' | '\t' | '\n' | '\r'] / "#" [^ '\n']* "\n")*

        // Identifiers: [a-zA-Z_][a-zA-Z0-9_]*
        pub rule ident() -> String
            = s:$(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*)
            { s.to_string() }

        // Floats: optional '-', digits, '.', digits
        pub rule float() -> f64
            = s:$("-"? ['0'..='9']+ "." ['0'..='9']+)
            { s.parse().unwrap() }

        // Integers: optional '-', digits
        pub rule int() -> i64
            = s:$("-"? ['0'..='9']+)
            { s.parse().unwrap() }

        // String escape sequences
        rule string_char() -> char
            = "\\\"" { '"' }
            / "\\\\" { '\\' }
            / "\\n" { '\n' }
            / "\\t" { '\t' }
            / c:$([^ '"' | '\\']) { c.chars().next().unwrap() }

        // Strings: double-quoted with escape sequences
        pub rule string() -> String
            = "\"" chars:string_char()* "\"" { chars.into_iter().collect() }

        // Values: ordered choice (float before int, ident last)
        pub rule value() -> Value
            = v:float() { Value::Float(v) }
            / v:int() { Value::Int(v) }
            / v:string() { Value::Text(v) }
            / "[" __ vals:(value() ** (__ "," __)) __ "]" { Value::List(vals) }
            / "{" __ entries:map_entry() ++ (__ "," __) __ "}"
              { Value::Map(entries) }
            / v:ident() { Value::Ident(v) }

        // Map entry helper: key: value
        rule map_entry() -> (String, Value)
            = k:ident() __ ":" __ v:value() { (k, v) }

        // Named argument: key = value
        rule named_arg() -> (String, Value)
            = k:ident() _ "=" _ v:value() { (k, v) }

        // Separator for structured entries: comma or newline (with surrounding whitespace)
        rule struct_sep() = __ "," __ / __

        // Config forms
        pub rule config() -> Config
            = "(" _ args:named_arg() ++ (_ "," _) _ ")" { Config::Named(args) }
            / "(" _ vals:value() ++ (_ "," _) _ ")" { Config::Positional(vals) }
            / "{" __ entries:map_entry() ++ struct_sep() __ "}" { Config::Structured(entries) }

        // Comment: # to end of line
        rule comment() = "#" [^ '\n']*

        // End of line: optional horizontal whitespace then newline or end-of-input
        rule eol() = _ ("\n" / ![_])

        // Line separator: one or more newlines (possibly with comments)
        rule line_sep() = (_ comment()? "\n")+ _

        // Annotation: @name(args...)
        pub rule annotation() -> Annotation
            = "@" name:ident() "(" _ args:(value() ** (_ "," _)) _ ")"
            { Annotation { name, args } }

        // Connection: from.port -> to.port
        pub rule connection() -> Connection
            = from_block:ident() "." from_port:ident() _ "->" _ to_block:ident() "." to_port:ident()
            { Connection { from_block, from_port, to_block, to_port } }

        // Block declaration: [annotations]* block id: type [config]
        pub rule block_decl() -> BlockDecl
            = annotations:(a:annotation() _ "\n" _ { a })* "block" _ id:ident() _ ":" _ block_type:ident() cfg:(_ c:config() { c })?
            {
                BlockDecl {
                    id,
                    block_type,
                    config: cfg.unwrap_or(Config::Empty),
                    annotations,
                }
            }

        // A single statement: block decl, connection, or comment/blank
        rule statement() -> (Option<BlockDecl>, Option<Connection>)
            = b:block_decl() { (Some(b), None) }
            / c:connection() { (None, Some(c)) }
            / comment() { (None, None) }

        // Top-level graph
        pub rule graph() -> Graph
            = __  stmts:(statement() ** line_sep()) __
            {
                let mut blocks = Vec::new();
                let mut connections = Vec::new();
                for (b, c) in stmts {
                    if let Some(b) = b { blocks.push(b); }
                    if let Some(c) = c { connections.push(c); }
                }
                Graph { blocks, connections }
            }
    }
}

#[cfg(test)]
mod tests {
    use super::flow_parser;
    use crate::ast::*;

    // Atom tests
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

    // Value tests
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

    // Config tests
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

    #[test]
    fn parse_connection() {
        assert_eq!(
            flow_parser::connection("sensor.out -> amp.input"),
            Ok(Connection {
                from_block: "sensor".into(), from_port: "out".into(),
                to_block: "amp".into(), to_port: "input".into(),
            })
        );
    }

    #[test]
    fn parse_annotation() {
        assert_eq!(
            flow_parser::annotation("@target(rp2040)"),
            Ok(Annotation { name: "target".into(), args: vec![Value::Ident("rp2040".into())] })
        );
    }

    #[test]
    fn parse_block_decl_simple() {
        assert_eq!(
            flow_parser::block_decl("block sensor: constant(42.0)"),
            Ok(BlockDecl {
                id: "sensor".into(), block_type: "constant".into(),
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
                id: "sum".into(), block_type: "add".into(),
                config: Config::Empty, annotations: vec![],
            })
        );
    }

    #[test]
    fn parse_block_decl_with_annotation() {
        let input = "@target(rp2040)\nblock sensor: adc_source(channel = 0)";
        assert_eq!(
            flow_parser::block_decl(input),
            Ok(BlockDecl {
                id: "sensor".into(), block_type: "adc_source".into(),
                config: Config::Named(vec![("channel".into(), Value::Int(0))]),
                annotations: vec![Annotation { name: "target".into(), args: vec![Value::Ident("rp2040".into())] }],
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
        let input = "@target(rp2040)\nblock sensor: adc_source(channel = 0)\nblock amp: gain(2.5)\n\n@target(host)\nblock display: plot(\"Sensor Output\")\n\nsensor.out -> amp.input\namp.out -> display.input\n";
        let g = flow_parser::graph(input).unwrap();
        assert_eq!(g.blocks.len(), 3);
        assert_eq!(g.connections.len(), 2);
        assert_eq!(g.blocks[0].annotations.len(), 1);
    }

    #[test]
    fn parse_graph_interleaved() {
        let input = "block a: constant(1.0)\na.out -> b.input\nblock b: gain(2.0)\n";
        let g = flow_parser::graph(input).unwrap();
        assert_eq!(g.blocks.len(), 2);
        assert_eq!(g.connections.len(), 1);
    }

    #[test]
    fn parse_graph_with_comments() {
        let input = "# This is a comment\nblock a: constant(1.0)\n# Another comment\na.out -> b.input\nblock b: gain(2.0)\n";
        let g = flow_parser::graph(input).unwrap();
        assert_eq!(g.blocks.len(), 2);
        assert_eq!(g.connections.len(), 1);
    }

    #[test]
    fn parse_error_invalid_syntax() {
        assert!(flow_parser::graph("block : missing_id").is_err());
    }
}
