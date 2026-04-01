use alloc::string::String;
use alloc::vec::Vec;

pub type NodeId = u16;
pub type ChannelName = String;
pub type Topic = String;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Op {
    // Sources
    Const(f64),
    Input(ChannelName),

    // Sinks
    Output(ChannelName, NodeId),

    // Binary math
    Add(NodeId, NodeId),
    Mul(NodeId, NodeId),
    Sub(NodeId, NodeId),
    Div(NodeId, NodeId),
    Pow(NodeId, NodeId),

    // Unary
    Neg(NodeId),
    Relu(NodeId),

    // Pub/Sub
    Subscribe(Topic),
    Publish(Topic, NodeId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DagError {
    InvalidNodeRef { op_index: usize, referenced: NodeId },
    Full,
}

pub struct Dag {
    nodes: Vec<Op>,
}

impl Dag {
    pub fn new() -> Self {
        Dag { nodes: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> &[Op] {
        &self.nodes
    }

    pub fn add_op(&mut self, op: Op) -> Result<NodeId, DagError> {
        let current = self.nodes.len();

        // Validate that all NodeId references point to earlier nodes
        match &op {
            Op::Const(_) | Op::Input(_) | Op::Subscribe(_) => {}
            Op::Output(_, src) | Op::Neg(src) | Op::Relu(src) | Op::Publish(_, src) => {
                if *src as usize >= current {
                    return Err(DagError::InvalidNodeRef {
                        op_index: current,
                        referenced: *src,
                    });
                }
            }
            Op::Add(a, b) | Op::Mul(a, b) | Op::Sub(a, b) | Op::Div(a, b) | Op::Pow(a, b) => {
                for &r in &[*a, *b] {
                    if r as usize >= current {
                        return Err(DagError::InvalidNodeRef {
                            op_index: current,
                            referenced: r,
                        });
                    }
                }
            }
        }

        let id = current as NodeId;
        self.nodes.push(op);
        Ok(id)
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for DagError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DagError::InvalidNodeRef {
                op_index,
                referenced,
            } => {
                write!(
                    f,
                    "node {} references invalid node {}",
                    op_index, referenced
                )
            }
            DagError::Full => write!(f, "DAG is full"),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Dag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.nodes.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Dag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ops = Vec::<Op>::deserialize(deserializer)?;
        let mut dag = Dag::new();
        for op in ops {
            dag.add_op(op).map_err(serde::de::Error::custom)?;
        }
        Ok(dag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_new_is_empty() {
        let dag = Dag::new();
        assert_eq!(dag.len(), 0);
        assert!(dag.is_empty());
    }

    #[test]
    fn test_add_const_op() {
        let mut dag = Dag::new();
        let id = dag.add_op(Op::Const(42.0)).unwrap();
        assert_eq!(id, 0);
        assert_eq!(dag.len(), 1);
        assert!(!dag.is_empty());
    }

    #[test]
    fn test_add_op_valid_ref() {
        let mut dag = Dag::new();
        dag.add_op(Op::Const(1.0)).unwrap();
        dag.add_op(Op::Const(2.0)).unwrap();
        let id = dag.add_op(Op::Add(0, 1)).unwrap();
        assert_eq!(id, 2);
        assert_eq!(dag.len(), 3);
    }

    #[test]
    fn test_add_op_invalid_forward_ref() {
        let mut dag = Dag::new();
        dag.add_op(Op::Const(1.0)).unwrap();
        dag.add_op(Op::Const(2.0)).unwrap();
        let err = dag.add_op(Op::Add(0, 5)).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 2,
                referenced: 5
            }
        );
    }

    #[test]
    fn test_add_op_self_ref() {
        let mut dag = Dag::new();
        dag.add_op(Op::Const(1.0)).unwrap();
        dag.add_op(Op::Const(2.0)).unwrap();
        // Node index would be 2, so referencing 2 is a self-ref
        let err = dag.add_op(Op::Add(0, 2)).unwrap_err();
        assert_eq!(
            err,
            DagError::InvalidNodeRef {
                op_index: 2,
                referenced: 2
            }
        );
    }

    #[test]
    fn test_dag_error_display() {
        let err = DagError::InvalidNodeRef {
            op_index: 3,
            referenced: 10,
        };
        let msg = alloc::format!("{err}");
        assert!(msg.contains("node 3 references invalid node 10"));

        let err2 = DagError::Full;
        let msg2 = alloc::format!("{err2}");
        assert!(msg2.contains("full"));
    }

    #[test]
    fn test_dag_default() {
        let dag = Dag::default();
        assert!(dag.is_empty());
    }

    #[test]
    fn test_nodes_returns_slice() {
        let mut dag = Dag::new();
        dag.add_op(Op::Const(1.0)).unwrap();
        dag.add_op(Op::Const(2.0)).unwrap();
        dag.add_op(Op::Add(0, 1)).unwrap();

        let nodes = dag.nodes();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0], Op::Const(1.0));
        assert_eq!(nodes[1], Op::Const(2.0));
        assert_eq!(nodes[2], Op::Add(0, 1));
    }
}
