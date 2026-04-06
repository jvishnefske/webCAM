//! Reactive dataflow graph engine.
//!
//! Blocks have typed input/output ports connected by channels.
//! The graph ticks at a configurable rate, propagating [`Value`]s
//! from sources through the network.

pub mod block;
pub mod blocks;
pub mod channel;
pub mod codegen;
pub mod connection;
pub mod dsl_bridge;
pub mod graph;
pub mod panel;
pub mod scheduler;
pub mod sim_peripherals;

pub use block::{BlockId, Module, PortDef, PortKind, Tick, Value};
pub use channel::{Channel, ChannelId};
pub use graph::DataflowGraph;
pub use panel::{PanelModel, PanelRuntime};
pub use scheduler::Scheduler;
