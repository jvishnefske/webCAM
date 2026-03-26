use crate::op::{Dag, DagError, NodeId, Op};

impl Dag {
    pub fn constant(&mut self, value: f64) -> Result<NodeId, DagError> {
        self.add_op(Op::Const(value))
    }

    pub fn input(&mut self, name: &str) -> Result<NodeId, DagError> {
        self.add_op(Op::Input(name.into()))
    }

    pub fn output(&mut self, name: &str, src: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Output(name.into(), src))
    }

    pub fn add(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Add(a, b))
    }

    pub fn mul(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Mul(a, b))
    }

    pub fn sub(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Sub(a, b))
    }

    pub fn div(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Div(a, b))
    }

    pub fn pow(&mut self, base: NodeId, exp: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Pow(base, exp))
    }

    pub fn neg(&mut self, a: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Neg(a))
    }

    pub fn relu(&mut self, a: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Relu(a))
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<NodeId, DagError> {
        self.add_op(Op::Subscribe(topic.into()))
    }

    pub fn publish(&mut self, topic: &str, src: NodeId) -> Result<NodeId, DagError> {
        self.add_op(Op::Publish(topic.into(), src))
    }
}

#[cfg(test)]
mod tests {
    use crate::op::{Dag, DagError, Op};

    #[test]
    fn test_constant() {
        let mut dag = Dag::new();
        let id = dag.constant(42.0).unwrap();
        assert_eq!(id, 0);
        assert_eq!(dag.nodes()[0], Op::Const(42.0));
    }

    #[test]
    fn test_input() {
        let mut dag = Dag::new();
        let id = dag.input("adc0").unwrap();
        assert_eq!(id, 0);
        assert_eq!(dag.nodes()[0], Op::Input("adc0".into()));
    }

    #[test]
    fn test_output() {
        let mut dag = Dag::new();
        let src = dag.constant(1.0).unwrap();
        let id = dag.output("pwm0", src).unwrap();
        assert_eq!(id, 1);
        assert_eq!(dag.nodes()[1], Op::Output("pwm0".into(), 0));
    }

