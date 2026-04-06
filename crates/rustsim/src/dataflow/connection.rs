//! Sans-IO connection validation for the dataflow graph.
//!
//! Pure functions that validate whether a connection between two ports
//! is valid, without touching DOM or WASM. Testable with property-based tests.

use serde::{Deserialize, Serialize};

/// Which side of a block a port is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortSide {
    Input,
    Output,
}

/// A request to connect two ports.
#[derive(Debug, Clone)]
pub struct ConnectionRequest {
    pub from_block: u32,
    pub from_port: usize,
    pub from_side: PortSide,
    pub to_block: u32,
    pub to_port: usize,
    pub to_side: PortSide,
}

/// Why a connection is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionError {
    /// Both ports are on the same side (both inputs or both outputs).
    SameSide,
    /// Cannot connect a block to itself.
    SelfConnection,
    /// This exact connection already exists.
    Duplicate,
    /// The source port doesn't exist on the block.
    InvalidSourcePort {
        block_id: u32,
        port: usize,
        max_ports: usize,
    },
    /// The target port doesn't exist on the block.
    InvalidTargetPort {
        block_id: u32,
        port: usize,
        max_ports: usize,
    },
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SameSide => write!(f, "cannot connect ports on the same side"),
            Self::SelfConnection => write!(f, "cannot connect a block to itself"),
            Self::Duplicate => write!(f, "connection already exists"),
            Self::InvalidSourcePort {
                block_id,
                port,
                max_ports,
            } => write!(
                f,
                "block {block_id} has no output port {port} (max {max_ports})"
            ),
            Self::InvalidTargetPort {
                block_id,
                port,
                max_ports,
            } => write!(
                f,
                "block {block_id} has no input port {port} (max {max_ports})"
            ),
        }
    }
}

/// Normalize a connection request so the output side is always "from".
/// Returns (output_block, output_port, input_block, input_port).
pub fn normalize(req: &ConnectionRequest) -> (u32, usize, u32, usize) {
    if req.from_side == PortSide::Output {
        (req.from_block, req.from_port, req.to_block, req.to_port)
    } else {
        (req.to_block, req.to_port, req.from_block, req.from_port)
    }
}

