//! Top-level Node API for the pubsub crate.
//!
//! A [`Node`] combines a [`NodeAddr`], [`Broker`], and [`Transport`] into a
//! single ergonomic type suitable for use in a main-loop polling architecture.
//!
//! For nodes connected to multiple buses, [`CompositeTransport`] merges two
//! transports so that sends go out on both and polls drain both.

use crate::addr::NodeAddr;
use crate::broker::{Broker, BrokerError, MessageHandler, SubHandle};
use crate::frame::Frame;
use crate::topic::TopicId;
use crate::transport::{Transport, TransportError};

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// A pub/sub node with a single transport.
///
/// This is the primary high-level API.  Create a `Node`, subscribe to topics,
/// publish messages, and call [`poll`](Self::poll) in your main loop.
///
/// ```
/// use pubsub::{NodeAddr, TopicId, Node, Transport, TransportError};
/// use pubsub::frame::Frame;
///
/// // Minimal no-op transport for demonstration
/// struct Noop;
/// impl Transport for Noop {
///     fn send(&mut self, _: &Frame) -> Result<(), TransportError> { Ok(()) }
///     fn recv(&mut self, _: &mut Frame) -> Result<bool, TransportError> { Ok(false) }
///     fn mtu(&self) -> usize { 64 }
/// }
///
/// fn handler(_src: NodeAddr, _topic: TopicId, _payload: &[u8]) {}
///
/// let mut node: Node<Noop, 16> = Node::new(
///     NodeAddr::new(1, 2, 0),
///     Noop,
/// );
/// node.subscribe(TopicId::from_name("temp"), handler).unwrap();
/// // In real code: loop { node.poll().unwrap(); }
/// assert_eq!(node.poll().unwrap(), 0); // nothing to receive
/// ```
pub struct Node<T: Transport, const MAX_SUBS: usize> {
    broker: Broker<MAX_SUBS>,
    transport: T,
}

impl<T: Transport, const MAX_SUBS: usize> Node<T, MAX_SUBS> {
    /// Create a new node with the given address and transport.
    pub fn new(addr: NodeAddr, transport: T) -> Self {
        Self {
            broker: Broker::new(addr),
            transport,
        }
    }

    /// Get this node's address.
    pub fn addr(&self) -> NodeAddr {
        self.broker.local_addr()
    }

    /// Subscribe to a topic.
    pub fn subscribe(
        &mut self,
        topic: TopicId,
        handler: MessageHandler,
    ) -> Result<SubHandle, BrokerError> {
        self.broker.subscribe(topic, handler)
    }

    /// Unsubscribe from a previously subscribed topic.
    pub fn unsubscribe(&mut self, handle: SubHandle) -> Result<(), BrokerError> {
        self.broker.unsubscribe(handle)
    }

    /// Publish to all subscribers of a topic (broadcast).
    pub fn publish(&mut self, topic: TopicId, payload: &[u8]) -> Result<(), BrokerError> {
        self.broker.publish(&mut self.transport, topic, payload)
    }

    /// Publish to a specific node (unicast).
    pub fn publish_to(
        &mut self,
        dest: NodeAddr,
        topic: TopicId,
        payload: &[u8],
    ) -> Result<(), BrokerError> {
        self.broker
            .publish_to(&mut self.transport, dest, topic, payload)
    }

    /// Poll for incoming messages and dispatch to handlers.
    ///
    /// Call this in your main loop.  Returns the number of frames processed.
    pub fn poll(&mut self) -> Result<usize, BrokerError> {
        self.broker.poll(&mut self.transport)
    }

    /// Access the underlying transport (shared reference).
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Access the underlying transport (mutable reference).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

// ---------------------------------------------------------------------------
// CompositeTransport
// ---------------------------------------------------------------------------

/// Combines two transports so a node can publish/subscribe across multiple
/// buses.
///
/// - **Send**: the frame goes out on **both** transports.  If the first
///   transport fails, the error is returned immediately and the second
///   transport is not attempted.
/// - **Receive / poll**: both transports are polled.  `recv` returns `true` as
///   soon as either transport produces a frame.  Transport `a` is polled
///   first.
/// - **MTU**: the minimum of both underlying MTUs.
pub struct CompositeTransport<A: Transport, B: Transport> {
    /// First transport.
    pub a: A,
    /// Second transport.
    pub b: B,
}

impl<A: Transport, B: Transport> CompositeTransport<A, B> {
    /// Create a composite from two transports.
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Transport, B: Transport> Transport for CompositeTransport<A, B> {
    fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
        self.a.send(frame)?;
        self.b.send(frame)?;
        Ok(())
    }

    fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
        if self.a.recv(buf)? {
            return Ok(true);
        }
        self.b.recv(buf)
    }

    fn mtu(&self) -> usize {
        let ma = self.a.mtu();
        let mb = self.b.mtu();
        if ma < mb {
            ma
        } else {
            mb
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::NodeAddr;
    use crate::frame::{Frame, MAX_FRAME_PAYLOAD};
    use crate::topic::TopicId;
    use crate::transport::{Transport, TransportError};

    use core::sync::atomic::{AtomicU32, Ordering};

    // -- Mock transport ---------------------------------------------------

    /// A simple mock transport backed by a fixed-size ring of frames.
    struct MockTransport {
        /// Frames written via `send`.
        sent: [Option<Frame>; 16],
        sent_count: usize,
        /// Frames staged for `recv` (enqueued by test setup).
        inbox: [Option<Frame>; 16],
        inbox_head: usize,
        inbox_tail: usize,
        mtu_value: usize,
    }

    impl MockTransport {
        fn new() -> Self {
            const NONE_FRAME: Option<Frame> = None;
            Self {
                sent: [NONE_FRAME; 16],
                sent_count: 0,
                inbox: [NONE_FRAME; 16],
                inbox_head: 0,
                inbox_tail: 0,
                mtu_value: MAX_FRAME_PAYLOAD,
            }
        }

        fn with_mtu(mtu: usize) -> Self {
            let mut t = Self::new();
            t.mtu_value = mtu;
            t
        }

        /// Stage a frame that will be returned by the next `recv` call.
        fn enqueue(&mut self, frame: Frame) {
            self.inbox[self.inbox_tail] = Some(frame);
            self.inbox_tail = (self.inbox_tail + 1) % 16;
        }

        /// Return how many frames were sent.
        fn sent_count(&self) -> usize {
            self.sent_count
        }

        /// Return a reference to the i-th sent frame.
        fn sent_frame(&self, i: usize) -> &Frame {
            self.sent[i].as_ref().expect("no frame at index")
        }
    }

    impl Transport for MockTransport {
        fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
            if self.sent_count >= 16 {
                return Err(TransportError::SendFailed);
            }
            self.sent[self.sent_count] = Some(frame.clone());
            self.sent_count += 1;
            Ok(())
        }

        fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
            if self.inbox_head == self.inbox_tail {
                return Ok(false);
            }
            if let Some(frame) = self.inbox[self.inbox_head].take() {
                *buf = frame;
                self.inbox_head = (self.inbox_head + 1) % 16;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn mtu(&self) -> usize {
            self.mtu_value
        }
    }

    // -- Helpers -----------------------------------------------------------

    static HANDLER_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
    static HANDLER_LAST_TOPIC: AtomicU32 = AtomicU32::new(0);

    fn reset_handler_state() {
        HANDLER_CALL_COUNT.store(0, Ordering::SeqCst);
        HANDLER_LAST_TOPIC.store(0, Ordering::SeqCst);
    }

    fn test_handler(source: NodeAddr, topic: TopicId, _payload: &[u8]) {
        let _ = source;
        HANDLER_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        HANDLER_LAST_TOPIC.store(topic.as_u32(), Ordering::SeqCst);
    }

    fn make_incoming_frame(
        source: NodeAddr,
        dest: NodeAddr,
        topic: TopicId,
        payload: &[u8],
    ) -> Frame {
        let mut f = Frame::new(source, dest, topic);
        f.set_payload(payload).unwrap();
        f
    }

    // -- Node basic tests -------------------------------------------------

    #[test]
    fn node_new_returns_correct_addr() {
        let addr = NodeAddr::new(1, 2, 3);
        let node: Node<MockTransport, 8> = Node::new(addr, MockTransport::new());
        assert_eq!(node.addr(), addr);
    }

    #[test]
    fn node_subscribe_and_unsubscribe() {
        let mut node: Node<MockTransport, 4> =
            Node::new(NodeAddr::new(1, 0, 0), MockTransport::new());
        let topic = TopicId::from_name("sensor/temp");
        let handle = node.subscribe(topic, test_handler).unwrap();
        assert!(node.unsubscribe(handle).is_ok());
        // Double unsubscribe should fail.
        assert_eq!(node.unsubscribe(handle), Err(BrokerError::InvalidHandle));
    }

    #[test]
    fn node_subscribe_full() {
        let mut node: Node<MockTransport, 2> =
            Node::new(NodeAddr::new(1, 0, 0), MockTransport::new());
        let t = TopicId::from_name("x");
        let _h1 = node.subscribe(t, test_handler).unwrap();
        let _h2 = node.subscribe(t, test_handler).unwrap();
        assert_eq!(
            node.subscribe(t, test_handler),
            Err(BrokerError::SubscriptionsFull)
        );
    }

    #[test]
    fn node_publish_broadcast() {
        let mut node: Node<MockTransport, 4> =
            Node::new(NodeAddr::new(1, 2, 0), MockTransport::new());
        let topic = TopicId::from_name("cmd/move");
        let payload = [0xAA, 0xBB];

        node.publish(topic, &payload).unwrap();

        assert_eq!(node.transport().sent_count(), 1);
        let sent = node.transport().sent_frame(0);
        assert_eq!(sent.source, NodeAddr::new(1, 2, 0));
        assert_eq!(sent.destination, NodeAddr::BROADCAST);
        assert_eq!(sent.topic, topic);
        assert_eq!(sent.payload_slice(), &payload);
    }

    #[test]
    fn node_publish_to_unicast() {
        let mut node: Node<MockTransport, 4> =
            Node::new(NodeAddr::new(1, 2, 0), MockTransport::new());
        let dest = NodeAddr::new(2, 3, 0);
        let topic = TopicId::from_name("ack");

        node.publish_to(dest, topic, &[42]).unwrap();

        let sent = node.transport().sent_frame(0);
        assert_eq!(sent.destination, dest);
    }

    #[test]
    fn node_publish_payload_too_large() {
        let mut node: Node<MockTransport, 4> =
            Node::new(NodeAddr::new(1, 0, 0), MockTransport::new());
        let big = [0u8; MAX_FRAME_PAYLOAD + 1];
        assert_eq!(
            node.publish(TopicId::from_name("x"), &big),
            Err(BrokerError::PayloadTooLarge)
        );
    }

    #[test]
    fn node_publish_exceeds_mtu() {
        let transport = MockTransport::with_mtu(8);
        let mut node: Node<MockTransport, 4> = Node::new(NodeAddr::new(1, 0, 0), transport);
        let payload = [0u8; 16]; // 16 > mtu of 8
        assert_eq!(
            node.publish(TopicId::from_name("x"), &payload),
            Err(BrokerError::PayloadTooLarge)
        );
    }

    // -- Full flow: subscribe, publish (loopback), poll, verify handler ---

    #[test]
    fn node_full_flow_subscribe_publish_poll() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let remote = NodeAddr::new(2, 0, 0);
        let topic = TopicId::from_name("sensor/temp");
        let payload = [10, 20, 30];

        let mut node: Node<MockTransport, 8> = Node::new(local, MockTransport::new());

        // Subscribe to the topic.
        let _h = node.subscribe(topic, test_handler).unwrap();

        // Simulate an incoming frame from a remote node addressed to us.
        let incoming = make_incoming_frame(remote, local, topic, &payload);
        node.transport_mut().enqueue(incoming);

        // Poll should dispatch to handler.
        let count = node.poll().unwrap();
        assert_eq!(count, 1);
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(HANDLER_LAST_TOPIC.load(Ordering::SeqCst), topic.as_u32());
    }

    #[test]
    fn node_poll_ignores_wrong_destination() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let other = NodeAddr::new(3, 0, 0);
        let topic = TopicId::from_name("x");

        let mut node: Node<MockTransport, 4> = Node::new(local, MockTransport::new());
        node.subscribe(topic, test_handler).unwrap();

        // Frame addressed to a different node.
        let incoming = make_incoming_frame(NodeAddr::new(2, 0, 0), other, topic, &[1]);
        node.transport_mut().enqueue(incoming);

        let count = node.poll().unwrap();
        assert_eq!(count, 0);
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn node_poll_accepts_broadcast() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let topic = TopicId::from_name("announce");

        let mut node: Node<MockTransport, 4> = Node::new(local, MockTransport::new());
        node.subscribe(topic, test_handler).unwrap();

        // Broadcast frame.
        let incoming = make_incoming_frame(NodeAddr::new(9, 0, 0), NodeAddr::BROADCAST, topic, &[]);
        node.transport_mut().enqueue(incoming);

        let count = node.poll().unwrap();
        assert_eq!(count, 1);
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn node_poll_no_matching_topic() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let subscribed = TopicId::from_name("alpha");
        let incoming_topic = TopicId::from_name("beta");

        let mut node: Node<MockTransport, 4> = Node::new(local, MockTransport::new());
        node.subscribe(subscribed, test_handler).unwrap();

        let incoming = make_incoming_frame(NodeAddr::new(2, 0, 0), local, incoming_topic, &[]);
        node.transport_mut().enqueue(incoming);

        // Frame is processed (accepted by address) but handler not called
        // because topic doesn't match.
        let count = node.poll().unwrap();
        assert_eq!(count, 1);
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn node_poll_multiple_frames() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let topic = TopicId::from_name("multi");

        let mut node: Node<MockTransport, 4> = Node::new(local, MockTransport::new());
        node.subscribe(topic, test_handler).unwrap();

        for i in 0..3u8 {
            let f = make_incoming_frame(NodeAddr::new(2, 0, i), local, topic, &[i]);
            node.transport_mut().enqueue(f);
        }

        let count = node.poll().unwrap();
        assert_eq!(count, 3);
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn node_unsubscribe_stops_delivery() {
        reset_handler_state();

        let local = NodeAddr::new(1, 0, 0);
        let topic = TopicId::from_name("unsub_test");

        let mut node: Node<MockTransport, 4> = Node::new(local, MockTransport::new());
        let h = node.subscribe(topic, test_handler).unwrap();

        // Deliver one frame.
        node.transport_mut().enqueue(make_incoming_frame(
            NodeAddr::new(2, 0, 0),
            local,
            topic,
            &[1],
        ));
        node.poll().unwrap();
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 1);

        // Unsubscribe and deliver another.
        node.unsubscribe(h).unwrap();
        node.transport_mut().enqueue(make_incoming_frame(
            NodeAddr::new(2, 0, 0),
            local,
            topic,
            &[2],
        ));
        node.poll().unwrap();
        // Count should not have increased.
        assert_eq!(HANDLER_CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn node_transport_accessors() {
        let mut node: Node<MockTransport, 4> =
            Node::new(NodeAddr::new(1, 0, 0), MockTransport::new());
        assert_eq!(node.transport().mtu_value, MAX_FRAME_PAYLOAD);
        node.transport_mut().mtu_value = 32;
        assert_eq!(node.transport().mtu_value, 32);
    }

    // -- CompositeTransport tests -----------------------------------------

    #[test]
    fn composite_send_goes_to_both() {
        let a = MockTransport::new();
        let b = MockTransport::new();
        let mut composite = CompositeTransport::new(a, b);

        let frame = make_incoming_frame(
            NodeAddr::new(1, 0, 0),
            NodeAddr::BROADCAST,
            TopicId::from_name("t"),
            &[0xAB],
        );
        composite.send(&frame).unwrap();

        assert_eq!(composite.a.sent_count(), 1);
        assert_eq!(composite.b.sent_count(), 1);
        assert_eq!(composite.a.sent_frame(0).payload_slice(), &[0xAB]);
        assert_eq!(composite.b.sent_frame(0).payload_slice(), &[0xAB]);
    }

    #[test]
    fn composite_recv_drains_a_first() {
        let mut a = MockTransport::new();
        let b = MockTransport::new();

        let topic_a = TopicId::from_name("from_a");
        let topic_b = TopicId::from_name("from_b");

        a.enqueue(make_incoming_frame(
            NodeAddr::new(2, 0, 0),
            NodeAddr::BROADCAST,
            topic_a,
            &[1],
        ));

        let mut composite = CompositeTransport::new(a, b);
        composite.b.enqueue(make_incoming_frame(
            NodeAddr::new(3, 0, 0),
            NodeAddr::BROADCAST,
            topic_b,
            &[2],
        ));

        let mut frame = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        // First recv should come from transport A.
        assert!(composite.recv(&mut frame).unwrap());
        assert_eq!(frame.topic, topic_a);
        assert_eq!(frame.payload_slice(), &[1]);

        // Second recv should come from transport B.
        assert!(composite.recv(&mut frame).unwrap());
        assert_eq!(frame.topic, topic_b);
        assert_eq!(frame.payload_slice(), &[2]);

        // No more frames.
        assert!(!composite.recv(&mut frame).unwrap());
    }

    #[test]
    fn composite_mtu_is_min() {
        let a = MockTransport::with_mtu(64);
        let b = MockTransport::with_mtu(8);
        let composite = CompositeTransport::new(a, b);
        assert_eq!(composite.mtu(), 8);

        let a2 = MockTransport::with_mtu(16);
        let b2 = MockTransport::with_mtu(32);
        let composite2 = CompositeTransport::new(a2, b2);
        assert_eq!(composite2.mtu(), 16);
    }

    #[test]
    fn composite_mtu_equal() {
        let a = MockTransport::with_mtu(24);
        let b = MockTransport::with_mtu(24);
        let composite = CompositeTransport::new(a, b);
        assert_eq!(composite.mtu(), 24);
    }

    // Private counter for composite test to avoid races with other tests
    // that share `HANDLER_CALL_COUNT`.
    static COMPOSITE_HANDLER_COUNT: AtomicU32 = AtomicU32::new(0);

    fn composite_test_handler(_source: NodeAddr, _topic: TopicId, _payload: &[u8]) {
        COMPOSITE_HANDLER_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn node_with_composite_full_flow() {
        COMPOSITE_HANDLER_COUNT.store(0, Ordering::SeqCst);

        let local = NodeAddr::new(1, 0, 0);
        let topic = TopicId::from_name("composite_test");

        let a = MockTransport::new();
        let b = MockTransport::new();
        let composite = CompositeTransport::new(a, b);

        let mut node: Node<CompositeTransport<MockTransport, MockTransport>, 8> =
            Node::new(local, composite);

        node.subscribe(topic, composite_test_handler).unwrap();

        // Publish should send on both transports.
        node.publish(topic, &[99]).unwrap();
        assert_eq!(node.transport().a.sent_count(), 1);
        assert_eq!(node.transport().b.sent_count(), 1);

        // Enqueue incoming on transport B only.
        let incoming = make_incoming_frame(NodeAddr::new(5, 0, 0), local, topic, &[77]);
        node.transport_mut().b.enqueue(incoming);

        let count = node.poll().unwrap();
        assert_eq!(count, 1);
        assert_eq!(COMPOSITE_HANDLER_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn composite_send_first_fails() {
        /// A transport that always fails to send.
        struct FailSend;

        impl Transport for FailSend {
            fn send(&mut self, _frame: &Frame) -> Result<(), TransportError> {
                Err(TransportError::SendFailed)
            }
            fn recv(&mut self, _buf: &mut Frame) -> Result<bool, TransportError> {
                Ok(false)
            }
            fn mtu(&self) -> usize {
                64
            }
        }

        let mut composite = CompositeTransport::new(FailSend, MockTransport::new());
        let frame = make_incoming_frame(
            NodeAddr::new(1, 0, 0),
            NodeAddr::BROADCAST,
            TopicId::from_raw(1),
            &[],
        );

        // Should fail because transport A fails.
        assert_eq!(composite.send(&frame), Err(TransportError::SendFailed));
        // Transport B should NOT have received the frame.
        assert_eq!(composite.b.sent_count(), 0);
    }

    #[test]
    fn composite_recv_empty() {
        let a = MockTransport::new();
        let b = MockTransport::new();
        let mut composite = CompositeTransport::new(a, b);
        let mut frame = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );
        assert!(!composite.recv(&mut frame).unwrap());
    }
}
