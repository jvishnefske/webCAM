#![allow(clippy::expect_used)]
//! Request/response correlation over loopback UDP.

use std::net::Ipv4Addr;
use std::thread;
use std::time::Duration;

use hil_backplane::message::{type_id_hash, BackplaneMessage};
use hil_backplane::node::Node;
use hil_backplane::node_id::NodeId;
use hil_backplane::transport::udp::UdpTransport;

/// A ping request message.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct Ping {
    #[n(0)]
    payload: u32,
}

impl BackplaneMessage for Ping {
    const TYPE_ID: u32 = type_id_hash("test::Ping");
}

/// A pong response message.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct Pong {
    #[n(0)]
    payload: u32,
}

impl BackplaneMessage for Pong {
    const TYPE_ID: u32 = type_id_hash("test::Pong");
}

/// Helper: find an available port by binding to port 0.
fn available_port() -> u16 {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").expect("bind ephemeral");
    sock.local_addr().expect("local addr").port()
}

#[test]
fn request_response_roundtrip() {
    let multicast_ip = Ipv4Addr::new(239, 255, 77, 88);

    // Use separate ports for server and client so unicast delivery is unambiguous.
    let server_port = available_port();
    let client_port = available_port();

    // Server node.
    let transport_server = UdpTransport::new(multicast_ip, server_port).expect("server transport");
    let mut server = Node::new(NodeId::new(1), transport_server);

    // Register handler: Ping -> Pong, echoing the payload.
    server.register_handler::<Ping, _>(|ping| {
        let pong = Pong {
            payload: ping.payload,
        };
        let encoded = minicbor::to_vec(&pong)
            .map_err(|_| hil_backplane::error::BackplaneError::EncodeFailed)?;
        Ok(Some((Pong::TYPE_ID, encoded)))
    });

    // Client node on a different port.
    let transport_client = UdpTransport::new(multicast_ip, client_port).expect("client transport");
    let mut client = Node::new(NodeId::new(2), transport_client);

    // Spawn server polling in a background thread.
    let server_handle = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            let _ = server.poll();
            thread::yield_now();
        }
    });

    // Small delay for the server thread to start polling.
    thread::sleep(Duration::from_millis(20));

    // Send request to server's port on localhost.
    let server_addr: std::net::SocketAddr = format!("127.0.0.1:{server_port}")
        .parse()
        .expect("parse addr");

    let ping = Ping { payload: 12345 };
    let pong: Pong = client
        .request(&ping, server_addr, Duration::from_secs(1))
        .expect("request should succeed");

    assert_eq!(pong.payload, 12345);

    server_handle.join().expect("server thread join");
}

#[test]
fn request_timeout() {
    let port = available_port();
    let multicast_ip = Ipv4Addr::new(239, 255, 77, 88);

    // Client with no server.
    let transport = UdpTransport::new(multicast_ip, port).expect("transport");
    let mut client = Node::new(NodeId::new(1), transport);

    let ping = Ping { payload: 0 };
    let result: Result<Pong, _> = client.request(
        &ping,
        "127.0.0.1:19999".parse().expect("parse addr"),
        Duration::from_millis(100),
    );

    assert!(
        matches!(result, Err(hil_backplane::error::BackplaneError::Timeout)),
        "expected Timeout error, got: {result:?}"
    );
}
