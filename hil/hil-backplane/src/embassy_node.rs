//! Async backplane node for embassy-net environments.
//!
//! [`EmbassyNode`] provides publish, send-request, and poll methods
//! over an embassy-net [`UdpSocket`]. It uses fixed-size buffers and
//! a [`Dispatcher`] trait implementation for heap-free message handling
//! on `no_std` targets.

use core::convert::Infallible;

use embassy_net::udp::UdpSocket;
use embassy_net::IpEndpoint;

use crate::codec::{decode_envelope, encode_to_slice};
use crate::dispatcher::Dispatcher;
use crate::envelope::{Envelope, MessageKind, ENVELOPE_SIZE};
use crate::error::BackplaneError;
use crate::message::BackplaneMessage;
use crate::node_id::NodeId;

/// Maximum datagram size (Ethernet MTU).
const MAX_DATAGRAM: usize = 1500;

/// An async backplane node using an embassy-net UDP socket.
///
/// Owns fixed-size transmit and receive buffers to avoid heap
/// allocation. The generic `D` parameter is a user-provided
/// [`Dispatcher`] that handles incoming messages.
pub struct EmbassyNode<'a, D: Dispatcher> {
    node_id: NodeId,
    socket: UdpSocket<'a>,
    dispatcher: D,
    seq_counter: u32,
    tx_buf: [u8; MAX_DATAGRAM],
    rx_buf: [u8; MAX_DATAGRAM],
}

impl<'a, D: Dispatcher> EmbassyNode<'a, D> {
    /// Creates a new embassy backplane node.
    ///
    /// The `socket` should already be bound to the desired local port.
    /// The `dispatcher` handles incoming messages via type-ID matching.
    pub fn new(node_id: NodeId, socket: UdpSocket<'a>, dispatcher: D) -> Self {
        Self {
            node_id,
            socket,
            dispatcher,
            seq_counter: 0,
            tx_buf: [0u8; MAX_DATAGRAM],
            rx_buf: [0u8; MAX_DATAGRAM],
        }
    }

    /// Returns this node's identity.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Returns the next sequence number and increments the counter.
    fn next_seq(&mut self) -> u32 {
        let seq = self.seq_counter;
        self.seq_counter = self.seq_counter.wrapping_add(1);
        seq
    }

    /// Publishes a message to the given multicast endpoint.
    ///
    /// Encodes the envelope and CBOR payload into the internal transmit
    /// buffer, then sends via the UDP socket.
    ///
    /// # Errors
    ///
    /// Returns [`BackplaneError::BufferTooSmall`] if the message is too
    /// large, [`BackplaneError::EncodeFailed`] if CBOR encoding fails,
    /// or [`BackplaneError::SendFailed`] if the socket send fails.
    pub async fn publish<M: BackplaneMessage>(
        &mut self,
        msg: &M,
        multicast_ep: IpEndpoint,
    ) -> Result<(), BackplaneError> {
        let seq = self.next_seq();
        let envelope = Envelope {
            type_id: M::TYPE_ID,
            seq,
            source: self.node_id,
            kind: MessageKind::Publish,
        };
        let n = encode_to_slice(&envelope, msg, &mut self.tx_buf)?;
        self.socket
            .send_to(&self.tx_buf[..n], multicast_ep)
            .await
            .map_err(|_| BackplaneError::SendFailed)
    }

    /// Sends a request message to a specific endpoint.
    ///
    /// Returns the sequence number for correlation with a future
    /// response. The caller should match incoming responses by
    /// checking `envelope.kind == Response { request_seq }` in
    /// their [`Dispatcher`] implementation.
    ///
    /// # Errors
    ///
    /// Returns [`BackplaneError::BufferTooSmall`] if the message is too
    /// large, [`BackplaneError::EncodeFailed`] if CBOR encoding fails,
    /// or [`BackplaneError::SendFailed`] if the socket send fails.
    pub async fn send_request<M: BackplaneMessage>(
        &mut self,
        msg: &M,
        target: IpEndpoint,
    ) -> Result<u32, BackplaneError> {
        let seq = self.next_seq();
        let envelope = Envelope {
            type_id: M::TYPE_ID,
            seq,
            source: self.node_id,
            kind: MessageKind::Request,
        };
        let n = encode_to_slice(&envelope, msg, &mut self.tx_buf)?;
        self.socket
            .send_to(&self.tx_buf[..n], target)
            .await
            .map_err(|_| BackplaneError::SendFailed)?;
        Ok(seq)
    }

    /// Receives and dispatches a single datagram.
    ///
    /// Blocks asynchronously until a datagram arrives. Decodes the
    /// envelope, calls the dispatcher, and sends back a response if
    /// the incoming message was a [`MessageKind::Request`] and the
    /// dispatcher produced a response.
    ///
    /// Returns `true` if a message was successfully received and
    /// dispatched.
    ///
    /// # Errors
    ///
    /// Returns [`BackplaneError::RecvFailed`] on socket receive error,
    /// or propagates errors from envelope decoding and dispatch.
    pub async fn poll_once(&mut self) -> Result<bool, BackplaneError> {
        let (n, sender) = self
            .socket
            .recv_from(&mut self.rx_buf)
            .await
            .map_err(|_| BackplaneError::RecvFailed)?;

        let (env, payload) = decode_envelope(&self.rx_buf[..n])?;

        // Dispatch using tx_buf as the response buffer (disjoint from rx_buf).
        let response = self.dispatcher.dispatch(
            env.type_id,
            payload,
            &env,
            &mut self.tx_buf[ENVELOPE_SIZE..],
        )?;

        if let (MessageKind::Request, Some((resp_type_id, resp_len))) = (&env.kind, response) {
            let resp_seq = self.next_seq();
            let resp_envelope = Envelope {
                type_id: resp_type_id,
                seq: resp_seq,
                source: self.node_id,
                kind: MessageKind::Response {
                    request_seq: env.seq,
                },
            };
            let header = resp_envelope.to_bytes();
            self.tx_buf[..ENVELOPE_SIZE].copy_from_slice(&header);
            let total = ENVELOPE_SIZE + resp_len;

            self.socket
                .send_to(&self.tx_buf[..total], sender)
                .await
                .map_err(|_| BackplaneError::SendFailed)?;
        }

        Ok(true)
    }

    /// Runs the dispatch loop indefinitely.
    ///
    /// Continuously calls [`poll_once`](Self::poll_once) to receive
    /// and dispatch messages. Returns only on error.
    pub async fn run(&mut self) -> Result<Infallible, BackplaneError> {
        loop {
            self.poll_once().await?;
        }
    }
}
