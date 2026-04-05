//! Unified type definitions shared across all panels.
//!
//! Re-exports `module-traits` and `dag-core` types directly — no TypeScript
//! mirror types needed.

pub use dag_core::op::{Dag, Op};
pub use module_traits::deployment::{
    BoardNode, ChannelBinding, ChannelTransport, DeploymentManifest, PeripheralBinding,
    SystemTopology, TaskBinding, TaskTrigger,
};
pub use module_traits::inventory::McuDef;
pub use module_traits::value::{PortDef, PortKind, Value};

/// Visual viewport state for SVG canvas pan/zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub pan_x: f32,
    pub pan_y: f32,
    pub scale: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            scale: 1.0,
        }
    }
}

/// Visual representation of a DAG node (dag-core Op + position + runtime result).
#[derive(Debug, Clone, PartialEq)]
pub struct DagNode {
    pub id: u16,
    pub op: Op,
    pub x: f32,
    pub y: f32,
    pub result: Option<f64>,
}

/// Snapshot of DAG editor state for undo/redo.
#[derive(Debug, Clone)]
pub struct DagSnapshot {
    pub nodes: Vec<DagNode>,
    pub viewport: Viewport,
    pub next_id: u16,
}

/// Known hardware channels from the MCU (for inspector dropdowns).
#[derive(Debug, Clone, Default)]
pub struct KnownChannels {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// Shared block set: a list of `(block_type, config_json)` pairs.
///
/// This is the bridge between the DAG editor (which writes) and the deploy
/// panel (which reads). Both use Leptos context to access the signal.
pub type BlockSet = Vec<(String, serde_json::Value)>;

/// DAG deployment status.
#[derive(Debug, Clone, PartialEq)]
pub enum DagStatus {
    Empty,
    Loaded { nodes: usize },
    Deployed { nodes: usize, ticks: u64 },
    Error(String),
}

impl Default for DagStatus {
    fn default() -> Self {
        Self::Empty
    }
}
