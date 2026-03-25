//! Pi Zero DAP processor construction.
//!
//! Wraps the stub DAP backend into a [`DapProcessor`] for sharing
//! across WebSocket connections.

use dap_dispatch::protocol::DapProcessor;
use dap_dispatch::stub::StubDapProcessor;

/// Creates a DAP processor for the Pi Zero.
///
/// Currently uses a stub implementation that responds to `DAP_Info`
/// queries. Will be replaced with a real bitbang SWD backend when
/// a compatible CMSIS-DAP crate is integrated.
pub fn create_dap_processor() -> impl DapProcessor {
    StubDapProcessor::new("Pi Zero DAP")
}
