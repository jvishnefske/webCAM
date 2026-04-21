//! # pubsub
//!
//! A `no_std`-compatible pub/sub messaging library for embedded systems.
//! Supports CAN, LIN, PMBus, and IP transports with globally unique
//! hierarchical node addressing and CBOR payload encoding.
//!
//! ## Quick start
//!
//! ```
//! use pubsub::{NodeAddr, TopicId, Node, Transport, TransportError};
//! use pubsub::frame::Frame;
//!
//! // --- Define a loopback transport for demo purposes ---
//! struct Loopback { queue: Vec<Frame> }
//! impl Loopback { fn new() -> Self { Self { queue: Vec::new() } } }
//! impl Transport for Loopback {
//!     fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
//!         self.queue.push(frame.clone());
//!         Ok(())
//!     }
//!     fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
//!         if let Some(f) = self.queue.pop() { *buf = f; Ok(true) } else { Ok(false) }
//!     }
//!     fn mtu(&self) -> usize { 64 }
//! }
//!
//! // --- Create two nodes on the same loopback bus ---
//! let temp_topic = TopicId::from_name("motor_temp");
//!
//! // Publisher node (bus 1, device 1, endpoint 0)
//! let mut publisher: Node<Loopback, 8> = Node::new(
//!     NodeAddr::new(1, 1, 0),
//!     Loopback::new(),
//! );
//!
//! // Publish a temperature reading (raw f32 bytes)
//! let temp: f32 = 42.5;
//! publisher.publish(temp_topic, &temp.to_le_bytes()).unwrap();
//!
//! // The frame is now in the loopback queue — transfer it
//! let mut frame = Frame::new(NodeAddr::new(0,0,0), NodeAddr::new(0,0,0), TopicId::from_raw(0));
//! assert!(publisher.transport_mut().recv(&mut frame).unwrap());
//!
//! // Subscriber node (bus 1, device 2, endpoint 0)
//! let mut subscriber: Node<Loopback, 8> = Node::new(
//!     NodeAddr::new(1, 2, 0),
//!     Loopback::new(),
//! );
//!
//! // Subscribe to the temperature topic
//! fn on_temp(source: NodeAddr, _topic: TopicId, payload: &[u8]) {
//!     let temp = f32::from_le_bytes(payload[..4].try_into().unwrap());
//!     assert!((temp - 42.5).abs() < f32::EPSILON);
//! }
//! subscriber.subscribe(temp_topic, on_temp).unwrap();
//!
//! // Deliver the frame to the subscriber
//! subscriber.transport_mut().send(&frame).unwrap();
//! let dispatched = subscriber.poll().unwrap();
//! assert_eq!(dispatched, 1);
//! ```
//!
//! ## Multi-bus nodes
//!
//! Use [`CompositeTransport`] to bridge two buses:
//!
//! ```
//! use pubsub::{NodeAddr, Node, CompositeTransport, Transport, TransportError};
//! use pubsub::frame::Frame;
//!
//! struct NullTransport;
//! impl Transport for NullTransport {
//!     fn send(&mut self, _: &Frame) -> Result<(), TransportError> { Ok(()) }
//!     fn recv(&mut self, _: &mut Frame) -> Result<bool, TransportError> { Ok(false) }
//!     fn mtu(&self) -> usize { 64 }
//! }
//!
//! let bus_a = NullTransport;
//! let bus_b = NullTransport;
//! let multi = CompositeTransport::new(bus_a, bus_b);
//! let node: Node<_, 16> = Node::new(NodeAddr::new(1, 1, 0), multi);
//! assert_eq!(node.addr(), NodeAddr::new(1, 1, 0));
//! ```
//!
//! ## CBOR payloads
//!
//! ```
//! use pubsub::payload;
//!
//! let mut buf = [0u8; 64];
//! let len = payload::encode(&42u32, &mut buf).unwrap();
//! let decoded: u32 = payload::decode(&buf[..len]).unwrap();
//! assert_eq!(decoded, 42);
//! ```

#![no_std]

#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod addr;
pub mod broker;
pub mod discovery;
pub mod frame;
pub mod node;
pub mod payload;
pub mod router;
pub mod topic;
pub mod transport;

// Re-export key types at crate root for convenience.
pub use addr::NodeAddr;
pub use broker::{Broker, BrokerError, MessageHandler, SubHandle};
pub use discovery::{SubscribeAnnouncement, SUBSCRIBE_TOPIC, UNSUBSCRIBE_TOPIC};
pub use frame::{Frame, FrameError, MAX_FRAME_PAYLOAD};
pub use node::{CompositeTransport, Node};
pub use router::{MeshError, MeshRouter, PollOutcome};
pub use topic::TopicId;
pub use transport::{Transport, TransportError};
