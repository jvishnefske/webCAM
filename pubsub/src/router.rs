//! Multi-hop pub/sub mesh router.
//!
//! A [`MeshRouter`] sits between N transports and forwards frames between
//! them based on a subscription-driven routing table. Bridges learn routes
//! by listening for [`SUBSCRIBE_TOPIC`] control frames on their attached
//! transports: when a frame arrives on interface `i`, the router records
//! "interface `i` has downstream demand for this topic" and will forward
//! future frames on that topic toward `i` from other interfaces.
//!
//! Loop prevention has no dedicated mechanism beyond the routing table
//! itself:
//! - The inbound interface is always excluded when forwarding.
//! - Re-broadcast of subscription announcements to a given interface is
//!   suppressed once that interface already has a route for the topic.
//!   This makes the advertise graph acyclic, which in turn keeps the
//!   forward set acyclic.
//!
//! The router is `no_std`, no-alloc, with compile-time capacities. It holds
//! no transports itself — callers pass a `&mut [&mut dyn Transport]` slice
//! on every poll. The pubsub [`Transport`] trait is already object-safe.
//!
//! ## Typical use
//!
//! ```
//! use pubsub::{NodeAddr, Transport, TransportError};
//! use pubsub::frame::Frame;
//! use pubsub::router::{MeshRouter, PollOutcome};
//! use pubsub::topic::TopicId;
//!
//! struct Null;
//! impl Transport for Null {
//!     fn send(&mut self, _: &Frame) -> Result<(), TransportError> { Ok(()) }
//!     fn recv(&mut self, _: &mut Frame) -> Result<bool, TransportError> { Ok(false) }
//!     fn mtu(&self) -> usize { 64 }
//! }
//!
//! let mut router: MeshRouter<16> = MeshRouter::new(NodeAddr::new(1, 0, 0));
//! let mut a = Null;
//! let mut b = Null;
//! let mut transports: [&mut dyn Transport; 2] = [&mut a, &mut b];
//!
//! // Advertise a local subscription to every attached transport.
//! router
//!     .advertise_subscribe(
//!         TopicId::from_name("heartbeat"),
//!         NodeAddr::new(1, 0, 0),
//!         &mut transports,
//!     )
//!     .unwrap();
//!
//! // In a loop, poll each interface in turn.
//! let outcome = router.poll_iface(0, &mut transports).unwrap();
//! assert!(matches!(outcome, PollOutcome::Idle));
//! ```

use crate::addr::NodeAddr;
use crate::discovery::{SubscribeAnnouncement, SUBSCRIBE_TOPIC, UNSUBSCRIBE_TOPIC};
use crate::frame::Frame;
use crate::topic::TopicId;
use crate::transport::{Transport, TransportError};

/// Errors produced by the mesh router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshError {
    /// The interface index is out of range for the transports slice.
    InvalidIface,
    /// The routing table is full.
    RoutesFull,
    /// A control frame had a malformed payload.
    MalformedControl,
    /// Building an outbound frame payload failed.
    Encoding,
    /// An underlying transport failed.
    Transport(TransportError),
}

impl From<TransportError> for MeshError {
    fn from(e: TransportError) -> Self {
        MeshError::Transport(e)
    }
}

/// The result of polling a single interface.
#[derive(Debug, Clone, PartialEq)]
pub enum PollOutcome {
    /// No frame was available on the interface.
    Idle,
    /// A control (discovery) frame was consumed; routes may have changed.
    Control,
    /// A non-control frame was forwarded toward at least one other interface.
    Forwarded,
    /// A non-control frame arrived that no routing entry matched; the
    /// destination was not local and no bridge had demand, so the frame was
    /// silently dropped.
    Dropped,
    /// A frame addressed to this router's local node (or broadcast) is
    /// returned for the caller to dispatch to a local broker. The frame may
    /// also have been forwarded if there was downstream demand on another
    /// interface; forwarding and local delivery are independent.
    LocalDelivery(Frame),
}

