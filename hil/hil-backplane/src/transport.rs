//! Transport abstraction for sending and receiving datagrams.

#[cfg(feature = "std")]
pub mod udp;

#[cfg(feature = "std")]
use crate::error::BackplaneError;

/// Abstraction over the datagram transport layer.
///
/// Implementations provide send/receive of raw byte buffers.
/// Addresses are represented as `std::net::SocketAddr`.
#[cfg(feature = "std")]
pub trait Transport {
    /// Sends a datagram to a specific address.
    fn send_to(&self, buf: &[u8], addr: std::net::SocketAddr) -> Result<(), BackplaneError>;

    /// Sends a datagram to the multicast group.
    fn multicast(&self, buf: &[u8]) -> Result<(), BackplaneError>;

    /// Receives a datagram, returning the number of bytes read and the
    /// sender's address. Non-blocking: returns `Ok(None)` if no data is
    /// available.
    fn recv_from(
        &mut self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, std::net::SocketAddr)>, BackplaneError>;
}
