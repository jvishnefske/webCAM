/// A value in the DSL.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Value>,
}

/// Block configuration.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Config {
    Empty,
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
    Structured(Vec<(String, Value)>),
}

/// A block declaration.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockDecl {
    pub id: String,
    pub block_type: String,
    pub config: Config,
    pub annotations: Vec<Annotation>,
}

/// A connection between ports.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Connection {
    pub from_block: String,
    pub from_port: String,
    pub to_block: String,
    pub to_port: String,
}

/// A complete dataflow graph.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