/// A single routing table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Route {
    topic: TopicId,
    iface: u8,
    remote: NodeAddr,
    active: bool,
}

const EMPTY_ROUTE: Route = Route {
    topic: TopicId::from_raw(0),
    iface: 0,
    remote: NodeAddr::new(0, 0, 0),
    active: false,
};

/// A mesh router with compile-time bounded routing table.
///
/// `MAX_ROUTES` is the maximum number of simultaneous `(topic, iface,
/// remote)` entries. A single subscriber typically costs one entry per
/// router on its path. Size this to the expected subscription count across
/// the mesh, not to the total topic count.
pub struct MeshRouter<const MAX_ROUTES: usize> {
    local_addr: NodeAddr,
    routes: [Route; MAX_ROUTES],
}

impl<const MAX_ROUTES: usize> MeshRouter<MAX_ROUTES> {
    /// Create a new router bound to the given local address.
    pub fn new(local_addr: NodeAddr) -> Self {
        Self {
            local_addr,
            routes: [EMPTY_ROUTE; MAX_ROUTES],
        }
    }

    /// Return the local address of this router.
    pub fn local_addr(&self) -> NodeAddr {
        self.local_addr
    }

    /// Number of currently active routes in the table.
    pub fn route_count(&self) -> usize {
        self.routes.iter().filter(|r| r.active).count()
    }

    /// True if there is at least one active route for `topic` on `iface`.
    pub fn has_any_route(&self, topic: TopicId, iface: u8) -> bool {
        self.routes
            .iter()
            .any(|r| r.active && r.topic == topic && r.iface == iface)
    }

    /// Add a route. Returns `Ok(true)` if the entry was newly inserted,
    /// `Ok(false)` if an identical entry already existed, or
    /// `Err(RoutesFull)` if the table is full.
    ///
    /// Entries are considered identical when all of `(topic, iface,
    /// remote)` match.
    pub fn add_route(
        &mut self,
        topic: TopicId,
        iface: u8,
        remote: NodeAddr,
    ) -> Result<bool, MeshError> {
        for r in self.routes.iter() {
            if r.active && r.topic == topic && r.iface == iface && r.remote == remote {
                return Ok(false);
            }
        }
        for slot in self.routes.iter_mut() {
            if !slot.active {
                slot.topic = topic;
                slot.iface = iface;
                slot.remote = remote;
                slot.active = true;
                return Ok(true);
            }
        }
        Err(MeshError::RoutesFull)
    }

    /// Remove a specific `(topic, iface, remote)` route.
    ///
    /// Returns `true` if an entry was removed, `false` if none matched.
    pub fn remove_route(&mut self, topic: TopicId, iface: u8, remote: NodeAddr) -> bool {
        for slot in self.routes.iter_mut() {
            if slot.active && slot.topic == topic && slot.iface == iface && slot.remote == remote {
                slot.active = false;
                return true;
            }
        }
        false
    }

    /// Originate a subscription announcement for `topic` on behalf of
    /// `remote`, sending it on every provided transport.
    ///
    /// Typically called by a local node when it adds a subscription, so the
    /// surrounding mesh learns to deliver frames for the topic toward this
    /// node. Errors from individual transports are ignored (best-effort
    /// flood); a transport-level failure never aborts the loop.
    pub fn advertise_subscribe(
        &self,
        topic: TopicId,
        remote: NodeAddr,
        transports: &mut [&mut dyn Transport],
    ) -> Result<(), MeshError> {
        let ann = SubscribeAnnouncement::new(topic, remote);
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut frame = Frame::new(self.local_addr, NodeAddr::BROADCAST, SUBSCRIBE_TOPIC);
        frame
            .set_payload(&payload)
            .map_err(|_| MeshError::Encoding)?;
        for tp in transports.iter_mut() {
            let _ = tp.send(&frame);
        }
        Ok(())
    }

