//! Static-capacity pub/sub message broker.
//!
//! The [`Broker`] routes incoming frames to registered [`MessageHandler`]
//! callbacks based on [`TopicId`].  It is designed for `no_std` use with no
//! heap allocation -- the subscription table is a fixed-size array.

use crate::addr::NodeAddr;
use crate::frame::{Frame, MAX_FRAME_PAYLOAD};
use crate::topic::TopicId;
use crate::transport::{Transport, TransportError};

/// Function pointer invoked when a message arrives on a subscribed topic.
pub type MessageHandler = fn(source: NodeAddr, topic: TopicId, payload: &[u8]);

/// Opaque handle returned by [`Broker::subscribe`], used to unsubscribe later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubHandle(pub(crate) usize);

/// Errors produced by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerError {
    /// The subscription table is full.
    SubscriptionsFull,
    /// The provided [`SubHandle`] is not valid or already unsubscribed.
    InvalidHandle,
    /// A transport-level error occurred.
    TransportError(TransportError),
    /// The payload is too large for a single frame or for the transport MTU.
    PayloadTooLarge,
}

impl From<TransportError> for BrokerError {
    fn from(e: TransportError) -> Self {
        BrokerError::TransportError(e)
    }
}

/// A single subscription entry.
struct Subscription {
    topic: TopicId,
    handler: MessageHandler,
    active: bool,
}

/// A no-op handler used to fill the initial array.
fn noop_handler(_source: NodeAddr, _topic: TopicId, _payload: &[u8]) {}

/// A static-capacity pub/sub message broker.
///
/// `MAX_SUBS` controls the maximum number of concurrent subscriptions.
pub struct Broker<const MAX_SUBS: usize> {
    local_addr: NodeAddr,
    subs: [Subscription; MAX_SUBS],
}

impl<const MAX_SUBS: usize> Broker<MAX_SUBS> {
    /// Create a new broker bound to the given local address.
    pub fn new(local_addr: NodeAddr) -> Self {
        const EMPTY: Subscription = Subscription {
            topic: TopicId::from_raw(0),
            handler: noop_handler,
            active: false,
        };
        Self {
            local_addr,
            subs: [EMPTY; MAX_SUBS],
        }
    }

    /// Return the local address this broker was created with.
    pub fn local_addr(&self) -> NodeAddr {
        self.local_addr
    }

    /// Subscribe to `topic` with the given `handler`.
    ///
    /// Returns a [`SubHandle`] that can be passed to [`unsubscribe`](Self::unsubscribe).
    pub fn subscribe(
        &mut self,
        topic: TopicId,
        handler: MessageHandler,
    ) -> Result<SubHandle, BrokerError> {
        for (i, slot) in self.subs.iter_mut().enumerate() {
            if !slot.active {
                slot.topic = topic;
                slot.handler = handler;
                slot.active = true;
                return Ok(SubHandle(i));
            }
        }
        Err(BrokerError::SubscriptionsFull)
    }

    /// Remove a subscription previously created with [`subscribe`](Self::subscribe).
    pub fn unsubscribe(&mut self, handle: SubHandle) -> Result<(), BrokerError> {
        let idx = handle.0;
        if idx >= MAX_SUBS || !self.subs[idx].active {
            return Err(BrokerError::InvalidHandle);
        }
        self.subs[idx].active = false;
        Ok(())
    }

    /// Publish a message to all subscribers of `topic` (broadcast).
    pub fn publish<T: Transport>(
        &self,
        transport: &mut T,
        topic: TopicId,
        payload: &[u8],
    ) -> Result<(), BrokerError> {
        self.publish_to(transport, NodeAddr::BROADCAST, topic, payload)
    }

    /// Publish a message to a specific destination node.
    pub fn publish_to<T: Transport>(
        &self,
        transport: &mut T,
        dest: NodeAddr,
        topic: TopicId,
        payload: &[u8],
    ) -> Result<(), BrokerError> {
        if payload.len() > MAX_FRAME_PAYLOAD || payload.len() > transport.mtu() {
            return Err(BrokerError::PayloadTooLarge);
        }
        let mut frame = Frame::new(self.local_addr, dest, topic);
        frame
            .set_payload(payload)
            .map_err(|_| BrokerError::PayloadTooLarge)?;
        transport.send(&frame)?;
        Ok(())
    }

    /// Poll the transport for incoming frames and dispatch to matching handlers.
    ///
    /// Frames addressed to [`NodeAddr::BROADCAST`] or to this broker's local
    /// address are accepted; all others are silently dropped.
    ///
    /// Returns the number of frames processed.
    pub fn poll<T: Transport>(&self, transport: &mut T) -> Result<usize, BrokerError> {
        let mut processed = 0usize;
        let mut frame = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );

