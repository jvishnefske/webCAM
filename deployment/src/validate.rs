//! Deployment validation checks.
//!
//! Validates that a [`DeploymentManifest`] is consistent with a
//! [`GraphSnapshot`] — all blocks assigned, inter-node channels have
//! physical links, etc.

use std::collections::HashSet;

use crate::manifest::{DeploymentManifest, NodeId};

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single validation diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

/// Validate a deployment manifest against a graph snapshot.
///
/// Returns a list of diagnostics (errors and warnings). An empty list
/// means the deployment is valid.
pub fn validate(
    graph: &graph_model::GraphSnapshot,
    manifest: &DeploymentManifest,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_all_blocks_assigned(graph, manifest, &mut diags);
    check_topology_coverage(graph, manifest, &mut diags);
    diags
}

/// Check that every block in the graph has an assignment in the manifest.
fn check_all_blocks_assigned(
    graph: &graph_model::GraphSnapshot,
    manifest: &DeploymentManifest,
    diags: &mut Vec<Diagnostic>,
) {
    for block in &graph.blocks {
        if !manifest.assignments.contains_key(&block.id) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "E001",
                message: format!(
                    "block {} ({:?}) has no node assignment",
                    block.id.0, block.name
                ),
            });
        }
    }
}

/// Check that every cross-node channel has a physical link in the topology.
fn check_topology_coverage(
    graph: &graph_model::GraphSnapshot,
    manifest: &DeploymentManifest,
    diags: &mut Vec<Diagnostic>,
) {
    // Build set of connected node pairs from topology (bidirectional).
    let mut linked_pairs: HashSet<(NodeId, NodeId)> = HashSet::new();
    for link in &manifest.topology {
        linked_pairs.insert((link.from_node.clone(), link.to_node.clone()));
        linked_pairs.insert((link.to_node.clone(), link.from_node.clone()));
    }

    for ch in &graph.channels {
        let from_node = manifest.assignments.get(&ch.from_block);
        let to_node = manifest.assignments.get(&ch.to_block);

        if let (Some(from), Some(to)) = (from_node, to_node) {
            if from != to {
                // Cross-node channel — check for physical link.
                let pair = (from.clone(), to.clone());
                if !linked_pairs.contains(&pair) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "E002",
                        message: format!(
                            "channel {} connects blocks on different nodes ({:?} -> {:?}) \
                             but no physical link exists between them",
                            ch.id.0, from.0, to.0
                        ),
                    });
                }
            }
        }
        // If blocks are unassigned, check_all_blocks_assigned already reports it.
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::manifest::{DeploymentManifest, NodeId, PhysicalLink, Protocol, TargetNode};
    use graph_model::{BlockId, BlockSnapshot, Channel, ChannelId, GraphSnapshot};
    use module_traits::value::{PortDef, PortKind};
    use std::collections::HashMap;

    fn simple_graph() -> GraphSnapshot {
        GraphSnapshot {
            blocks: vec![
                BlockSnapshot {
                    id: BlockId(1),
                    block_type: "constant".to_string(),
                    name: "src".to_string(),
                    inputs: vec![],
                    outputs: vec![PortDef::new("out", PortKind::Float)],
                    config: serde_json::json!({}),
                    is_delay: false,
                },
                BlockSnapshot {
                    id: BlockId(2),
                    block_type: "gain".to_string(),
                    name: "amp".to_string(),
                    inputs: vec![PortDef::new("in", PortKind::Float)],
                    outputs: vec![PortDef::new("out", PortKind::Float)],
                    config: serde_json::json!({}),
                    is_delay: false,
                },
            ],
            channels: vec![Channel {
                id: ChannelId(1),
                from_block: BlockId(1),
                from_port: 0,
                to_block: BlockId(2),
                to_port: 0,
            }],
        }
    }

    fn full_manifest() -> DeploymentManifest {
        DeploymentManifest {
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
                    role: None,
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
        }
    }

    #[test]
    fn valid_deployment_returns_no_diagnostics() {
        let graph = simple_graph();
        let manifest = full_manifest();
        let diags = validate(&graph, &manifest);
        assert!(
            diags.is_empty(),
            "expected no diagnostics, got: {:?}",
            diags
        );
    }

    #[test]
    fn unassigned_block_produces_error() {
        let graph = simple_graph();
        let mut manifest = full_manifest();
        manifest.assignments.remove(&BlockId(2)); // remove assignment for block 2

        let diags = validate(&graph, &manifest);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].code, "E001");
        assert!(diags[0].message.contains("block 2"));
    }

    #[test]
    fn missing_link_produces_error() {
        let graph = simple_graph();
        let mut manifest = full_manifest();
        manifest.topology.clear(); // remove all links

        let diags = validate(&graph, &manifest);
        // Should have E002 for the cross-node channel
        let e002: Vec<_> = diags.iter().filter(|d| d.code == "E002").collect();
        assert_eq!(e002.len(), 1);
        assert!(e002[0].message.contains("mcu-a"));
        assert!(e002[0].message.contains("mcu-b"));
    }

    #[test]
    fn same_node_channel_needs_no_link() {
        let graph = simple_graph();
        let mut manifest = full_manifest();
        // Put both blocks on the same node — no link needed.
        manifest
            .assignments
            .insert(BlockId(2), NodeId("mcu-a".to_string()));
        manifest.topology.clear();

        let diags = validate(&graph, &manifest);
        assert!(diags.is_empty());
    }

    #[test]
    fn multiple_errors_reported() {
        let graph = simple_graph();
        let manifest = DeploymentManifest {
            assignments: HashMap::new(), // nothing assigned
            nodes: vec![],
            topology: vec![],
        };

        let diags = validate(&graph, &manifest);
        // At least 2 E001 errors (one per unassigned block).
        let e001_count = diags.iter().filter(|d| d.code == "E001").count();
        assert!(e001_count >= 2);
    }
}
