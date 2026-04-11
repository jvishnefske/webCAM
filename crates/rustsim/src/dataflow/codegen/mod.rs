//! Code generation for dataflow graphs.
//!
//! Transforms a [`GraphSnapshot`] into a standalone Rust crate that executes
//! the same dataflow logic as the browser-based simulator, suitable for
//! deployment on embedded or server targets.

pub mod concurrency;
pub mod emit;
pub mod partition;
pub mod topo;
pub mod types;

// Re-export target-registry types for backward compatibility within rustsim.
pub use target_registry::binding;
pub use target_registry::generators as targets;
pub use target_registry::target;

#[cfg(feature = "mlir")]
pub use emit::generate_workspace_mlir;
pub use emit::{
    generate_distributed_workspace, generate_rust, generate_workspace, CodegenBackend,
    DistributedConfig, DistributedWorkspace, GeneratedCrate, GeneratedWorkspace, TransportConfig,
};