        loop {
            match transport.recv(&mut frame) {
                Ok(true) => {
                    let dest = frame.destination;
                    if dest != NodeAddr::BROADCAST && dest != self.local_addr {
                        continue;
                    }
                    let topic = frame.topic;
                    let source = frame.source;
                    // We must extract the payload slice before iterating subs,
                    // because the handler borrows it immutably.
                    for sub in &self.subs {
                        if sub.active && sub.topic == topic {
                            (sub.handler)(source, topic, frame.payload_slice());
                        }
                    }
                    processed += 1;
                }
                Ok(false) => break,
                Err(e) => return Err(BrokerError::TransportError(e)),
            }
        }

        Ok(processed)
    }

    /// The number of currently active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subs.iter().filter(|s| s.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::MAX_FRAME_PAYLOAD;
    use core::cell::Cell;
    use std::vec;
    use std::vec::Vec;

    // -- MockTransport -------------------------------------------------------

    /// In-memory transport: stores sent frames, replays queued incoming frames.
    struct MockTransport {
        sent: Vec<Frame>,
        incoming: Vec<Frame>,
        recv_idx: usize,
        send_error: Option<TransportError>,
        recv_error: Option<TransportError>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                sent: Vec::new(),
                incoming: Vec::new(),
                recv_idx: 0,
                send_error: None,
                recv_error: None,
            }
        }

        fn push_incoming(&mut self, frame: Frame) {
            self.incoming.push(frame);
        }
    }

    impl Transport for MockTransport {
        fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
            if let Some(e) = self.send_error {
                return Err(e);
            }
            self.sent.push(frame.clone());
            Ok(())
        }

        fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
            if let Some(e) = self.recv_error {
                return Err(e);
            }
            if self.recv_idx < self.incoming.len() {
                let f = &self.incoming[self.recv_idx];
                buf.source = f.source;
                buf.destination = f.destination;
                buf.topic = f.topic;
                buf.payload = f.payload;
                buf.len = f.len;
                self.recv_idx += 1;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn mtu(&self) -> usize {
            MAX_FRAME_PAYLOAD
        }
    }

    // -- Handler capture infrastructure --------------------------------------
    //
    // MessageHandler is a plain fn pointer so we use thread-locals to record
    // invocations.

    std::thread_local! {
        static HANDLER_A_CALLS: Cell<u32> = const { Cell::new(0) };
        static HANDLER_A_PAYLOAD: core::cell::RefCell<Vec<u8>> = const { core::cell::RefCell::new(Vec::new()) };
        static HANDLER_B_CALLS: Cell<u32> = const { Cell::new(0) };
    }

    fn reset_handlers() {
        HANDLER_A_CALLS.with(|c| c.set(0));
        HANDLER_A_PAYLOAD.with(|p| p.borrow_mut().clear());
        HANDLER_B_CALLS.with(|c| c.set(0));
    }

    fn handler_a_count() -> u32 {
        HANDLER_A_CALLS.with(|c| c.get())
    }

    fn handler_a_payload() -> Vec<u8> {
        HANDLER_A_PAYLOAD.with(|p| p.borrow().clone())
    }

    fn handler_b_count() -> u32 {
        HANDLER_B_CALLS.with(|c| c.get())
    }

    fn handler_a(_source: NodeAddr, _topic: TopicId, payload: &[u8]) {
        HANDLER_A_CALLS.with(|c| c.set(c.get() + 1));
        HANDLER_A_PAYLOAD.with(|p| {
            let mut v = p.borrow_mut();
            v.clear();
            v.extend_from_slice(payload);
        });
    }

    fn handler_b(_source: NodeAddr, _topic: TopicId, _payload: &[u8]) {
        HANDLER_B_CALLS.with(|c| c.set(c.get() + 1));
    }

    // -- Constants -----------------------------------------------------------

    const LOCAL: NodeAddr = NodeAddr::new(1, 2, 3);
    const REMOTE: NodeAddr = NodeAddr::new(4, 5, 6);
    const TOPIC_TEMP: TopicId = TopicId::from_name("temperature");
    const TOPIC_RPM: TopicId = TopicId::from_name("rpm");

    // -- subscribe / unsubscribe tests ---------------------------------------

    #[test]
    fn new_broker_has_no_subscriptions() {
        let broker: Broker<8> = Broker::new(LOCAL);
        assert_eq!(broker.subscription_count(), 0);
    }

    #[test]
    fn subscribe_increases_count() {
        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        assert_eq!(broker.subscription_count(), 1);
        broker.subscribe(TOPIC_RPM, handler_a).unwrap();
        assert_eq!(broker.subscription_count(), 2);
    }

    #[test]
    fn subscribe_returns_distinct_handles() {
        let mut broker: Broker<8> = Broker::new(LOCAL);
        let h1 = broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        let h2 = broker.subscribe(TOPIC_RPM, handler_a).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn subscribe_full_returns_error() {
        let mut broker: Broker<2> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        broker.subscribe(TOPIC_RPM, handler_a).unwrap();
        let result = broker.subscribe(TopicId::from_name("extra"), handler_a);
        assert_eq!(result, Err(BrokerError::SubscriptionsFull));
    }

    #[test]
    fn unsubscribe_decreases_count() {
        let mut broker: Broker<8> = Broker::new(LOCAL);
        let h = broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        assert_eq!(broker.subscription_count(), 1);
        broker.unsubscribe(h).unwrap();
        assert_eq!(broker.subscription_count(), 0);
    }

    #[test]
    fn unsubscribe_invalid_handle() {
        let mut broker: Broker<8> = Broker::new(LOCAL);
        assert_eq!(
            broker.unsubscribe(SubHandle(42)),
            Err(BrokerError::InvalidHandle)
        );
    }

    #[test]
    fn double_unsubscribe_is_error() {
        let mut broker: Broker<8> = Broker::new(LOCAL);
        let h = broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        broker.unsubscribe(h).unwrap();
        assert_eq!(broker.unsubscribe(h), Err(BrokerError::InvalidHandle));
    }

    #[test]
    fn slot_reuse_after_unsubscribe() {
        let mut broker: Broker<2> = Broker::new(LOCAL);
        let h1 = broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        let _h2 = broker.subscribe(TOPIC_RPM, handler_a).unwrap();

        // Table is full.
        assert_eq!(
            broker.subscribe(TopicId::from_name("x"), handler_a),
            Err(BrokerError::SubscriptionsFull)
        );

        // Free a slot and reuse it.
        broker.unsubscribe(h1).unwrap();
        assert_eq!(broker.subscription_count(), 1);

        let h3 = broker
            .subscribe(TopicId::from_name("x"), handler_a)
            .unwrap();
        assert_eq!(broker.subscription_count(), 2);
        // Reused slot has the same index as h1.
        assert_eq!(h3, h1);
    }

    #[test]
    fn local_addr_accessor() {
        let addr = NodeAddr::new(3, 4, 5);
        let broker: Broker<4> = Broker::new(addr);
        assert_eq!(broker.local_addr(), addr);
    }

    // -- publish tests -------------------------------------------------------

    #[test]
    fn publish_sends_broadcast_frame() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();

        broker
            .publish(&mut transport, TOPIC_TEMP, &[1, 2, 3])
            .unwrap();

        assert_eq!(transport.sent.len(), 1);
        let f = &transport.sent[0];
        assert_eq!(f.source, LOCAL);
        assert_eq!(f.destination, NodeAddr::BROADCAST);
        assert_eq!(f.topic, TOPIC_TEMP);
        assert_eq!(f.payload_slice(), &[1, 2, 3]);
    }

    #[test]
    fn publish_to_sends_unicast_frame() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();

        broker
            .publish_to(&mut transport, REMOTE, TOPIC_RPM, &[42])
            .unwrap();

        assert_eq!(transport.sent.len(), 1);
        let f = &transport.sent[0];
        assert_eq!(f.source, LOCAL);
        assert_eq!(f.destination, REMOTE);
        assert_eq!(f.topic, TOPIC_RPM);
        assert_eq!(f.payload_slice(), &[42]);
    }

    #[test]
    fn publish_empty_payload() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        broker.publish(&mut transport, TOPIC_TEMP, &[]).unwrap();
        assert_eq!(transport.sent.len(), 1);
        assert_eq!(transport.sent[0].payload_slice(), &[]);
    }

    #[test]
    fn publish_max_payload() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        let data = [0xABu8; MAX_FRAME_PAYLOAD];
        broker.publish(&mut transport, TOPIC_TEMP, &data).unwrap();
        assert_eq!(transport.sent[0].payload_slice(), &data[..]);
    }

    #[test]
    fn publish_payload_too_large() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        let big = [0u8; MAX_FRAME_PAYLOAD + 1];
        let result = broker.publish(&mut transport, TOPIC_TEMP, &big);
        assert_eq!(result, Err(BrokerError::PayloadTooLarge));
        assert!(transport.sent.is_empty());
    }

    #[test]
    fn publish_transport_send_error() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        transport.send_error = Some(TransportError::SendFailed);

        let result = broker.publish(&mut transport, TOPIC_TEMP, &[1]);
        assert_eq!(
            result,
            Err(BrokerError::TransportError(TransportError::SendFailed))
        );
    }

    // -- poll tests ----------------------------------------------------------

    #[test]
    fn poll_returns_zero_when_no_frames() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        assert_eq!(broker.poll(&mut transport).unwrap(), 0);
    }

    #[test]
    fn poll_dispatches_matching_topic() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();
        let mut frame = Frame::new(REMOTE, LOCAL, TOPIC_TEMP);
        frame.set_payload(&[10, 20]).unwrap();
        transport.push_incoming(frame);

        let count = broker.poll(&mut transport).unwrap();
        // poll returns frames processed (1 frame), handler was called once.
        assert!(count >= 1);
        assert_eq!(handler_a_count(), 1);
        assert_eq!(handler_a_payload(), vec![10, 20]);
    }

    #[test]
    fn poll_ignores_non_matching_topic() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();
        // Different topic -- should not trigger handler.
        let frame = Frame::new(REMOTE, LOCAL, TOPIC_RPM);
        transport.push_incoming(frame);

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 0);
    }

    #[test]
    fn poll_dispatches_broadcast_frame() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();
        let mut frame = Frame::new(REMOTE, NodeAddr::BROADCAST, TOPIC_TEMP);
        frame.set_payload(&[99]).unwrap();
        transport.push_incoming(frame);

        let count = broker.poll(&mut transport).unwrap();
        assert!(count >= 1);
        assert_eq!(handler_a_count(), 1);
        assert_eq!(handler_a_payload(), vec![99]);
    }

    #[test]
    fn poll_rejects_frame_for_other_node() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();
        let other = NodeAddr::new(9, 9, 9);
        let frame = Frame::new(REMOTE, other, TOPIC_TEMP);
        transport.push_incoming(frame);

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 0);
    }

    #[test]
    fn poll_multiple_subscriptions_same_topic() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        broker.subscribe(TOPIC_TEMP, handler_b).unwrap();

        let mut transport = MockTransport::new();
        let frame = Frame::new(REMOTE, LOCAL, TOPIC_TEMP);
        transport.push_incoming(frame);

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 1);
        assert_eq!(handler_b_count(), 1);
    }

    #[test]
    fn poll_does_not_dispatch_to_unsubscribed() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        let h = broker.subscribe(TOPIC_TEMP, handler_a).unwrap();
        broker.unsubscribe(h).unwrap();

        let mut transport = MockTransport::new();
        let frame = Frame::new(REMOTE, LOCAL, TOPIC_TEMP);
        transport.push_incoming(frame);

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 0);
    }

    #[test]
    fn poll_multiple_frames() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();
        for i in 0..3u8 {
            let mut frame = Frame::new(REMOTE, LOCAL, TOPIC_TEMP);
            frame.set_payload(&[i]).unwrap();
            transport.push_incoming(frame);
        }

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 3);
        // Last handler call received the third frame's payload.
        assert_eq!(handler_a_payload(), vec![2]);
    }

    #[test]
    fn poll_mixed_matching_and_non_matching() {
        reset_handlers();

        let mut broker: Broker<8> = Broker::new(LOCAL);
        broker.subscribe(TOPIC_TEMP, handler_a).unwrap();

        let mut transport = MockTransport::new();

        // Frame 1: matching topic, addressed to us.
        let mut f1 = Frame::new(REMOTE, LOCAL, TOPIC_TEMP);
        f1.set_payload(&[1]).unwrap();
        transport.push_incoming(f1);

        // Frame 2: wrong topic.
        transport.push_incoming(Frame::new(REMOTE, LOCAL, TOPIC_RPM));

        // Frame 3: wrong destination.
        transport.push_incoming(Frame::new(REMOTE, NodeAddr::new(9, 9, 9), TOPIC_TEMP));

        // Frame 4: matching topic, broadcast.
        let mut f4 = Frame::new(REMOTE, NodeAddr::BROADCAST, TOPIC_TEMP);
        f4.set_payload(&[4]).unwrap();
        transport.push_incoming(f4);

        let _count = broker.poll(&mut transport).unwrap();
        assert_eq!(handler_a_count(), 2);
        assert_eq!(handler_a_payload(), vec![4]);
    }

    #[test]
    fn poll_transport_recv_error() {
        let broker: Broker<8> = Broker::new(LOCAL);
        let mut transport = MockTransport::new();
        transport.recv_error = Some(TransportError::RecvFailed);

        let result = broker.poll(&mut transport);
        assert_eq!(
            result,
            Err(BrokerError::TransportError(TransportError::RecvFailed))
        );
    }

    // -- BrokerError / From tests --------------------------------------------

    #[test]
    fn broker_error_from_transport_error() {
        let te = TransportError::BusError;
        let be: BrokerError = te.into();
        assert_eq!(be, BrokerError::TransportError(TransportError::BusError));
    }

    #[test]
    fn noop_handler_does_not_panic() {
        // Exercise the noop_handler function body (used as the default
        // initializer for subscription slots but never invoked in normal
        // operation).
        super::noop_handler(LOCAL, TOPIC_TEMP, &[1, 2, 3]);
    }
}
