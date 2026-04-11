#![forbid(unsafe_code)]

//! Code generation for dataflow graphs.
//!
//! Transforms a [`snapshot::CodegenGraphSnapshot`] into a standalone Rust
//! workspace that executes the same dataflow logic as the browser-based
//! simulator, suitable for deployment on embedded or server targets.

pub mod concurrency;
pub mod emit;
pub mod snapshot;
pub mod topo;
pub mod types;

/// Re-export deployment partition for backward compatibility.
pub use deployment::partition;

// Re-export target-registry types for backward compatibility.
pub use target_registry::binding;
pub use target_registry::generators as targets;
pub use target_registry::target;

#[cfg(feature = "mlir")]
pub use emit::generate_workspace_mlir;
pub use emit::{
    generate_distributed_workspace, generate_rust, generate_workspace, CodegenBackend,
    DistributedConfig, DistributedWorkspace, GeneratedCrate, GeneratedWorkspace, TransportConfig,
};

pub use snapshot::{CodegenBlockSnapshot, CodegenGraphSnapshot};