/// Validate a connection request against the current graph state.
///
/// `output_port_counts`: maps block_id -> number of output ports
/// `input_port_counts`: maps block_id -> number of input ports
/// `existing_connections`: list of (from_block, from_port, to_block, to_port)
pub fn validate_connection(
    req: &ConnectionRequest,
    output_port_counts: &std::collections::HashMap<u32, usize>,
    input_port_counts: &std::collections::HashMap<u32, usize>,
    existing_connections: &[(u32, usize, u32, usize)],
) -> Result<(u32, usize, u32, usize), ConnectionError> {
    // Rule 1: Must connect output to input (different sides)
    if req.from_side == req.to_side {
        return Err(ConnectionError::SameSide);
    }

    // Rule 2: Cannot connect to self
    if req.from_block == req.to_block {
        return Err(ConnectionError::SelfConnection);
    }

    // Normalize to (output_block, output_port, input_block, input_port)
    let (out_block, out_port, in_block, in_port) = normalize(req);

    // Rule 3: Validate port indices
    let out_max = output_port_counts.get(&out_block).copied().unwrap_or(0);
    if out_port >= out_max {
        return Err(ConnectionError::InvalidSourcePort {
            block_id: out_block,
            port: out_port,
            max_ports: out_max,
        });
    }
    let in_max = input_port_counts.get(&in_block).copied().unwrap_or(0);
    if in_port >= in_max {
        return Err(ConnectionError::InvalidTargetPort {
            block_id: in_block,
            port: in_port,
            max_ports: in_max,
        });
    }

    // Rule 4: No duplicates
    let normalized = (out_block, out_port, in_block, in_port);
    if existing_connections.contains(&normalized) {
        return Err(ConnectionError::Duplicate);
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper: build port count maps for two blocks.
    /// Block 1 has `out1` outputs and `in1` inputs.
    /// Block 2 has `out2` outputs and `in2` inputs.
    fn port_counts(
        out1: usize,
        in1: usize,
        out2: usize,
        in2: usize,
    ) -> (HashMap<u32, usize>, HashMap<u32, usize>) {
        let mut outputs = HashMap::new();
        let mut inputs = HashMap::new();
        outputs.insert(1, out1);
        inputs.insert(1, in1);
        outputs.insert(2, out2);
        inputs.insert(2, in2);
        (outputs, inputs)
    }

    #[test]
    fn test_valid_output_to_input() {
        let (outputs, inputs) = port_counts(2, 1, 1, 2);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Output,
            to_block: 2,
            to_port: 0,
            to_side: PortSide::Input,
        };
        let result = validate_connection(&req, &outputs, &inputs, &[]);
        assert_eq!(result, Ok((1, 0, 2, 0)));
    }

    #[test]
    fn test_valid_input_to_output() {
        let (outputs, inputs) = port_counts(2, 1, 1, 2);
        // User drags from block 2's input to block 1's output — normalization flips.
        let req = ConnectionRequest {
            from_block: 2,
            from_port: 1,
            from_side: PortSide::Input,
            to_block: 1,
            to_port: 0,
            to_side: PortSide::Output,
        };
        let result = validate_connection(&req, &outputs, &inputs, &[]);
        // After normalization: output side (block 1, port 0) -> input side (block 2, port 1)
        assert_eq!(result, Ok((1, 0, 2, 1)));
    }

    #[test]
    fn test_same_side_output_output() {
        let (outputs, inputs) = port_counts(2, 1, 2, 1);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Output,
            to_block: 2,
            to_port: 0,
            to_side: PortSide::Output,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &[]),
            Err(ConnectionError::SameSide)
        );
    }

    #[test]
    fn test_same_side_input_input() {
        let (outputs, inputs) = port_counts(1, 2, 1, 2);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Input,
            to_block: 2,
            to_port: 0,
            to_side: PortSide::Input,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &[]),
            Err(ConnectionError::SameSide)
        );
    }

    #[test]
    fn test_self_connection() {
        let (outputs, inputs) = port_counts(2, 2, 1, 1);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Output,
            to_block: 1,
            to_port: 0,
            to_side: PortSide::Input,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &[]),
            Err(ConnectionError::SelfConnection)
        );
    }

    #[test]
    fn test_duplicate_connection() {
        let (outputs, inputs) = port_counts(2, 1, 1, 2);
        let existing = vec![(1, 0, 2, 0)];
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Output,
            to_block: 2,
            to_port: 0,
            to_side: PortSide::Input,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &existing),
            Err(ConnectionError::Duplicate)
        );
    }

    #[test]
    fn test_invalid_source_port() {
        let (outputs, inputs) = port_counts(1, 1, 1, 1);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 5, // block 1 only has 1 output (index 0)
            from_side: PortSide::Output,
            to_block: 2,
            to_port: 0,
            to_side: PortSide::Input,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &[]),
            Err(ConnectionError::InvalidSourcePort {
                block_id: 1,
                port: 5,
                max_ports: 1,
            })
        );
    }

    #[test]
    fn test_invalid_target_port() {
        let (outputs, inputs) = port_counts(2, 1, 1, 1);
        let req = ConnectionRequest {
            from_block: 1,
            from_port: 0,
            from_side: PortSide::Output,
            to_block: 2,
            to_port: 3, // block 2 only has 1 input (index 0)
            to_side: PortSide::Input,
        };
        assert_eq!(
            validate_connection(&req, &outputs, &inputs, &[]),
            Err(ConnectionError::InvalidTargetPort {
                block_id: 2,
                port: 3,
                max_ports: 1,
            })
        );
    }

    #[test]
    fn test_normalize_output_first() {
        let req = ConnectionRequest {
            from_block: 10,
            from_port: 2,
            from_side: PortSide::Output,
            to_block: 20,
            to_port: 3,
            to_side: PortSide::Input,
        };
        assert_eq!(normalize(&req), (10, 2, 20, 3));
    }

    #[test]
    fn test_normalize_input_first() {
        let req = ConnectionRequest {
            from_block: 10,
            from_port: 2,
            from_side: PortSide::Input,
            to_block: 20,
            to_port: 3,
            to_side: PortSide::Output,
        };
        // Input is "from", so normalize swaps: output side (20, 3) -> input side (10, 2)
        assert_eq!(normalize(&req), (20, 3, 10, 2));
    }
}