    /// Originate an unsubscribe announcement. Symmetric to
    /// [`advertise_subscribe`](Self::advertise_subscribe).
    pub fn advertise_unsubscribe(
        &self,
        topic: TopicId,
        remote: NodeAddr,
        transports: &mut [&mut dyn Transport],
    ) -> Result<(), MeshError> {
        let ann = SubscribeAnnouncement::new(topic, remote);
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut frame = Frame::new(self.local_addr, NodeAddr::BROADCAST, UNSUBSCRIBE_TOPIC);
        frame
            .set_payload(&payload)
            .map_err(|_| MeshError::Encoding)?;
        for tp in transports.iter_mut() {
            let _ = tp.send(&frame);
        }
        Ok(())
    }

    /// Poll a single interface, forwarding or absorbing at most one frame.
    ///
    /// The caller is expected to round-robin across interfaces from its own
    /// loop. This method does not itself iterate all interfaces, to keep
    /// per-call work bounded.
    ///
    /// See [`PollOutcome`] for the possible results.
    pub fn poll_iface(
        &mut self,
        iface: u8,
        transports: &mut [&mut dyn Transport],
    ) -> Result<PollOutcome, MeshError> {
        let idx = iface as usize;
        if idx >= transports.len() {
            return Err(MeshError::InvalidIface);
        }

        let mut frame = Frame::new(
            NodeAddr::new(0, 0, 0),
            NodeAddr::new(0, 0, 0),
            TopicId::from_raw(0),
        );
        let got = transports[idx].recv(&mut frame)?;
        if !got {
            return Ok(PollOutcome::Idle);
        }

        if frame.topic == SUBSCRIBE_TOPIC {
            self.handle_subscribe(iface, &frame, transports)?;
            return Ok(PollOutcome::Control);
        }
        if frame.topic == UNSUBSCRIBE_TOPIC {
            self.handle_unsubscribe(iface, &frame, transports)?;
            return Ok(PollOutcome::Control);
        }

        let mut forwarded = false;
        for (j, tp) in transports.iter_mut().enumerate() {
            if j as u8 == iface {
                continue;
            }
            if self.has_any_route(frame.topic, j as u8) {
                let _ = tp.send(&frame);
                forwarded = true;
            }
        }

        let dest = frame.destination;
        if dest == NodeAddr::BROADCAST || dest == self.local_addr {
            return Ok(PollOutcome::LocalDelivery(frame));
        }
        if forwarded {
            Ok(PollOutcome::Forwarded)
        } else {
            Ok(PollOutcome::Dropped)
        }
    }

    fn handle_subscribe(
        &mut self,
        iface: u8,
        frame: &Frame,
        transports: &mut [&mut dyn Transport],
    ) -> Result<(), MeshError> {
        let ann = SubscribeAnnouncement::decode(frame.payload_slice())
            .ok_or(MeshError::MalformedControl)?;
        let topic = ann.topic_id();
        let inserted = self.add_route(topic, iface, ann.remote)?;
        if !inserted {
            return Ok(());
        }
        // Re-emit to every interface that does not already have a route
        // for this topic. The check is what keeps the advertise graph
        // acyclic. Re-emit the original announcement unchanged so the
        // original `remote` is preserved, letting downstream bridges record
        // who ultimately wants the data.
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut out = Frame::new(self.local_addr, NodeAddr::BROADCAST, SUBSCRIBE_TOPIC);
        out.set_payload(&payload).map_err(|_| MeshError::Encoding)?;
        for (j, tp) in transports.iter_mut().enumerate() {
            if j as u8 == iface {
                continue;
            }
            if !self.has_any_route(topic, j as u8) {
                let _ = tp.send(&out);
            }
        }
        Ok(())
    }

