//! Code generation for dataflow graphs.
//!
//! This module re-exports from the `codegen-emit` crate, which contains the
//! actual implementation. Rustsim provides conversion from its local
//! snapshot types to the codegen-emit types.

// Re-export submodules from codegen-emit for backward compatibility.
pub use codegen_emit::concurrency;
pub use codegen_emit::emit;
pub use codegen_emit::snapshot;
pub use codegen_emit::topo;
pub use codegen_emit::types;

/// Re-export deployment partition for backward compatibility.
pub use deployment::partition;

// Re-export target-registry types for backward compatibility within rustsim.
pub use target_registry::binding;
pub use target_registry::generators as targets;
pub use target_registry::target;

#[cfg(feature = "mlir")]
pub use codegen_emit::generate_workspace_mlir;
pub use codegen_emit::{
    generate_distributed_workspace, generate_rust, generate_workspace, CodegenBackend,
    CodegenBlockSnapshot, CodegenGraphSnapshot, DistributedConfig, DistributedWorkspace,
    GeneratedCrate, GeneratedWorkspace, TransportConfig,
};
