//! Heap-free message dispatch trait for backplane nodes.
//!
//! The [`Dispatcher`] trait replaces `HashMap<u32, Box<dyn Fn>>` with a
//! user-implemented concrete struct that matches on `type_id`. This
//! avoids heap allocation and dynamic dispatch, making it suitable for
//! `no_std` environments.
//!
//! # Example
//!
//! ```rust
//! use hil_backplane::dispatcher::Dispatcher;
//! use hil_backplane::envelope::Envelope;
//! use hil_backplane::error::BackplaneError;
//!
//! struct MyDispatcher;
//!
//! impl Dispatcher for MyDispatcher {
//!     fn dispatch(
//!         &mut self,
//!         type_id: u32,
//!         payload: &[u8],
//!         envelope: &Envelope,
//!         response_buf: &mut [u8],
//!     ) -> Result<Option<(u32, usize)>, BackplaneError> {
//!         // Match on type_id values and decode/handle each message type.
//!         Ok(None) // Unknown type_id: silently ignore.
//!     }
//! }
//! ```

use crate::envelope::Envelope;
use crate::error::BackplaneError;

/// Dispatches incoming backplane messages without heap allocation.
///
/// Implementors match on `type_id` to decode and handle each known
/// message type. Responses are written directly into `response_buf`.
///
/// Returns `Ok(Some((response_type_id, response_len)))` if the handler
/// produced a response, or `Ok(None)` if no response is needed (e.g.
/// for publish messages or unknown type IDs).
pub trait Dispatcher {
    /// Dispatches a received message by its type ID.
    ///
    /// # Parameters
    ///
    /// - `type_id`: FNV-1a hash identifying the message type.
    /// - `payload`: CBOR-encoded message bytes (after the envelope header).
    /// - `envelope`: The decoded envelope header for routing metadata.
    /// - `response_buf`: Buffer for writing a CBOR-encoded response.
    ///
    /// # Returns
    ///
    /// - `Ok(Some((type_id, len)))` — a response was written into
    ///   `response_buf[..len]` with the given response type ID.
    /// - `Ok(None)` — no response (publish handler, or unknown type).
    /// - `Err(_)` — dispatch or encoding error.
    fn dispatch(
        &mut self,
        type_id: u32,
        payload: &[u8],
        envelope: &Envelope,
        response_buf: &mut [u8],
    ) -> Result<Option<(u32, usize)>, BackplaneError>;
}