    fn handle_unsubscribe(
        &mut self,
        iface: u8,
        frame: &Frame,
        transports: &mut [&mut dyn Transport],
    ) -> Result<(), MeshError> {
        let ann = SubscribeAnnouncement::decode(frame.payload_slice())
            .ok_or(MeshError::MalformedControl)?;
        let topic = ann.topic_id();
        let removed = self.remove_route(topic, iface, ann.remote);
        if !removed {
            return Ok(());
        }
        // If no routes remain for this topic on `iface`, we lost interest
        // on that side, but upstream demand from other interfaces may still
        // exist. Propagate only if we now have zero routes for the topic
        // anywhere — otherwise the withdrawal would be premature.
        let still_wanted = self.routes.iter().any(|r| r.active && r.topic == topic);
        if still_wanted {
            return Ok(());
        }
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut out = Frame::new(self.local_addr, NodeAddr::BROADCAST, UNSUBSCRIBE_TOPIC);
        out.set_payload(&payload).map_err(|_| MeshError::Encoding)?;
        for (j, tp) in transports.iter_mut().enumerate() {
            if j as u8 == iface {
                continue;
            }
            let _ = tp.send(&out);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::MAX_FRAME_PAYLOAD;

    // -------- Mock transport --------------------------------------------

    struct MockTransport {
        inbox: heapless::Deque<Frame, 8>,
        outbox: heapless::Vec<Frame, 16>,
        mtu: usize,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                inbox: heapless::Deque::new(),
                outbox: heapless::Vec::new(),
                mtu: MAX_FRAME_PAYLOAD,
            }
        }

        fn enqueue(&mut self, f: Frame) {
            self.inbox.push_back(f).ok();
        }

        fn sent_count(&self) -> usize {
            self.outbox.len()
        }

        fn sent(&self, i: usize) -> &Frame {
            &self.outbox[i]
        }
    }

