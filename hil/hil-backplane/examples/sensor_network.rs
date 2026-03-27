#![allow(clippy::expect_used)]
//! Example: two-node sensor network over UDP multicast.
//!
//! Demonstrates adding an Ethernet message bus between embedded nodes
//! using `hil-backplane`. A sensor node publishes temperature readings
//! via multicast, and an observer node subscribes to them.
//!
//! The same message types and wire format work identically on a
//! `no_std` embedded target (RP2040 with CDC NCM Ethernet) — only the
//! transport layer differs.
//!
//! Run with:
//! ```sh
//! cargo run -p hil-backplane --example sensor_network
//! ```

use std::thread;
use std::time::Duration;

use hil_backplane::error::BackplaneError;
use hil_backplane::message::{type_id_hash, BackplaneMessage};
use hil_backplane::node::Node;
use hil_backplane::node_id::NodeId;
use hil_backplane::transport::udp::UdpTransport;

// ---------------------------------------------------------------------------
// Step 1: Define your application messages.
//
// Each message implements `BackplaneMessage` with a unique TYPE_ID
// derived from its type path via FNV-1a hashing. The CBOR encoding
// is handled by minicbor's derive macros for simple types.
// ---------------------------------------------------------------------------

/// A temperature reading published by a sensor node.
#[derive(Debug, minicbor::Encode, minicbor::Decode)]
struct TemperatureReading {
    /// Sensor channel identifier.
    #[n(0)]
    channel: u32,
    /// Temperature in millidegrees Celsius (integer, no floats).
    #[n(1)]
    millidegrees_c: i32,
}

impl BackplaneMessage for TemperatureReading {
    const TYPE_ID: u32 = type_id_hash("example::TemperatureReading");
}

fn main() {
    // Use a non-default port to avoid conflicts with other instances.
    let port = 5899;
    let multicast_ip = hil_backplane::transport::udp::DEFAULT_MULTICAST_ADDR;

    // ---------------------------------------------------------------------------
    // Step 2: Create nodes with unique IDs and a shared multicast group.
    //
    // Each node binds to the same multicast group so published messages
    // reach all subscribers. For request/response, use separate ports
    // (see the req_resp test for that pattern).
    // ---------------------------------------------------------------------------

    let sensor_transport = UdpTransport::new(multicast_ip, port).expect("sensor transport");
    let mut sensor_node = Node::new(NodeId::new(1), sensor_transport);

    let observer_transport = UdpTransport::new(multicast_ip, port).expect("observer transport");
    let mut observer_node = Node::new(NodeId::new(2), observer_transport);

    // ---------------------------------------------------------------------------
    // Step 3: Register handlers on the observer.
    //
    // Handlers are closures that receive the deserialized message and
    // return an optional response. For pub/sub, return `Ok(None)`.
    // For request/response, return `Ok(Some((TYPE_ID, encoded_bytes)))`.
    // ---------------------------------------------------------------------------

    observer_node.register_handler::<TemperatureReading, _>(|reading| {
        println!(
            "  observer received: channel={} temp={}.{:03} C",
            reading.channel,
            reading.millidegrees_c / 1000,
            (reading.millidegrees_c % 1000).unsigned_abs(),
        );
        Ok(None)
    });

    // ---------------------------------------------------------------------------
    // Step 4: Publish messages from the sensor node.
    //
    // `publish` multicasts the message to all nodes on the group.
    // The 17-byte envelope header is prepended automatically.
    // ---------------------------------------------------------------------------

    println!("publishing 3 temperature readings...");
    let readings = [
        TemperatureReading {
            channel: 0,
            millidegrees_c: 25_500,
        },
        TemperatureReading {
            channel: 1,
            millidegrees_c: 30_125,
        },
        TemperatureReading {
            channel: 0,
            millidegrees_c: -10_750,
        },
    ];
    for reading in &readings {
        sensor_node.publish(reading).expect("publish");
    }

    // ---------------------------------------------------------------------------
    // Step 5: Poll the observer to process incoming messages.
    //
    // In a real embedded application, `poll` runs in an async task or
    // main loop. Here we sleep briefly then poll to let multicast arrive.
    // ---------------------------------------------------------------------------

    thread::sleep(Duration::from_millis(50));
    println!("polling observer...");
    loop {
        match observer_node.poll() {
            Ok(true) => continue,
            Ok(false) => break,
            Err(BackplaneError::InvalidEnvelope) => continue,
            Err(e) => {
                eprintln!("poll error: {e}");
                break;
            }
        }
    }

    println!("done.");
}
