#![allow(clippy::expect_used)]
//! Two-node pub/sub over loopback UDP.

use std::net::Ipv4Addr;
use std::thread;
use std::time::Duration;

use hil_backplane::envelope::{Envelope, MessageKind, ENVELOPE_SIZE};
use hil_backplane::message::{type_id_hash, BackplaneMessage};
use hil_backplane::node::Node;
use hil_backplane::node_id::NodeId;
use hil_backplane::transport::udp::UdpTransport;
use hil_backplane::transport::Transport;

/// A simple test message for pub/sub.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct SensorReading {
    #[n(0)]
    sensor_id: u32,
    #[n(1)]
    value: i32,
}

impl BackplaneMessage for SensorReading {
    const TYPE_ID: u32 = type_id_hash("test::SensorReading");
}

/// Helper: find an available port by binding to port 0.
fn available_port() -> u16 {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").expect("bind ephemeral");
    sock.local_addr().expect("local addr").port()
}

#[test]
fn publish_received_by_subscriber() {
    let port = available_port();
    let multicast_ip = Ipv4Addr::new(239, 255, 77, 88);

    // Publisher node.
    let transport_pub = UdpTransport::new(multicast_ip, port).expect("pub transport");
    let mut publisher = Node::new(NodeId::new(1), transport_pub);

    // Subscriber node (separate socket on same port).
    let transport_sub = UdpTransport::new(multicast_ip, port).expect("sub transport");
    let mut subscriber = Node::new(NodeId::new(2), transport_sub);

    let received = std::sync::Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();

    subscriber.register_handler::<SensorReading, _>(move |msg| {
        *received_clone.lock().expect("lock") = Some((msg.sensor_id, msg.value));
        Ok(None)
    });

    // Publish a message.
    let reading = SensorReading {
        sensor_id: 5,
        value: -100,
    };
    publisher.publish(&reading).expect("publish");

    // Give the multicast a moment to arrive, then poll.
    thread::sleep(Duration::from_millis(50));

    // Poll subscriber several times to receive.
    for _ in 0..10 {
        let _ = subscriber.poll();
        thread::sleep(Duration::from_millis(10));
    }

    let result = received.lock().expect("lock").take();
    assert_eq!(result, Some((5, -100)));
}

#[test]
fn publish_envelope_has_correct_fields() {
    let port = available_port();
    let multicast_ip = Ipv4Addr::new(239, 255, 77, 88);

    let transport_pub = UdpTransport::new(multicast_ip, port).expect("pub transport");
    let mut publisher = Node::new(NodeId::new(10), transport_pub);

    let mut transport_raw = UdpTransport::new(multicast_ip, port).expect("raw transport");

    let reading = SensorReading {
        sensor_id: 1,
        value: 42,
    };
    publisher.publish(&reading).expect("publish");

    thread::sleep(Duration::from_millis(50));

    let mut buf = [0u8; 1500];
    let mut received = false;
    for _ in 0..10 {
        if let Some((n, _addr)) = transport_raw.recv_from(&mut buf).expect("recv") {
            let env = Envelope::from_bytes(&buf[..n]).expect("decode envelope");
            assert_eq!(env.type_id, SensorReading::TYPE_ID);
            assert_eq!(env.source, NodeId::new(10));
            assert_eq!(env.kind, MessageKind::Publish);

            // Decode payload.
            let payload = &buf[ENVELOPE_SIZE..n];
            let msg: SensorReading = minicbor::decode(payload).expect("decode payload");
            assert_eq!(msg.sensor_id, 1);
            assert_eq!(msg.value, 42);
            received = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(received, "should have received the published message");
}
