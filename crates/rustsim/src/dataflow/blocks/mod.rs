//! Block implementations — re-exported from block-registry crate.

pub use block_registry::{
    create_block, available_block_types, BlockTypeInfo,
    // Re-export sub-modules that tests or other code might reference
    data_driven, embedded, plot, pubsub,
    register, registry, serde_block, state_machine, udp,
};
