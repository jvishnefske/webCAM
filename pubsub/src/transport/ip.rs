//! UDP/IP transport for the pub/sub layer.
//!
//! Each pubsub [`Frame`] is sent as a single UDP datagram -- no
//! fragmentation is required since a serialised frame (header + max
//! payload = 75 bytes) fits comfortably within a single datagram.
//!
//! This module is gated on `#[cfg(feature = "ip")]` which implies `std`.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use crate::frame::{Frame, FRAME_HEADER_SIZE, MAX_FRAME_PAYLOAD};

use super::{Transport, TransportError};

/// Maximum wire size of a serialised frame.
const MAX_WIRE_SIZE: usize = FRAME_HEADER_SIZE + MAX_FRAME_PAYLOAD;

/// UDP-based transport.
///
/// The socket is set to non-blocking mode in the constructor so that
/// [`Transport::recv`] returns `Ok(false)` immediately when no data is
/// available.
pub struct IpTransport {
    socket: UdpSocket,
    peer: SocketAddr,
    recv_buf: [u8; 256],
}

impl IpTransport {
    /// Create a unicast transport that binds to `bind_addr` and sends
    /// datagrams to `peer_addr`.
    ///
    /// Both addresses are parsed as `"host:port"` strings (e.g.
    /// `"127.0.0.1:9000"`).
    pub fn unicast(bind_addr: &str, peer_addr: &str) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(bind_addr).map_err(|_| TransportError::BusError)?;
        socket
            .set_nonblocking(true)
            .map_err(|_| TransportError::BusError)?;

        let peer: SocketAddr = peer_addr.parse().map_err(|_| TransportError::BusError)?;

        Ok(Self {
            socket,
            peer,
            recv_buf: [0u8; 256],
        })
    }

    /// Create a multicast transport.
    ///
    /// Binds to `bind_addr` (typically `"0.0.0.0:<port>"`), joins the
    /// specified `multicast_group`, and sends datagrams to
    /// `multicast_group:port`.
    pub fn multicast(
        bind_addr: &str,
        multicast_group: &str,
        port: u16,
    ) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(bind_addr).map_err(|_| TransportError::BusError)?;
        socket
            .set_nonblocking(true)
            .map_err(|_| TransportError::BusError)?;

        let group_ip: Ipv4Addr = multicast_group
            .parse()
            .map_err(|_| TransportError::BusError)?;

        socket
            .join_multicast_v4(&group_ip, &Ipv4Addr::UNSPECIFIED)
            .map_err(|_| TransportError::BusError)?;

        let peer = SocketAddr::new(group_ip.into(), port);

        Ok(Self {
            socket,
            peer,
            recv_buf: [0u8; 256],
        })
    }
}

impl Transport for IpTransport {
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
        let mut wire = [0u8; MAX_WIRE_SIZE];
        let n = frame
            .to_bytes(&mut wire)
            .map_err(|_| TransportError::FrameTooLarge)?;
        self.socket
            .send_to(&wire[..n], self.peer)
            .map_err(|_| TransportError::SendFailed)?;
        Ok(())
    }

    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
        match self.socket.recv_from(&mut self.recv_buf) {
            Ok((n, _src)) => {
                let decoded = Frame::from_bytes(&self.recv_buf[..n])
                    .map_err(|_| TransportError::RecvFailed)?;
                *buf = decoded;
                Ok(true)
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(_) => Err(TransportError::RecvFailed),
        }
    }

    fn mtu(&self) -> usize {
        MAX_FRAME_PAYLOAD
    }
}

#[cfg(test)]
mod tests {
    use std::format;
    use std::string::String;
    use std::vec::Vec;

    use super::*;
    use crate::addr::NodeAddr;
    use crate::topic::TopicId;

    /// Helper: bind to an ephemeral port on localhost and return the address
    /// string and the allocated port.
    fn ephemeral_addr() -> (String, u16) {
        // Bind to port 0 to let the OS pick a free port, then close it.
        let tmp = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral");
        let port = tmp.local_addr().expect("local_addr").port();
        drop(tmp);
        (format!("127.0.0.1:{port}"), port)
    }

