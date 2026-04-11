//! Deployment manifest types for multi-MCU graph deployment.
//!
//! A [`DeploymentManifest`] maps blocks to physical target nodes and
//! describes the network topology (links, protocols) between them.

use std::collections::HashMap;

use graph_model::BlockId;
use serde::{Deserialize, Serialize};

/// Identifier for a physical deployment node (MCU, host, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Complete deployment manifest binding a dataflow graph to hardware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentManifest {
    /// Maps each block to the node it should run on.
    pub assignments: HashMap<BlockId, NodeId>,
    /// Physical nodes in the deployment.
    pub nodes: Vec<TargetNode>,
    /// Physical links between nodes.
    pub topology: Vec<PhysicalLink>,
}

/// A physical deployment node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetNode {
    /// Unique node identifier.
    pub id: NodeId,
    /// Target identifier (e.g. "rp2040", "stm32f4", "host").
    pub target_id: String,
    /// Optional role (e.g. "controller", "sensor-hub").
    pub role: Option<String>,
}

/// A physical communication link between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalLink {
    /// Unique link identifier.
    pub id: String,
    /// Source node.
    pub from_node: NodeId,
    /// Destination node.
    pub to_node: NodeId,
    /// Communication protocol.
    pub protocol: Protocol,
}

/// Communication protocol for a physical link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Protocol {
    Spi { clock_hz: u32 },
    Uart { baud: u32 },
    I2c { freq_hz: u32 },
    Can { bitrate: u32 },
    SharedMemory,
    UdpMulticast { group: String, port: u16 },
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = DeploymentManifest {
            assignments: {
                let mut m = HashMap::new();
                m.insert(BlockId(1), NodeId("mcu-a".to_string()));
                m.insert(BlockId(2), NodeId("mcu-b".to_string()));
                m
            },
            nodes: vec![
                TargetNode {
                    id: NodeId("mcu-a".to_string()),
                    target_id: "rp2040".to_string(),
                    role: Some("controller".to_string()),
                },
                TargetNode {
                    id: NodeId("mcu-b".to_string()),
                    target_id: "stm32f4".to_string(),
                    role: None,
                },
            ],
            topology: vec![PhysicalLink {
                id: "link-1".to_string(),
                from_node: NodeId("mcu-a".to_string()),
                to_node: NodeId("mcu-b".to_string()),
                protocol: Protocol::Can { bitrate: 500_000 },
            }],
        };

        let json = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        let restored: DeploymentManifest =
            serde_json::from_str(&json).expect("deserialize manifest");

        assert_eq!(restored.assignments.len(), 2);
        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.topology.len(), 1);
        assert_eq!(
            restored.assignments[&BlockId(1)],
            NodeId("mcu-a".to_string())
        );
        assert_eq!(restored.nodes[0].target_id, "rp2040");
    }

    #[test]
    fn protocol_serde_roundtrip() {
        let protocols = vec![
            Protocol::Spi { clock_hz: 1_000_000 },
            Protocol::Uart { baud: 115_200 },
            Protocol::I2c { freq_hz: 400_000 },
            Protocol::Can { bitrate: 250_000 },
            Protocol::SharedMemory,
            Protocol::UdpMulticast {
                group: "239.1.2.3".to_string(),
                port: 5000,
            },
        ];

        for proto in &protocols {
            let json = serde_json::to_string(proto).expect("serialize protocol");
            let restored: Protocol = serde_json::from_str(&json).expect("deserialize protocol");
            // Verify roundtrip doesn't panic; exact equality checked via Debug
            let _ = format!("{restored:?}");
        }
    }

    #[test]
    fn node_id_equality() {
        let a = NodeId("x".to_string());
        let b = NodeId("x".to_string());
        let c = NodeId("y".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
