use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::decode::Decoder;
use minicbor::encode::{Encoder, Write};
use minicbor::{Decode, Encode};

use crate::op::{Dag, DagError, Op};

// ---------------------------------------------------------------------------
// DecodeError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum DecodeError {
    Cbor(String),
    Dag(DagError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Cbor(msg) => write!(f, "CBOR decode error: {}", msg),
            DecodeError::Dag(e) => write!(f, "DAG validation error: {:?}", e),
        }
    }
}

// ---------------------------------------------------------------------------
// Encode<()> for Op
// ---------------------------------------------------------------------------

impl Encode<()> for Op {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Op::Const(v) => {
                e.array(2)?.u8(0)?.f64(*v)?;
            }
            Op::Input(name) => {
                e.array(2)?.u8(1)?.str(name)?;
            }
            Op::Output(name, src) => {
                e.array(3)?.u8(2)?.str(name)?.u16(*src)?;
            }
            Op::Add(a, b) => {
                e.array(3)?.u8(3)?.u16(*a)?.u16(*b)?;
            }
            Op::Mul(a, b) => {
                e.array(3)?.u8(4)?.u16(*a)?.u16(*b)?;
            }
            Op::Sub(a, b) => {
                e.array(3)?.u8(5)?.u16(*a)?.u16(*b)?;
            }
            Op::Div(a, b) => {
                e.array(3)?.u8(6)?.u16(*a)?.u16(*b)?;
            }
            Op::Pow(a, b) => {
                e.array(3)?.u8(7)?.u16(*a)?.u16(*b)?;
            }
            Op::Neg(a) => {
                e.array(2)?.u8(8)?.u16(*a)?;
            }
            Op::Relu(a) => {
                e.array(2)?.u8(9)?.u16(*a)?;
            }
            Op::Subscribe(topic) => {
                e.array(2)?.u8(10)?.str(topic)?;
            }
            Op::Publish(topic, src) => {
                e.array(3)?.u8(11)?.str(topic)?.u16(*src)?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Decode<'b, ()> for Op
// ---------------------------------------------------------------------------

impl<'b> Decode<'b, ()> for Op {
    fn decode(d: &mut Decoder<'b>, _ctx: &mut ()) -> Result<Self, minicbor::decode::Error> {
        let _len = d
            .array()?
            .ok_or_else(|| minicbor::decode::Error::message("expected definite-length array"))?;
        let tag = d.u8()?;
        match tag {
            0 => {
                let v = d.f64()?;
                Ok(Op::Const(v))
            }
            1 => {
                let s = d.str()?;
                Ok(Op::Input(s.into()))
            }
            2 => {
                let s = d.str()?;
                let src = d.u16()?;
                Ok(Op::Output(s.into(), src))
            }
            3 => {
                let a = d.u16()?;
                let b = d.u16()?;
                Ok(Op::Add(a, b))
            }
            4 => {
                let a = d.u16()?;
                let b = d.u16()?;
                Ok(Op::Mul(a, b))
            }
            5 => {
                let a = d.u16()?;
                let b = d.u16()?;
                Ok(Op::Sub(a, b))
            }
            6 => {
                let a = d.u16()?;
                let b = d.u16()?;
                Ok(Op::Div(a, b))
            }
            7 => {
                let a = d.u16()?;
                let b = d.u16()?;
                Ok(Op::Pow(a, b))
            }
            8 => {
                let a = d.u16()?;
                Ok(Op::Neg(a))
            }
            9 => {
                let a = d.u16()?;
                Ok(Op::Relu(a))
            }
            10 => {
                let s = d.str()?;
                Ok(Op::Subscribe(s.into()))
            }
            11 => {
                let s = d.str()?;
                let src = d.u16()?;
                Ok(Op::Publish(s.into(), src))
            }
            _ => Err(minicbor::decode::Error::message("unknown Op tag")),
        }
    }
}

// ---------------------------------------------------------------------------
// Encode<()> for Dag
// ---------------------------------------------------------------------------

impl Encode<()> for Dag {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut (),
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        let nodes = self.nodes();
        e.array(nodes.len() as u64)?;
        for op in nodes {
            op.encode(e, ctx)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Decode<'b, ()> for Dag  (not used directly — decode_dag validates via add_op)
// ---------------------------------------------------------------------------

impl<'b> Decode<'b, ()> for Dag {
    fn decode(d: &mut Decoder<'b>, ctx: &mut ()) -> Result<Self, minicbor::decode::Error> {
        let len = d
            .array()?
            .ok_or_else(|| minicbor::decode::Error::message("expected definite-length array"))?;
        let mut dag = Dag::new();
        for _ in 0..len {
            let op = Op::decode(d, ctx)?;
            dag.add_op(op).map_err(|_| {
                minicbor::decode::Error::message("DAG topological invariant violated")
            })?;
        }
        Ok(dag)
    }
}

// ---------------------------------------------------------------------------
// Public convenience API
// ---------------------------------------------------------------------------

/// Encode a `Dag` to CBOR bytes.
pub fn encode_dag(dag: &Dag) -> Vec<u8> {
    minicbor::to_vec(dag).expect("encoding a Dag to Vec<u8> should not fail")
}

/// Decode a `Dag` from CBOR bytes, validating topological order via `add_op`.
pub fn decode_dag(bytes: &[u8]) -> Result<Dag, DecodeError> {
    let mut d = Decoder::new(bytes);
    let len = d
        .array()
        .map_err(|e| DecodeError::Cbor(alloc::format!("{}", e)))?
        .ok_or_else(|| DecodeError::Cbor("expected definite-length array".into()))?;

    let mut dag = Dag::new();
    for _ in 0..len {
        let op =
            Op::decode(&mut d, &mut ()).map_err(|e| DecodeError::Cbor(alloc::format!("{}", e)))?;
        dag.add_op(op).map_err(DecodeError::Dag)?;
    }
    Ok(dag)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{Dag, Op};

    /// Helper: round-trip a single Op through CBOR encode/decode.
    fn round_trip_op(op: &Op) -> Op {
        let bytes = minicbor::to_vec(op).expect("encode failed");
        minicbor::decode::<Op>(&bytes).expect("decode failed")
    }

    #[test]
    fn test_encode_decode_const() {
        let op = Op::Const(42.0);
        assert_eq!(round_trip_op(&op), op);
    }

    #[test]
    fn test_encode_decode_input() {
        let op = Op::Input("adc0".into());
        assert_eq!(round_trip_op(&op), op);
    }

    #[test]
    fn test_encode_decode_output() {
        let op = Op::Output("pwm0".into(), 0);
        assert_eq!(round_trip_op(&op), op);
    }

    #[test]
    fn test_encode_decode_binary_ops() {
        for op in &[
            Op::Add(0, 1),
            Op::Mul(2, 3),
            Op::Sub(4, 5),
            Op::Div(6, 7),
            Op::Pow(8, 9),
        ] {
            assert_eq!(&round_trip_op(op), op);
        }
    }

    #[test]
    fn test_encode_decode_unary_ops() {
        for op in &[Op::Neg(0), Op::Relu(1)] {
            assert_eq!(&round_trip_op(op), op);
        }
    }

    #[test]
    fn test_encode_decode_pubsub() {
        let sub = Op::Subscribe("sensor/temp".into());
        assert_eq!(round_trip_op(&sub), sub);

        let pub_op = Op::Publish("actuator/fan".into(), 0);
        assert_eq!(round_trip_op(&pub_op), pub_op);
    }

    #[test]
    fn test_encode_decode_full_dag() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let c = dag.add(a, b).unwrap();
        let d = dag.mul(a, c).unwrap();
        let _ = dag.neg(d).unwrap();
        let inp = dag.input("adc0").unwrap();
        let _ = dag.output("pwm0", inp).unwrap();

        let bytes = encode_dag(&dag);
        let decoded = decode_dag(&bytes).expect("decode_dag failed");

        assert_eq!(dag.len(), decoded.len());
        assert_eq!(dag.nodes(), decoded.nodes());
    }

    #[test]
    fn test_encode_decode_micrograd_dag() {
        let mut dag = Dag::new();
        let a = dag.constant(-4.0).unwrap(); // 0
        let b = dag.constant(2.0).unwrap(); // 1
        let _c0 = dag.add(a, b).unwrap(); // 2
        let ab = dag.mul(a, b).unwrap(); // 3
        let three = dag.constant(3.0).unwrap(); // 4
        let b3 = dag.pow(b, three).unwrap(); // 5
        let d0 = dag.add(ab, b3).unwrap(); // 6
        let one = dag.constant(1.0).unwrap(); // 7
        let c1 = dag.add(2, one).unwrap(); // 8: c0 + 1
        let c1 = dag.add(2, c1).unwrap(); // 9
        let neg_a = dag.neg(a).unwrap(); // 10
        let c2 = dag.add(one, c1).unwrap(); // 11
        let c2 = dag.add(c2, neg_a).unwrap(); // 12
        let c2 = dag.add(c1, c2).unwrap(); // 13
        let two = dag.constant(2.0).unwrap(); // 14
        let d1 = dag.mul(d0, two).unwrap(); // 15
        let ba = dag.add(b, a).unwrap(); // 16
        let ba_relu = dag.relu(ba).unwrap(); // 17
        let d1 = dag.add(d1, ba_relu).unwrap(); // 18
        let d1 = dag.add(d0, d1).unwrap(); // 19
        let d2 = dag.mul(three, d1).unwrap(); // 20
        let bsa = dag.sub(b, a).unwrap(); // 21
        let bsa_relu = dag.relu(bsa).unwrap(); // 22
        let d2 = dag.add(d2, bsa_relu).unwrap(); // 23
        let d2 = dag.add(d1, d2).unwrap(); // 24
        let e = dag.sub(c2, d2).unwrap(); // 25
        let f = dag.pow(e, two).unwrap(); // 26
        let half = dag.constant(0.5).unwrap(); // 27
        let _g0 = dag.mul(f, half).unwrap(); // 28
        let ten = dag.constant(10.0).unwrap(); // 29
        let _g1 = dag.div(ten, f).unwrap(); // 30
        let _g = dag.add(28, 30).unwrap(); // 31

        assert_eq!(dag.len(), 32);

        let bytes = encode_dag(&dag);
        let decoded = decode_dag(&bytes).expect("decode_dag failed");

        assert_eq!(dag.len(), decoded.len());
        assert_eq!(dag.nodes(), decoded.nodes());
    }

    #[test]
    fn test_decode_error_display() {
        let err = DecodeError::Cbor("bad data".into());
        let msg = alloc::format!("{err}");
        assert!(msg.contains("CBOR decode error"));

        let err2 = DecodeError::Dag(crate::op::DagError::Full);
        let msg2 = alloc::format!("{err2}");
        assert!(msg2.contains("DAG validation error"));
    }

    #[test]
    fn test_dag_decode_impl_roundtrip() {
        let mut dag = Dag::new();
        dag.subscribe("s").unwrap();
        dag.publish("p", 0).unwrap();

        let bytes = minicbor::to_vec(&dag).expect("encode Dag");
        let decoded: Dag = minicbor::decode(&bytes).expect("decode Dag");
        assert_eq!(dag.len(), decoded.len());
        assert_eq!(dag.nodes(), decoded.nodes());
    }

    #[test]
    fn test_decode_invalid_cbor() {
        let garbage = &[0xFF, 0xFE, 0x00, 0x01];
        let result = decode_dag(garbage);
        assert!(result.is_err());
        match result {
            Err(DecodeError::Cbor(_)) => {} // expected
            Err(other) => panic!("expected DecodeError::Cbor, got {:?}", other),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn test_decode_unknown_op_tag() {
        let mut buf = Vec::new();
        {
            let mut e = minicbor::Encoder::new(&mut buf);
            e.array(2).unwrap().u8(255).unwrap().f64(0.0).unwrap();
        }

        let result = minicbor::decode::<Op>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_dag_topological_error() {
        let mut buf = Vec::new();
        {
            let mut e = minicbor::Encoder::new(&mut buf);
            e.array(1).unwrap();
            e.array(3).unwrap().u8(3).unwrap().u16(5).unwrap().u16(6).unwrap();
        }

        let result = decode_dag(&buf);
        assert!(matches!(result, Err(DecodeError::Dag(_))));
    }

    #[test]
    fn test_decode_dag_indefinite_length_error() {
        let bytes = &[0x9F, 0xFF]; // CBOR indefinite-length array
        let result = decode_dag(bytes);
        assert!(matches!(result, Err(DecodeError::Cbor(_))));
    }

    #[test]
    fn test_decode_dag_impl_topological_error() {
        let mut buf = Vec::new();
        {
            let mut e = minicbor::Encoder::new(&mut buf);
            e.array(1).unwrap();
            e.array(3).unwrap().u8(3).unwrap().u16(5).unwrap().u16(6).unwrap();
        }

        let result = minicbor::decode::<Dag>(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_cbor_compactness() {
        // Build the same 32-node micrograd DAG
        let mut dag = Dag::new();
        dag.constant(-4.0).unwrap();
        dag.constant(2.0).unwrap();
        dag.add(0, 1).unwrap();
        dag.mul(0, 1).unwrap();
        dag.constant(3.0).unwrap();
        dag.pow(1, 4).unwrap();
        dag.add(3, 5).unwrap();
        dag.constant(1.0).unwrap();
        dag.add(2, 7).unwrap();
        dag.add(2, 8).unwrap();
        dag.neg(0).unwrap();
        dag.add(7, 9).unwrap();
        dag.add(11, 10).unwrap();
        dag.add(9, 12).unwrap();
        dag.constant(2.0).unwrap();
        dag.mul(6, 14).unwrap();
        dag.add(1, 0).unwrap();
        dag.relu(16).unwrap();
        dag.add(15, 17).unwrap();
        dag.add(6, 18).unwrap();
        dag.mul(4, 19).unwrap();
        dag.sub(1, 0).unwrap();
        dag.relu(21).unwrap();
        dag.add(20, 22).unwrap();
        dag.add(19, 23).unwrap();
        dag.sub(13, 24).unwrap();
        dag.pow(25, 14).unwrap();
        dag.constant(0.5).unwrap();
        dag.mul(26, 27).unwrap();
        dag.constant(10.0).unwrap();
        dag.div(29, 26).unwrap();
        dag.add(28, 30).unwrap();

        assert_eq!(dag.len(), 32);

        let bytes = encode_dag(&dag);
        assert!(
            bytes.len() < 200,
            "CBOR encoding too large: {} bytes (limit 200)",
            bytes.len()
        );
    }
}