    #[test]
    fn unicast_send_recv_roundtrip() {
        let (addr_a, _) = ephemeral_addr();
        let (addr_b, _) = ephemeral_addr();

        let mut tx = IpTransport::unicast(&addr_a, &addr_b).expect("tx bind");
        let mut rx = IpTransport::unicast(&addr_b, &addr_a).expect("rx bind");

        let src = NodeAddr::new(1, 2, 3);
        let dst = NodeAddr::new(4, 5, 6);
        let topic = TopicId::from_name("test/roundtrip");

        let mut frame = Frame::new(src, dst, topic);
        frame
            .set_payload(&[0xDE, 0xAD, 0xBE, 0xEF])
            .expect("set_payload");

        tx.send(&frame).expect("send");

        // Spin briefly until the datagram arrives (non-blocking).
        let mut received = Frame::new(
            NodeAddr::BROADCAST,
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        let mut got = false;
        for _ in 0..1000 {
            if rx.recv(&mut received).expect("recv") {
                got = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(got, "did not receive frame within timeout");
        assert_eq!(received.source, src);
        assert_eq!(received.destination, dst);
        assert_eq!(received.topic, topic);
        assert_eq!(received.len, 4);
        assert_eq!(received.payload_slice(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn recv_returns_false_when_empty() {
        let (addr, _) = ephemeral_addr();
        let mut t = IpTransport::unicast(&addr, "127.0.0.1:1").expect("bind");

        let mut frame = Frame::new(
            NodeAddr::BROADCAST,
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        let got = t.recv(&mut frame).expect("recv on empty socket");
        assert!(!got);
    }

    #[test]
    fn mtu_is_max_frame_payload() {
        let (addr, _) = ephemeral_addr();
        let t = IpTransport::unicast(&addr, "127.0.0.1:1").expect("bind");
        assert_eq!(t.mtu(), MAX_FRAME_PAYLOAD);
    }

    #[test]
    fn unicast_empty_payload() {
        let (addr_a, _) = ephemeral_addr();
        let (addr_b, _) = ephemeral_addr();

        let mut tx = IpTransport::unicast(&addr_a, &addr_b).expect("tx bind");
        let mut rx = IpTransport::unicast(&addr_b, &addr_a).expect("rx bind");

        let frame = Frame::new(
            NodeAddr::new(0, 0, 1),
            NodeAddr::BROADCAST,
            TopicId::from_name("empty"),
        );

        tx.send(&frame).expect("send empty");

        let mut received = Frame::new(
            NodeAddr::BROADCAST,
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        let mut got = false;
        for _ in 0..1000 {
            if rx.recv(&mut received).expect("recv") {
                got = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(got, "did not receive empty-payload frame");
        assert_eq!(received.len, 0);
        assert_eq!(received.payload_slice(), &[]);
    }

    #[test]
    fn unicast_max_payload() {
        let (addr_a, _) = ephemeral_addr();
        let (addr_b, _) = ephemeral_addr();

        let mut tx = IpTransport::unicast(&addr_a, &addr_b).expect("tx bind");
        let mut rx = IpTransport::unicast(&addr_b, &addr_a).expect("rx bind");

        let mut frame = Frame::new(
            NodeAddr::new(0xAA, 0xBB, 0xCC),
            NodeAddr::new(0xDD, 0xEE, 0xFF),
            TopicId::from_name("big"),
        );
        let big_payload: Vec<u8> = (0..MAX_FRAME_PAYLOAD as u8).collect();
        frame.set_payload(&big_payload).expect("set max payload");

        tx.send(&frame).expect("send max");

        let mut received = Frame::new(
            NodeAddr::BROADCAST,
            NodeAddr::BROADCAST,
            TopicId::from_raw(0),
        );
        let mut got = false;
        for _ in 0..1000 {
            if rx.recv(&mut received).expect("recv") {
                got = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(got, "did not receive max-payload frame");
        assert_eq!(received.len as usize, MAX_FRAME_PAYLOAD);
        assert_eq!(received.payload_slice(), &big_payload[..]);
    }

    #[test]
    fn bad_bind_address_returns_error() {
        let result = IpTransport::unicast("not_an_address", "127.0.0.1:9999");
        assert!(result.is_err());
    }

    #[test]
    fn bad_peer_address_returns_error() {
        let (addr, _) = ephemeral_addr();
        let result = IpTransport::unicast(&addr, "not_valid");
        assert!(result.is_err());
    }
}
