//! Reactive dataflow graph engine.
//!
//! Blocks have typed input/output ports connected by channels.
//! The graph ticks at a configurable rate, propagating [`Value`]s
//! from sources through the network.

pub mod block;
pub mod blocks;
pub mod channel;
pub mod codegen;
pub mod graph;
pub mod scheduler;

pub use block::{Block, BlockId, PortDef, PortKind, Value};
pub use channel::{Channel, ChannelId};
pub use graph::DataflowGraph;
pub use scheduler::Scheduler;
