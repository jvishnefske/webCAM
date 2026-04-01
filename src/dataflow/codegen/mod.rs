//! Code generation for dataflow graphs.
//!
//! Transforms a [`GraphSnapshot`] into a standalone Rust crate that executes
//! the same dataflow logic as the browser-based simulator, suitable for
//! deployment on embedded or server targets.

pub mod binding;
pub mod concurrency;
pub mod emit;
pub mod partition;
pub mod target;
pub mod targets;
pub mod topo;
pub mod types;

#[cfg(feature = "mlir")]
pub use emit::generate_workspace_mlir;
#[cfg(feature = "mlir")]
pub use emit::generate_workspace_mlir;
pub use emit::{
    generate_distributed_workspace, generate_rust, generate_workspace, CodegenBackend,
    DistributedConfig, DistributedWorkspace, GeneratedCrate, GeneratedWorkspace, TransportConfig,
};
