use wasm_bindgen::prelude::*;

use dag_core::cbor;
use dag_core::eval::{NullChannels, NullPubSub};
use dag_core::op::{Dag, Op};

#[wasm_bindgen]
pub struct DagHandle {
    dag: Dag,
}

impl Default for DagHandle {
    fn default() -> Self {
        Self::new()
    }
}


#[wasm_bindgen]
impl DagHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        DagHandle { dag: Dag::new() }
    }

    pub fn len(&self) -> usize {
        self.dag.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dag.is_empty()
    }

    // Builder methods -- return node ID or throw JS error

    pub fn constant(&mut self, value: f64) -> Result<u16, JsValue> {
        self.dag
            .constant(value)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn input(&mut self, name: &str) -> Result<u16, JsValue> {
        self.dag
            .input(name)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn output(&mut self, name: &str, src: u16) -> Result<u16, JsValue> {
        self.dag
            .output(name, src)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn add(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.dag
            .add(a, b)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn mul(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.dag
            .mul(a, b)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn sub(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.dag
            .sub(a, b)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn div(&mut self, a: u16, b: u16) -> Result<u16, JsValue> {
        self.dag
            .div(a, b)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn pow(&mut self, base: u16, exp: u16) -> Result<u16, JsValue> {
        self.dag
            .pow(base, exp)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn neg(&mut self, a: u16) -> Result<u16, JsValue> {
        self.dag
            .neg(a)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn relu(&mut self, a: u16) -> Result<u16, JsValue> {
        self.dag
            .relu(a)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<u16, JsValue> {
        self.dag
            .subscribe(topic)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    pub fn publish(&mut self, topic: &str, src: u16) -> Result<u16, JsValue> {
        self.dag
            .publish(topic, src)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    /// Evaluate the DAG with null channels (pure math).
    /// Returns the values array as a `Float64Array`.
    pub fn evaluate(&self) -> Vec<f64> {
        let mut values = vec![0.0; self.dag.len()];
        self.dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        values
    }

    /// Get value at a specific node after evaluation.
    pub fn evaluate_node(&self, node_id: u16) -> f64 {
        let mut values = vec![0.0; self.dag.len()];
        self.dag.evaluate(&NullChannels, &NullPubSub, &mut values);
        values[node_id as usize]
    }

    /// Encode to CBOR bytes.
    pub fn to_cbor(&self) -> Vec<u8> {
        cbor::encode_dag(&self.dag)
    }

    /// Decode from CBOR bytes.
    pub fn from_cbor(bytes: &[u8]) -> Result<DagHandle, JsValue> {
        let dag = cbor::decode_dag(bytes).map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        Ok(DagHandle { dag })
    }

    /// Decode from CBOR bytes (testable helper).
    pub fn from_cbor_impl(bytes: &[u8]) -> Result<DagHandle, String> {
        let dag = cbor::decode_dag(bytes).map_err(|e| format!("{}", e))?;
        Ok(DagHandle { dag })
    }

    /// Get a JSON representation of the DAG structure for the UI.
    pub fn to_json(&self) -> Result<String, JsValue> {
        let mut nodes = Vec::new();
        for (i, op) in self.dag.nodes().iter().enumerate() {
            let node_str = match op {
                Op::Const(v) => {
                    format!(r#"{{"id":{},"op":"const","value":{}}}"#, i, v)
                }
                Op::Input(name) => {
                    format!(r#"{{"id":{},"op":"input","name":"{}"}}"#, i, name)
                }
                Op::Output(name, src) => {
                    format!(
                        r#"{{"id":{},"op":"output","name":"{}","src":{}}}"#,
                        i, name, src
                    )
                }
                Op::Add(a, b) => {
                    format!(r#"{{"id":{},"op":"add","a":{},"b":{}}}"#, i, a, b)
                }
                Op::Mul(a, b) => {
                    format!(r#"{{"id":{},"op":"mul","a":{},"b":{}}}"#, i, a, b)
                }
                Op::Sub(a, b) => {
                    format!(r#"{{"id":{},"op":"sub","a":{},"b":{}}}"#, i, a, b)
                }
                Op::Div(a, b) => {
                    format!(r#"{{"id":{},"op":"div","a":{},"b":{}}}"#, i, a, b)
                }
                Op::Pow(a, b) => {
                    format!(r#"{{"id":{},"op":"pow","a":{},"b":{}}}"#, i, a, b)
                }
                Op::Neg(a) => {
                    format!(r#"{{"id":{},"op":"neg","a":{}}}"#, i, a)
                }
                Op::Relu(a) => {
                    format!(r#"{{"id":{},"op":"relu","a":{}}}"#, i, a)
                }
                Op::Subscribe(topic) => {
                    format!(r#"{{"id":{},"op":"subscribe","topic":"{}"}}"#, i, topic)
                }
                Op::Publish(topic, src) => {
                    format!(
                        r#"{{"id":{},"op":"publish","topic":"{}","src":{}}}"#,
                        i, topic, src
                    )
                }
            };
            nodes.push(node_str);
        }
        Ok(format!("[{}]", nodes.join(",")))
    }
}

// ── Testable _impl helpers (returning Result<_, String>) ──────────────
impl DagHandle {
    /// Add constant (testable).
    pub fn constant_impl(&mut self, value: f64) -> Result<u16, String> {
        self.dag.constant(value).map_err(|e| format!("{:?}", e))
    }
    /// Add input (testable).
    pub fn input_impl(&mut self, name: &str) -> Result<u16, String> {
        self.dag.input(name).map_err(|e| format!("{:?}", e))
    }
    /// Add output (testable).
    pub fn output_impl(&mut self, name: &str, src: u16) -> Result<u16, String> {
        self.dag.output(name, src).map_err(|e| format!("{:?}", e))
    }
    /// Add (testable).
    pub fn add_impl(&mut self, a: u16, b: u16) -> Result<u16, String> {
        self.dag.add(a, b).map_err(|e| format!("{:?}", e))
    }
    /// Mul (testable).
    pub fn mul_impl(&mut self, a: u16, b: u16) -> Result<u16, String> {
        self.dag.mul(a, b).map_err(|e| format!("{:?}", e))
    }
    /// Sub (testable).
    pub fn sub_impl(&mut self, a: u16, b: u16) -> Result<u16, String> {
        self.dag.sub(a, b).map_err(|e| format!("{:?}", e))
    }
    /// Div (testable).
    pub fn div_impl(&mut self, a: u16, b: u16) -> Result<u16, String> {
        self.dag.div(a, b).map_err(|e| format!("{:?}", e))
    }
    /// Pow (testable).
    pub fn pow_impl(&mut self, base: u16, exp: u16) -> Result<u16, String> {
        self.dag.pow(base, exp).map_err(|e| format!("{:?}", e))
    }
    /// Neg (testable).
    pub fn neg_impl(&mut self, a: u16) -> Result<u16, String> {
        self.dag.neg(a).map_err(|e| format!("{:?}", e))
    }
    /// Relu (testable).
    pub fn relu_impl(&mut self, a: u16) -> Result<u16, String> {
        self.dag.relu(a).map_err(|e| format!("{:?}", e))
    }
    /// Subscribe (testable).
    pub fn subscribe_impl(&mut self, topic: &str) -> Result<u16, String> {
        self.dag.subscribe(topic).map_err(|e| format!("{:?}", e))
    }
    /// Publish (testable).
    pub fn publish_impl(&mut self, topic: &str, src: u16) -> Result<u16, String> {
        self.dag.publish(topic, src).map_err(|e| format!("{:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_handle_new() {
        let handle = DagHandle::new();
        assert_eq!(handle.len(), 0);
        assert!(handle.is_empty());
    }

    #[test]
    fn test_dag_handle_builder() {
        let mut handle = DagHandle::new();
        let a = handle.constant(1.0).unwrap();
        let b = handle.constant(2.0).unwrap();
        let _c = handle.add(a, b).unwrap();
        assert_eq!(handle.len(), 3);
        assert!(!handle.is_empty());
    }

    #[test]
    fn test_dag_handle_evaluate() {
        // Reproduce the micrograd example from dag-core eval tests.
        let mut h = DagHandle::new();
        let a = h.constant(-4.0).unwrap(); // 0
        let b = h.constant(2.0).unwrap(); // 1
        let c0 = h.add(a, b).unwrap(); // 2
        let ab = h.mul(a, b).unwrap(); // 3
        let three = h.constant(3.0).unwrap(); // 4
        let b3 = h.pow(b, three).unwrap(); // 5
        let d0 = h.add(ab, b3).unwrap(); // 6
        let one = h.constant(1.0).unwrap(); // 7
        let c1 = h.add(c0, one).unwrap(); // 8
        let c1 = h.add(c0, c1).unwrap(); // 9
        let neg_a = h.neg(a).unwrap(); // 10
        let c2 = h.add(one, c1).unwrap(); // 11
        let c2 = h.add(c2, neg_a).unwrap(); // 12
        let c2 = h.add(c1, c2).unwrap(); // 13
        let two = h.constant(2.0).unwrap(); // 14
        let d1 = h.mul(d0, two).unwrap(); // 15
        let ba = h.add(b, a).unwrap(); // 16
        let ba_relu = h.relu(ba).unwrap(); // 17
        let d1 = h.add(d1, ba_relu).unwrap(); // 18
        let d1 = h.add(d0, d1).unwrap(); // 19
        let d2 = h.mul(three, d1).unwrap(); // 20
        let bsa = h.sub(b, a).unwrap(); // 21
        let bsa_relu = h.relu(bsa).unwrap(); // 22
        let d2 = h.add(d2, bsa_relu).unwrap(); // 23
        let d2 = h.add(d1, d2).unwrap(); // 24
        let e = h.sub(c2, d2).unwrap(); // 25
        let f = h.pow(e, two).unwrap(); // 26
        let half = h.constant(0.5).unwrap(); // 27
        let g0 = h.mul(f, half).unwrap(); // 28
        let ten = h.constant(10.0).unwrap(); // 29
        let g1 = h.div(ten, f).unwrap(); // 30
        let g = h.add(g0, g1).unwrap(); // 31

        let result = h.evaluate_node(g);
        assert!(
            (result - 24.7041).abs() < 1e-4,
            "Expected ~24.7041, got {}",
            result
        );
    }

    #[test]
    fn test_dag_handle_cbor_roundtrip() {
        let mut h = DagHandle::new();
        let a = h.constant(3.0).unwrap();
        let b = h.constant(7.0).unwrap();
        let c = h.add(a, b).unwrap();
        let d = h.mul(a, c).unwrap();
        let _ = h.neg(d).unwrap();

        let bytes = h.to_cbor();
        let h2 = DagHandle::from_cbor(&bytes).unwrap();

        assert_eq!(h.len(), h2.len());

        let vals1 = h.evaluate();
        let vals2 = h2.evaluate();
        assert_eq!(vals1, vals2);
    }

    #[test]
    fn test_dag_handle_to_json() {
        let mut h = DagHandle::new();
        let a = h.constant(1.0).unwrap();
        let b = h.constant(2.0).unwrap();
        let _c = h.add(a, b).unwrap();

        let json = h.to_json().unwrap();
        assert!(json.contains(r#""op":"const""#));
        assert!(json.contains(r#""op":"add""#));
        assert!(json.contains(r#""value":1"#));
        assert!(json.contains(r#""value":2"#));
        assert!(json.contains(r#""a":0"#));
        assert!(json.contains(r#""b":1"#));
    }

    #[test]
    fn test_dag_handle_constant() {
        let mut h = DagHandle::new();
        let id = h.constant(3.25).unwrap();
        assert_eq!(id, 0);
        assert_eq!(h.evaluate_node(id), 3.25);
    }

    #[test]
    fn test_dag_handle_input() {
        let mut h = DagHandle::new();
        let id = h.input("x").unwrap();
        assert_eq!(id, 0);
        // Input with NullChannels yields 0.0
        assert_eq!(h.evaluate_node(id), 0.0);
    }

    #[test]
    fn test_dag_handle_output() {
        let mut h = DagHandle::new();
        let c = h.constant(5.0).unwrap();
        let id = h.output("y", c).unwrap();
        assert_eq!(id, 1);
        assert_eq!(h.evaluate_node(id), 5.0);
    }

    #[test]
    fn test_dag_handle_add() {
        let mut h = DagHandle::new();
        let a = h.constant(1.0).unwrap();
        let b = h.constant(2.0).unwrap();
        let c = h.add(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 3.0);
    }

    #[test]
    fn test_dag_handle_sub() {
        let mut h = DagHandle::new();
        let a = h.constant(5.0).unwrap();
        let b = h.constant(3.0).unwrap();
        let c = h.sub(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 2.0);
    }

    #[test]
    fn test_dag_handle_mul() {
        let mut h = DagHandle::new();
        let a = h.constant(3.0).unwrap();
        let b = h.constant(4.0).unwrap();
        let c = h.mul(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 12.0);
    }

    #[test]
    fn test_dag_handle_div() {
        let mut h = DagHandle::new();
        let a = h.constant(10.0).unwrap();
        let b = h.constant(2.0).unwrap();
        let c = h.div(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 5.0);
    }

    #[test]
    fn test_dag_handle_neg() {
        let mut h = DagHandle::new();
        let a = h.constant(7.0).unwrap();
        let b = h.neg(a).unwrap();
        assert_eq!(h.evaluate_node(b), -7.0);
    }

    #[test]
    fn test_dag_handle_pow() {
        let mut h = DagHandle::new();
        let a = h.constant(2.0).unwrap();
        let b = h.constant(3.0).unwrap();
        let c = h.pow(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 8.0);
    }

    #[test]
    fn test_dag_handle_relu() {
        let mut h = DagHandle::new();
        let pos = h.constant(5.0).unwrap();
        let neg = h.constant(-3.0).unwrap();
        let r1 = h.relu(pos).unwrap();
        let r2 = h.relu(neg).unwrap();
        assert_eq!(h.evaluate_node(r1), 5.0);
        assert_eq!(h.evaluate_node(r2), 0.0);
    }

    #[test]
    fn test_dag_handle_publish_subscribe() {
        let mut h = DagHandle::new();
        let sub_id = h.subscribe("topic_a").unwrap();
        let c = h.constant(1.0).unwrap();
        let pub_id = h.publish("topic_a", c).unwrap();
        assert_eq!(sub_id, 0);
        assert_eq!(pub_id, 2);
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn test_dag_handle_from_cbor() {
        let mut h = DagHandle::new();
        let a = h.constant(2.0).unwrap();
        let b = h.constant(3.0).unwrap();
        let _ = h.add(a, b).unwrap();
        let bytes = h.to_cbor();

        let h2 = DagHandle::from_cbor(&bytes).unwrap();
        assert_eq!(h2.len(), 3);
        assert_eq!(h2.evaluate_node(2), 5.0);
    }

    #[test]
    fn test_dag_handle_invalid_ref() {
        // Test the underlying Dag error path directly, since JsValue
        // cannot be constructed on non-wasm32 targets.
        use dag_core::op::{Dag, DagError};

        let mut dag = Dag::new();
        let _a = dag.constant(1.0).unwrap();

        // Reference to non-existent node 99
        let result = dag.add(0, 99);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            DagError::InvalidNodeRef {
                op_index: 1,
                referenced: 99
            }
        );

        let result = dag.neg(50);
        assert!(result.is_err());

        let result = dag.output("out", 10);
        assert!(result.is_err());
    }

    #[test]
    fn dag_handle_default() {
        let d = DagHandle::default();
        assert!(d.is_empty());
    }

    // ── Error path tests for underlying Dag API ────────────────────────

    #[test]
    fn test_dag_invalid_ref_all_binary_ops() {
        use dag_core::op::Dag;

        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();

        // mul with invalid ref
        let result = dag.mul(a, 99);
        assert!(result.is_err());

        // sub with invalid ref
        let result = dag.sub(a, 99);
        assert!(result.is_err());

        // div with invalid ref
        let result = dag.div(a, 99);
        assert!(result.is_err());

        // pow with invalid ref
        let result = dag.pow(a, 99);
        assert!(result.is_err());

        // relu with invalid ref
        let result = dag.relu(99);
        assert!(result.is_err());

        // subscribe and publish with invalid ref
        let result = dag.publish("topic", 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_from_cbor_invalid() {
        // Invalid CBOR data should return an error
        let result = dag_core::cbor::decode_dag(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_from_cbor_empty() {
        // Empty bytes should fail
        let result = dag_core::cbor::decode_dag(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_to_json_all_op_types() {
        let mut h = DagHandle::new();
        let a = h.constant(1.0).unwrap();
        let b = h.constant(2.0).unwrap();
        let inp = h.input("x").unwrap();
        let _ = h.output("y", a).unwrap();
        let _ = h.add(a, b).unwrap();
        let _ = h.mul(a, b).unwrap();
        let _ = h.sub(a, b).unwrap();
        let _ = h.div(a, b).unwrap();
        let _ = h.pow(a, b).unwrap();
        let _ = h.neg(a).unwrap();
        let _ = h.relu(inp).unwrap();
        let _sub = h.subscribe("topic_a").unwrap();
        let _ = h.publish("topic_b", a).unwrap();

        let json = h.to_json().unwrap();
        assert!(json.contains(r#""op":"const""#));
        assert!(json.contains(r#""op":"input""#));
        assert!(json.contains(r#""op":"output""#));
        assert!(json.contains(r#""op":"add""#));
        assert!(json.contains(r#""op":"mul""#));
        assert!(json.contains(r#""op":"sub""#));
        assert!(json.contains(r#""op":"div""#));
        assert!(json.contains(r#""op":"pow""#));
        assert!(json.contains(r#""op":"neg""#));
        assert!(json.contains(r#""op":"relu""#));
        assert!(json.contains(r#""op":"subscribe""#));
        assert!(json.contains(r#""op":"publish""#));
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), h.len());
    }

    #[test]
    fn test_dag_evaluate_all_values() {
        let mut h = DagHandle::new();
        let a = h.constant(3.0).unwrap();
        let b = h.constant(4.0).unwrap();
        let _ = h.add(a, b).unwrap();

        let values = h.evaluate();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], 3.0);
        assert_eq!(values[1], 4.0);
        assert_eq!(values[2], 7.0);
    }

    // ── _impl helper tests (error paths) ──────────────────────────────

    #[test]
    fn test_constant_impl_success() {
        let mut h = DagHandle::new();
        let id = h.constant_impl(3.25).unwrap();
        assert_eq!(id, 0);
    }

    #[test]
    fn test_input_impl_success() {
        let mut h = DagHandle::new();
        let id = h.input_impl("x").unwrap();
        assert_eq!(id, 0);
    }

    #[test]
    fn test_output_impl_success() {
        let mut h = DagHandle::new();
        let c = h.constant_impl(5.0).unwrap();
        let id = h.output_impl("y", c).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn test_output_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let result = h.output_impl("y", 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let b = h.constant_impl(2.0).unwrap();
        let c = h.add_impl(a, b).unwrap();
        assert_eq!(c, 2);
    }

    #[test]
    fn test_add_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let result = h.add_impl(a, 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_mul_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let result = h.mul_impl(a, 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_sub_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let result = h.sub_impl(a, 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_div_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let result = h.div_impl(a, 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_pow_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(1.0).unwrap();
        let result = h.pow_impl(a, 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_neg_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let result = h.neg_impl(99);
        assert!(result.is_err());
    }

    #[test]
    fn test_relu_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let result = h.relu_impl(99);
        assert!(result.is_err());
    }

    #[test]
    fn test_subscribe_impl_success() {
        let mut h = DagHandle::new();
        let id = h.subscribe_impl("topic").unwrap();
        assert_eq!(id, 0);
    }

    #[test]
    fn test_publish_impl_success() {
        let mut h = DagHandle::new();
        let c = h.constant_impl(1.0).unwrap();
        let id = h.publish_impl("topic", c).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn test_publish_impl_invalid_ref() {
        let mut h = DagHandle::new();
        let result = h.publish_impl("topic", 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_cbor_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(2.0).unwrap();
        let b = h.constant_impl(3.0).unwrap();
        let _ = h.add_impl(a, b).unwrap();
        let bytes = h.to_cbor();
        let h2 = DagHandle::from_cbor_impl(&bytes).unwrap();
        assert_eq!(h2.len(), 3);
    }

    #[test]
    fn test_from_cbor_impl_invalid() {
        let result = DagHandle::from_cbor_impl(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_cbor_impl_empty() {
        let result = DagHandle::from_cbor_impl(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mul_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(3.0).unwrap();
        let b = h.constant_impl(4.0).unwrap();
        let c = h.mul_impl(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 12.0);
    }

    #[test]
    fn test_sub_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(5.0).unwrap();
        let b = h.constant_impl(3.0).unwrap();
        let c = h.sub_impl(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 2.0);
    }

    #[test]
    fn test_div_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(10.0).unwrap();
        let b = h.constant_impl(2.0).unwrap();
        let c = h.div_impl(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 5.0);
    }

    #[test]
    fn test_pow_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(2.0).unwrap();
        let b = h.constant_impl(3.0).unwrap();
        let c = h.pow_impl(a, b).unwrap();
        assert_eq!(h.evaluate_node(c), 8.0);
    }

    #[test]
    fn test_neg_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(7.0).unwrap();
        let b = h.neg_impl(a).unwrap();
        assert_eq!(h.evaluate_node(b), -7.0);
    }

    #[test]
    fn test_relu_impl_success() {
        let mut h = DagHandle::new();
        let a = h.constant_impl(-3.0).unwrap();
        let b = h.relu_impl(a).unwrap();
        assert_eq!(h.evaluate_node(b), 0.0);
    }
}
