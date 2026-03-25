//! Transport-agnostic DAP processor trait.
//!
//! Board crates implement [`DapProcessor`] by wrapping their concrete
//! CMSIS-DAP backend. This trait is the only abstraction the library
//! knows — all transport and encoding logic works through it.

/// Processes raw CMSIS-DAP commands.
///
/// Implementors wrap a concrete CMSIS-DAP backend (e.g. `rust-dap`'s
/// `CmsisDap` or a custom bitbang implementation) and forward the raw
/// command bytes to it.
///
/// # Example
///
/// ```ignore
/// struct MyDap { /* wraps CmsisDap<...> */ }
///
/// impl DapProcessor for MyDap {
///     fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize {
///         self.inner.process(request, response)
///     }
/// }
/// ```
pub trait DapProcessor {
    /// Processes a single CMSIS-DAP command.
    ///
    /// Reads the command from `request`, writes the response into
    /// `response`, and returns the number of response bytes written.
    /// A return value of 0 indicates no response.
    fn process_command(&mut self, request: &[u8], response: &mut [u8]) -> usize;
}
