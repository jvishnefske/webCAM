//! UDP multicast + unicast transport.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use crate::error::BackplaneError;
use crate::transport::Transport;

/// Default multicast group address.
pub const DEFAULT_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 77, 88);

/// Default multicast port.
pub const DEFAULT_PORT: u16 = 5877;

/// UDP transport using IPv4 multicast for pub/sub and unicast for
/// request/response.
pub struct UdpTransport {
    socket: UdpSocket,
    multicast_addr: SocketAddr,
}

impl UdpTransport {
    /// Creates a new UDP transport bound to `0.0.0.0:{port}`, joined
    /// to the specified multicast group.
    ///
    /// Sets `SO_REUSEADDR` so multiple nodes can bind the same port
    /// on one host (common in tests and co-located deployments).
    pub fn new(multicast_ip: Ipv4Addr, port: u16) -> Result<Self, BackplaneError> {
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);

        let sock = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        sock.set_reuse_address(true)?;
        #[cfg(unix)]
        sock.set_reuse_port(true)?;
        sock.set_nonblocking(true)?;
        sock.set_multicast_if_v4(&Ipv4Addr::LOCALHOST)?;
        sock.bind(&socket2::SockAddr::from(bind_addr))?;

        let socket: UdpSocket = sock.into();
        socket.join_multicast_v4(&multicast_ip, &Ipv4Addr::LOCALHOST)?;
        socket.set_multicast_loop_v4(true)?;

        let multicast_addr = SocketAddr::V4(SocketAddrV4::new(multicast_ip, port));

        Ok(Self {
            socket,
            multicast_addr,
        })
    }

    /// Creates a transport with default multicast settings
    /// (`239.255.77.88:5877`).
    pub fn with_defaults() -> Result<Self, BackplaneError> {
        Self::new(DEFAULT_MULTICAST_ADDR, DEFAULT_PORT)
    }

    /// Returns the local address this transport is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, BackplaneError> {
        Ok(self.socket.local_addr()?)
    }
}

impl Transport for UdpTransport {
    fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<(), BackplaneError> {
        self.socket.send_to(buf, addr)?;
        Ok(())
    }

    fn multicast(&self, buf: &[u8]) -> Result<(), BackplaneError> {
        self.socket.send_to(buf, self.multicast_addr)?;
        Ok(())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddr)>, BackplaneError> {
        match self.socket.recv_from(buf) {
            Ok((n, addr)) => Ok(Some((n, addr))),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(BackplaneError::Transport(e)),
        }
    }
}