    impl Transport for MockTransport {
        fn send(&mut self, frame: &Frame) -> Result<(), TransportError> {
            self.outbox
                .push(frame.clone())
                .map_err(|_| TransportError::SendFailed)
        }
        fn recv(&mut self, buf: &mut Frame) -> Result<bool, TransportError> {
            if let Some(f) = self.inbox.pop_front() {
                *buf = f;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        fn mtu(&self) -> usize {
            self.mtu
        }
    }

    fn sub_frame(src: NodeAddr, topic: TopicId, remote: NodeAddr) -> Frame {
        let ann = SubscribeAnnouncement::new(topic, remote);
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut f = Frame::new(src, NodeAddr::BROADCAST, SUBSCRIBE_TOPIC);
        f.set_payload(&payload).unwrap();
        f
    }

    const LOCAL: NodeAddr = NodeAddr::new(1, 0, 0);
    const A: NodeAddr = NodeAddr::new(2, 0, 0);
    const B: NodeAddr = NodeAddr::new(3, 0, 0);

    // -------- Basic table operations ------------------------------------

    #[test]
    fn new_router_has_no_routes() {
        let router: MeshRouter<8> = MeshRouter::new(LOCAL);
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn add_route_is_idempotent() {
        let mut router: MeshRouter<4> = MeshRouter::new(LOCAL);
        let t = TopicId::from_name("x");
        assert!(router.add_route(t, 0, A).unwrap());
        assert!(!router.add_route(t, 0, A).unwrap());
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn add_route_different_ifaces_stored_separately() {
        let mut router: MeshRouter<4> = MeshRouter::new(LOCAL);
        let t = TopicId::from_name("x");
        router.add_route(t, 0, A).unwrap();
        router.add_route(t, 1, A).unwrap();
        assert_eq!(router.route_count(), 2);
        assert!(router.has_any_route(t, 0));
        assert!(router.has_any_route(t, 1));
    }

    #[test]
    fn add_route_table_full() {
        let mut router: MeshRouter<2> = MeshRouter::new(LOCAL);
        let t = TopicId::from_name("x");
        router.add_route(t, 0, A).unwrap();
        router.add_route(t, 1, A).unwrap();
        assert_eq!(router.add_route(t, 2, A), Err(MeshError::RoutesFull));
    }

    #[test]
    fn remove_route_returns_true_when_present() {
        let mut router: MeshRouter<4> = MeshRouter::new(LOCAL);
        let t = TopicId::from_name("x");
        router.add_route(t, 0, A).unwrap();
        assert!(router.remove_route(t, 0, A));
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn remove_route_returns_false_when_absent() {
        let mut router: MeshRouter<4> = MeshRouter::new(LOCAL);
        assert!(!router.remove_route(TopicId::from_name("x"), 0, A));
    }

    #[test]
    fn remove_frees_slot_for_reuse() {
        let mut router: MeshRouter<1> = MeshRouter::new(LOCAL);
        let t = TopicId::from_name("x");
        router.add_route(t, 0, A).unwrap();
        assert_eq!(
            router.add_route(TopicId::from_name("y"), 0, A),
            Err(MeshError::RoutesFull)
        );
        router.remove_route(t, 0, A);
        assert!(router.add_route(TopicId::from_name("y"), 0, A).unwrap());
    }

    // -------- Advertise subscribe/unsubscribe ---------------------------

    #[test]
    fn advertise_subscribe_floods_all_transports() {
        let router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        let mut c = MockTransport::new();
        let mut ts: [&mut dyn Transport; 3] = [&mut a, &mut b, &mut c];
        router
            .advertise_subscribe(TopicId::from_name("temp"), LOCAL, &mut ts)
            .unwrap();
        assert_eq!(a.sent_count(), 1);
        assert_eq!(b.sent_count(), 1);
        assert_eq!(c.sent_count(), 1);
        assert_eq!(a.sent(0).topic, SUBSCRIBE_TOPIC);
        let decoded = SubscribeAnnouncement::decode(a.sent(0).payload_slice()).unwrap();
        assert_eq!(decoded.topic_id(), TopicId::from_name("temp"));
        assert_eq!(decoded.remote, LOCAL);
    }

    #[test]
    fn advertise_unsubscribe_uses_unsubscribe_topic() {
        let router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        router
            .advertise_unsubscribe(TopicId::from_name("temp"), LOCAL, &mut ts)
            .unwrap();
        assert_eq!(a.sent(0).topic, UNSUBSCRIBE_TOPIC);
    }

    // -------- Forwarding ------------------------------------------------

    #[test]
    fn forwards_to_subscribed_iface_only() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("payload");
        // Pretend we received an earlier _ps/sub from iface 2 for `topic`.
        router.add_route(topic, 2, B).unwrap();

        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        let mut c = MockTransport::new();

        let mut data = Frame::new(A, B, topic);
        data.set_payload(&[0xAA, 0xBB]).unwrap();
        a.enqueue(data);

        let mut ts: [&mut dyn Transport; 3] = [&mut a, &mut b, &mut c];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert!(matches!(outcome, PollOutcome::Forwarded));
        // b has no route; c has a route.
        assert_eq!(b.sent_count(), 0);
        assert_eq!(c.sent_count(), 1);
        assert_eq!(c.sent(0).payload_slice(), &[0xAA, 0xBB]);
    }

    #[test]
    fn no_loop_when_inbound_iface_has_route() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("t");
        // Both ifaces have a route — frame arrives on iface 0 and should
        // NOT go back out on iface 0.
        router.add_route(topic, 0, A).unwrap();
        router.add_route(topic, 1, B).unwrap();

        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        let mut data = Frame::new(A, B, topic);
        data.set_payload(&[1]).unwrap();
        a.enqueue(data);

        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert!(matches!(outcome, PollOutcome::Forwarded));
        // a (inbound) must not see an echo of its own frame.
        assert_eq!(a.sent_count(), 0);
        // b should receive the forwarded frame.
        assert_eq!(b.sent_count(), 1);
    }

    #[test]
    fn dropped_when_no_route_and_not_local() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut b = MockTransport::new();

        // Frame not for local, no routes in table.
        let data = Frame::new(A, B, TopicId::from_name("unknown"));
        a.enqueue(data);
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert!(matches!(outcome, PollOutcome::Dropped));
        assert_eq!(b.sent_count(), 0);
    }

    #[test]
    fn local_delivery_returned_to_caller() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut data = Frame::new(A, LOCAL, TopicId::from_name("ping"));
        data.set_payload(&[0x42]).unwrap();
        a.enqueue(data);
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        match outcome {
            PollOutcome::LocalDelivery(f) => assert_eq!(f.payload_slice(), &[0x42]),
            other => panic!("expected LocalDelivery, got {:?}", other),
        }
    }

