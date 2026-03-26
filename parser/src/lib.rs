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
        let input = "block a: constant(1.0)\nblock b: gain(2.5)\n\na.out -> b.input\n";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_named_config() {
        let input = "block m: pwm_sink(channel = 0, frequency = 1000)\n";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_annotations() {
        let input = "@target(rp2040)\nblock s: adc_source(channel = 0)\n\n@target(host)\nblock p: plot(\"Signal\")\n\ns.out -> p.input\n";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_structured_config() {
        let input = "block ctrl: state_machine {\n  initial: idle\n  states: [idle, running]\n}\n";
        let ast1 = parse(input).unwrap();
        let output = serialize(&ast1);
        let ast2 = parse(&output).unwrap();
        assert_eq!(ast1, ast2);
    }

    #[test]
    fn round_trip_no_config_block() {
        let input = "block sum: add\n";
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
