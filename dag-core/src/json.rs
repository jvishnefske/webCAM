//! JSON serialization for Op and Dag via serde_json.
//!
//! Enabled by the `json` cargo feature. Serializes `Op` using serde's
//! default externally-tagged enum representation and validates topological
//! order on deserialization via [`Dag::add_op`](crate::op::Dag::add_op).

use alloc::string::String;
use alloc::vec::Vec;

use crate::op::{Dag, DagError, Op};

/// Error type for JSON decoding.
#[derive(Debug)]
pub enum JsonDecodeError {
    Json(String),
    Dag(DagError),
}

impl core::fmt::Display for JsonDecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JsonDecodeError::Json(msg) => write!(f, "JSON decode error: {}", msg),
            JsonDecodeError::Dag(e) => write!(f, "DAG validation error: {}", e),
        }
    }
}

/// Encode a `Dag` to a JSON byte vector.
pub fn encode_dag_json(dag: &Dag) -> Vec<u8> {
    serde_json::to_vec(dag.nodes()).expect("encoding a Dag to JSON should not fail")
}

/// Encode a `Dag` to a JSON string.
pub fn encode_dag_json_string(dag: &Dag) -> String {
    let bytes = serde_json::to_vec(dag.nodes()).expect("encoding a Dag to JSON should not fail");
    String::from_utf8(bytes).expect("serde_json always produces valid UTF-8")
}

/// Decode a `Dag` from a JSON byte slice, validating via `add_op`.
pub fn decode_dag_json(bytes: &[u8]) -> Result<Dag, JsonDecodeError> {
    let ops: Vec<Op> =
        serde_json::from_slice(bytes).map_err(|e| JsonDecodeError::Json(alloc::format!("{}", e)))?;
    let mut dag = Dag::new();
    for op in ops {
        dag.add_op(op).map_err(JsonDecodeError::Dag)?;
    }
    Ok(dag)
}

/// Decode a `Dag` from a JSON string, validating via `add_op`.
pub fn decode_dag_json_str(s: &str) -> Result<Dag, JsonDecodeError> {
    decode_dag_json(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;

    fn round_trip_ops(ops: &[Op]) -> Vec<Op> {
        let json = serde_json::to_vec(ops).expect("encode failed");
        serde_json::from_slice::<Vec<Op>>(&json).expect("decode failed")
    }

    #[test]
    fn test_round_trip_const() {
        let ops = vec![Op::Const(42.0)];
        assert_eq!(round_trip_ops(&ops), ops);
    }

    #[test]
    fn test_round_trip_input_output() {
        let ops = vec![
            Op::Input("adc0".into()),
            Op::Output("pwm0".into(), 0),
        ];
        assert_eq!(round_trip_ops(&ops), ops);
    }

    #[test]
    fn test_round_trip_binary_ops() {
        let ops = vec![
            Op::Const(1.0),
            Op::Const(2.0),
            Op::Add(0, 1),
            Op::Mul(0, 1),
            Op::Sub(0, 1),
            Op::Div(0, 1),
            Op::Pow(0, 1),
        ];
        assert_eq!(round_trip_ops(&ops), ops);
    }

    #[test]
    fn test_round_trip_unary_ops() {
        let ops = vec![Op::Const(1.0), Op::Neg(0), Op::Relu(0)];
        assert_eq!(round_trip_ops(&ops), ops);
    }

    #[test]
    fn test_round_trip_pubsub() {
        let ops = vec![
            Op::Subscribe("sensor/temp".into()),
            Op::Publish("actuator/fan".into(), 0),
        ];
        assert_eq!(round_trip_ops(&ops), ops);
    }

    #[test]
    fn test_encode_decode_full_dag() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let c = dag.add(a, b).unwrap();
        let _ = dag.mul(a, c).unwrap();
        let inp = dag.input("adc0").unwrap();
        let _ = dag.output("pwm0", inp).unwrap();

        let json = encode_dag_json(&dag);
        let decoded = decode_dag_json(&json).expect("decode failed");
        assert_eq!(dag.nodes(), decoded.nodes());
    }

    #[test]
    fn test_decode_from_string() {
        let json = r#"[{"Const":3.0},{"Const":4.0},{"Add":[0,1]}]"#;
        let dag = decode_dag_json_str(json).unwrap();
        assert_eq!(dag.len(), 3);
        assert_eq!(dag.nodes()[2], Op::Add(0, 1));
    }

    #[test]
    fn test_decode_invalid_json() {
        let result = decode_dag_json(b"not json");
        assert!(matches!(result, Err(JsonDecodeError::Json(_))));
    }

    #[test]
    fn test_decode_invalid_forward_ref() {
        let json = r#"[{"Const":1.0},{"Add":[0,5]}]"#;
        let result = decode_dag_json_str(json);
        assert!(matches!(result, Err(JsonDecodeError::Dag(_))));
    }

    #[test]
    fn test_json_human_readable() {
        let mut dag = Dag::new();
        dag.input("adc0").unwrap();
        dag.constant(2.0).unwrap();
        dag.mul(0, 1).unwrap();
        dag.output("pwm0", 2).unwrap();

        let json = encode_dag_json_string(&dag);
        assert!(json.contains(r#""Input":"adc0""#));
        assert!(json.contains(r#""Const":2.0"#));
        assert!(json.contains(r#""Mul":[0,1]"#));
        assert!(json.contains(r#""Output":["pwm0",2]"#));
    }
}
