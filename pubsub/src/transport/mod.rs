//! Transport abstraction for sending and receiving frames.

use crate::frame::Frame;

/// Errors that can occur during transport operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// The send operation failed.
    SendFailed,
    /// The receive operation failed.
    RecvFailed,
    /// The transport is not ready (e.g. buffer full, not initialised).
    NotReady,
    /// Frame exceeds the transport's MTU.
    FrameTooLarge,
    /// Underlying bus error (CAN, LIN, etc.).
    BusError,
}

/// Trait implemented by all message transports (CAN, LIN, PMBus, IP, etc.).
pub trait Transport {
    /// Send a frame over this transport.
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError>;

    /// Try to receive a frame. Returns `Ok(true)` if a frame was written into
    /// `buf`, `Ok(false)` if no frame is available, or `Err` on failure.
    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError>;

    /// Maximum transmission unit (payload bytes) for this transport.
    fn mtu(&self) -> usize;
}

#[cfg(feature = "can")]
pub mod can;

#[cfg(feature = "lin")]
pub mod lin;

#[cfg(feature = "pmbus")]
pub mod pmbus;

#[cfg(feature = "ip")]
pub mod ip;