    #[test]
    fn test_add() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();
        let b = dag.constant(2.0).unwrap();
        let id = dag.add(a, b).unwrap();
        assert_eq!(dag.nodes()[id as usize], Op::Add(0, 1));
    }

    #[test]
    fn test_mul() {
        let mut dag = Dag::new();
        let a = dag.constant(3.0).unwrap();
        let b = dag.constant(4.0).unwrap();
        let id = dag.mul(a, b).unwrap();
        assert_eq!(dag.nodes()[id as usize], Op::Mul(0, 1));
    }

    #[test]
    fn test_sub_div_pow() {
        let mut dag = Dag::new();
        let a = dag.constant(10.0).unwrap();
        let b = dag.constant(3.0).unwrap();

        let s = dag.sub(a, b).unwrap();
        assert_eq!(dag.nodes()[s as usize], Op::Sub(0, 1));

        let d = dag.div(a, b).unwrap();
        assert_eq!(dag.nodes()[d as usize], Op::Div(0, 1));

        let p = dag.pow(a, b).unwrap();
        assert_eq!(dag.nodes()[p as usize], Op::Pow(0, 1));
    }

    #[test]
    fn test_neg_relu() {
        let mut dag = Dag::new();
        let a = dag.constant(5.0).unwrap();
        let n = dag.neg(a).unwrap();
        assert_eq!(dag.nodes()[n as usize], Op::Neg(0));

        let r = dag.relu(a).unwrap();
        assert_eq!(dag.nodes()[r as usize], Op::Relu(0));
    }

    #[test]
    fn test_subscribe_publish() {
        let mut dag = Dag::new();
        let sub = dag.subscribe("sensor/temp").unwrap();
        assert_eq!(
            dag.nodes()[sub as usize],
            Op::Subscribe("sensor/temp".into())
        );

        let pub_id = dag.publish("actuator/fan", sub).unwrap();
        assert_eq!(
            dag.nodes()[pub_id as usize],
            Op::Publish("actuator/fan".into(), 0)
        );
    }

    #[test]
    fn test_builder_invalid_ref() {
        let mut dag = Dag::new();
        let a = dag.constant(1.0).unwrap();

        let err = dag.add(a, 99).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 1,
                referenced: 99
            }
        );

        let err = dag.neg(50).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 1,
                referenced: 50
            }
        );

        let err = dag.output("out", 10).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 1,
                referenced: 10
            }
        );

        let err = dag.publish("topic", 10).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 1,
                referenced: 10
            }
        );
    }

    #[test]
    fn test_micrograd_dag_construction() {
        let mut dag = Dag::new();
        let a = dag.constant(-4.0).unwrap(); // 0
        let b = dag.constant(2.0).unwrap(); // 1
        let c0 = dag.add(a, b).unwrap(); // 2: a + b
        let ab = dag.mul(a, b).unwrap(); // 3: a * b
        let three = dag.constant(3.0).unwrap(); // 4
        let b3 = dag.pow(b, three).unwrap(); // 5: b**3
        let d0 = dag.add(ab, b3).unwrap(); // 6: a*b + b**3
        let one = dag.constant(1.0).unwrap(); // 7
        let c1 = dag.add(c0, one).unwrap(); // 8: c + 1
        let c1 = dag.add(c0, c1).unwrap(); // 9: c += c + 1
        let neg_a = dag.neg(a).unwrap(); // 10: -a
        let c2 = dag.add(one, c1).unwrap(); // 11: 1 + c
        let c2 = dag.add(c2, neg_a).unwrap(); // 12: 1 + c + (-a)
        let c2 = dag.add(c1, c2).unwrap(); // 13: c += ...
        let two = dag.constant(2.0).unwrap(); // 14
        let d1 = dag.mul(d0, two).unwrap(); // 15: d * 2
        let ba = dag.add(b, a).unwrap(); // 16: b + a
        let ba_relu = dag.relu(ba).unwrap(); // 17: (b+a).relu()
        let d1 = dag.add(d1, ba_relu).unwrap(); // 18: d*2 + (b+a).relu()
        let d1 = dag.add(d0, d1).unwrap(); // 19: d += ...
        let d2 = dag.mul(three, d1).unwrap(); // 20: 3 * d
        let bsa = dag.sub(b, a).unwrap(); // 21: b - a
        let bsa_relu = dag.relu(bsa).unwrap(); // 22: (b-a).relu()
        let d2 = dag.add(d2, bsa_relu).unwrap(); // 23: 3*d + (b-a).relu()
        let d2 = dag.add(d1, d2).unwrap(); // 24: d += ...
        let e = dag.sub(c2, d2).unwrap(); // 25: c - d
        let f = dag.pow(e, two).unwrap(); // 26: e**2
        let half = dag.constant(0.5).unwrap(); // 27
        let g0 = dag.mul(f, half).unwrap(); // 28: f * 0.5
        let ten = dag.constant(10.0).unwrap(); // 29
        let g1 = dag.div(ten, f).unwrap(); // 30: 10.0 / f
        let g = dag.add(g0, g1).unwrap(); // 31: final result

        // 32 nodes total (indices 0..31)
        assert_eq!(dag.len(), 32);
        assert_eq!(g, 31);

        // Verify some key nodes
        assert_eq!(dag.nodes()[0], Op::Const(-4.0));
        assert_eq!(dag.nodes()[1], Op::Const(2.0));
        assert_eq!(dag.nodes()[2], Op::Add(0, 1));
        assert_eq!(dag.nodes()[10], Op::Neg(0));
        assert_eq!(dag.nodes()[17], Op::Relu(16));
        assert_eq!(dag.nodes()[31], Op::Add(28, 30));
    }
}