    #[test]
    fn broadcast_is_local_delivery_and_forwarded() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("broadcast_test");
        router.add_route(topic, 1, B).unwrap();

        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        let mut data = Frame::new(A, NodeAddr::BROADCAST, topic);
        data.set_payload(&[0xFF]).unwrap();
        a.enqueue(data);
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert!(matches!(outcome, PollOutcome::LocalDelivery(_)));
        // Forward still happened.
        assert_eq!(b.sent_count(), 1);
    }

    #[test]
    fn idle_when_transport_empty() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        assert_eq!(router.poll_iface(0, &mut ts).unwrap(), PollOutcome::Idle);
    }

    #[test]
    fn invalid_iface_returns_error() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        assert_eq!(router.poll_iface(5, &mut ts), Err(MeshError::InvalidIface));
    }

    // -------- Discovery absorption --------------------------------------

    #[test]
    fn receiving_subscribe_creates_route() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("sensor");
        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        a.enqueue(sub_frame(A, topic, A));
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert_eq!(outcome, PollOutcome::Control);
        assert!(router.has_any_route(topic, 0));
        // Re-emitted to iface 1 (which had no prior route).
        assert_eq!(b.sent_count(), 1);
        assert_eq!(b.sent(0).topic, SUBSCRIBE_TOPIC);
    }

    #[test]
    fn subscribe_not_reemit_to_iface_with_existing_route() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("sensor");
        // Iface 1 already has downstream demand recorded.
        router.add_route(topic, 1, B).unwrap();

        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        a.enqueue(sub_frame(A, topic, A));
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        router.poll_iface(0, &mut ts).unwrap();
        // Iface 1 should NOT receive a re-emit since it already has a route
        // for `topic` (suppression rule).
        assert_eq!(b.sent_count(), 0);
        assert!(router.has_any_route(topic, 0));
    }

    #[test]
    fn duplicate_subscribe_not_reemitted() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("sensor");
        let mut a = MockTransport::new();
        let mut b = MockTransport::new();

        // First sub from A — learns (topic, 0, A), re-emits to iface 1.
        a.enqueue(sub_frame(A, topic, A));
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        router.poll_iface(0, &mut ts).unwrap();
        assert_eq!(b.sent_count(), 1);

        // Second identical sub from A — already in table; no new re-emit.
        a.enqueue(sub_frame(A, topic, A));
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        router.poll_iface(0, &mut ts).unwrap();
        assert_eq!(b.sent_count(), 1); // unchanged
    }

    #[test]
    fn triangle_advertise_terminates_without_loop() {
        // Simulate three bridges in a ring by running two routers and
        // showing that a sub originating at one reaches the other without
        // infinite re-forwarding.
        let mut bridge1: MeshRouter<16> = MeshRouter::new(NodeAddr::new(10, 0, 0));
        let mut bridge2: MeshRouter<16> = MeshRouter::new(NodeAddr::new(11, 0, 0));

        let topic = TopicId::from_name("ring_topic");
        let subscriber = NodeAddr::new(12, 0, 0);

        // Link topology: bridge1 has ifaces [0=to-bridge2, 1=to-subscriber]
        //                bridge2 has ifaces [0=to-bridge1]
        let mut b1_to_b2 = MockTransport::new();
        let mut b1_to_sub = MockTransport::new();
        let mut b2_to_b1 = MockTransport::new();

        // Subscriber sends sub on its link into bridge1 iface 1.
        b1_to_sub.enqueue(sub_frame(subscriber, topic, subscriber));

        // bridge1 processes iface 1, should re-emit on iface 0 to bridge2.
        {
            let mut ts: [&mut dyn Transport; 2] = [&mut b1_to_b2, &mut b1_to_sub];
            let out = bridge1.poll_iface(1, &mut ts).unwrap();
            assert_eq!(out, PollOutcome::Control);
        }
        // Route learned on bridge1: (topic, iface=1, remote=subscriber).
        assert!(bridge1.has_any_route(topic, 1));
        // A re-emit frame should now sit in b1_to_b2.outbox.
        assert_eq!(b1_to_b2.sent_count(), 1);

        // Transfer the re-emit from b1_to_b2 outbox into b2_to_b1 inbox.
        let reemit = b1_to_b2.sent(0).clone();
        b2_to_b1.enqueue(reemit);

        // bridge2 processes iface 0.
        {
            let mut ts: [&mut dyn Transport; 1] = [&mut b2_to_b1];
            let out = bridge2.poll_iface(0, &mut ts).unwrap();
            assert_eq!(out, PollOutcome::Control);
        }
        assert!(bridge2.has_any_route(topic, 0));
        // bridge2 has only one iface, so no re-emit possible; it simply
        // records the route. No infinite loop.
    }

    // -------- Unsubscribe -----------------------------------------------

    #[test]
    fn unsubscribe_removes_route() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("sensor");
        router.add_route(topic, 0, A).unwrap();

        let ann = SubscribeAnnouncement::new(topic, A);
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut frame = Frame::new(A, NodeAddr::BROADCAST, UNSUBSCRIBE_TOPIC);
        frame.set_payload(&payload).unwrap();

        let mut a = MockTransport::new();
        a.enqueue(frame);
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        let outcome = router.poll_iface(0, &mut ts).unwrap();
        assert_eq!(outcome, PollOutcome::Control);
        assert!(!router.has_any_route(topic, 0));
    }

    #[test]
    fn unsubscribe_suppressed_when_other_demand_exists() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let topic = TopicId::from_name("sensor");
        // Two sources of demand for the same topic.
        router.add_route(topic, 0, A).unwrap();
        router.add_route(topic, 1, B).unwrap();

        let ann = SubscribeAnnouncement::new(topic, A);
        let mut payload = [0u8; SubscribeAnnouncement::SIZE];
        ann.encode(&mut payload);
        let mut frame = Frame::new(A, NodeAddr::BROADCAST, UNSUBSCRIBE_TOPIC);
        frame.set_payload(&payload).unwrap();

        let mut a = MockTransport::new();
        let mut b = MockTransport::new();
        a.enqueue(frame);
        let mut ts: [&mut dyn Transport; 2] = [&mut a, &mut b];
        router.poll_iface(0, &mut ts).unwrap();
        // Route on iface 1 still exists; withdrawal should not have been
        // re-broadcast to iface 1.
        assert_eq!(b.sent_count(), 0);
        assert!(router.has_any_route(topic, 1));
    }

    // -------- Error paths -----------------------------------------------

    #[test]
    fn malformed_control_frame_errors() {
        let mut router: MeshRouter<8> = MeshRouter::new(LOCAL);
        let mut a = MockTransport::new();
        let mut frame = Frame::new(A, NodeAddr::BROADCAST, SUBSCRIBE_TOPIC);
        frame.set_payload(&[0xAA]).unwrap(); // too short
        a.enqueue(frame);
        let mut ts: [&mut dyn Transport; 1] = [&mut a];
        assert_eq!(
            router.poll_iface(0, &mut ts),
            Err(MeshError::MalformedControl)
        );
    }

    #[test]
    fn mesh_error_from_transport_error() {
        let e: MeshError = TransportError::BusError.into();
        assert_eq!(e, MeshError::Transport(TransportError::BusError));
    }
}
