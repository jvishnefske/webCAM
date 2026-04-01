//! Node dispatcher for handling backplane messages.
//!
//! A [`Node`] owns a transport, maintains handler registrations, and
//! provides publish / request / poll methods. It is single-threaded
//! with no `Arc` or `Mutex` — the caller drives the event loop.

#[cfg(feature = "std")]
mod imp {
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use crate::envelope::{Envelope, MessageKind, ENVELOPE_SIZE};
    use crate::error::BackplaneError;
    use crate::message::BackplaneMessage;
    use crate::node_id::NodeId;
    use crate::transport::udp::UdpTransport;
    use crate::transport::Transport;

    /// Maximum datagram size (Ethernet MTU).
    const MAX_DATAGRAM: usize = 1500;

    /// Handler return: optional `(response_type_id, cbor_payload)`.
    type HandlerResult = Result<Option<(u32, Vec<u8>)>, BackplaneError>;
    type HandlerFn = Box<dyn Fn(&[u8], &Envelope) -> HandlerResult + Send>;

    /// A backplane node that dispatches incoming messages to registered
    /// handlers and provides publish / request methods.
    pub struct Node {
        node_id: NodeId,
        transport: UdpTransport,
        handlers: HashMap<u32, HandlerFn>,
        seq_counter: u32,
    }

    impl Node {
        /// Creates a new node with the given ID and transport.
        pub fn new(node_id: NodeId, transport: UdpTransport) -> Self {
            Self {
                node_id,
                transport,
                handlers: HashMap::new(),
                seq_counter: 0,
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

        /// Registers a handler for a specific message type.
        ///
        /// The handler receives the deserialized message and returns an
        /// optional `(type_id, cbor_payload)` response. For publish
        /// messages, return `None`. For request messages, return
        /// `Some((ResponseType::TYPE_ID, encoded_response))`.
        pub fn register_handler<M, F>(&mut self, handler: F)
        where
            M: BackplaneMessage,
            F: Fn(M) -> Result<Option<(u32, Vec<u8>)>, BackplaneError> + Send + 'static,
        {
            self.handlers.insert(
                M::TYPE_ID,
                Box::new(move |payload: &[u8], _envelope: &Envelope| {
                    let msg: M =
                        minicbor::decode(payload).map_err(|_| BackplaneError::DecodeFailed)?;
                    handler(msg)
                }),
            );
        }

        /// Publishes a message to all subscribers via multicast.
        pub fn publish<M: BackplaneMessage>(&mut self, msg: &M) -> Result<(), BackplaneError> {
            let seq = self.next_seq();
            let envelope = Envelope {
                type_id: M::TYPE_ID,
                seq,
                source: self.node_id,
                kind: MessageKind::Publish,
            };
            let buf = encode_datagram(&envelope, msg)?;
            self.transport.multicast(&buf)
        }

        /// Sends a request and blocks until a matching response arrives
        /// or the timeout expires.
        pub fn request<M, R>(
            &mut self,
            msg: &M,
            target: SocketAddr,
            timeout: Duration,
        ) -> Result<R, BackplaneError>
        where
            M: BackplaneMessage,
            R: BackplaneMessage,
        {
            let seq = self.next_seq();
            let envelope = Envelope {
                type_id: M::TYPE_ID,
                seq,
                source: self.node_id,
                kind: MessageKind::Request,
            };
            let buf = encode_datagram(&envelope, msg)?;
            self.transport.send_to(&buf, target)?;

            let deadline = Instant::now() + timeout;
            let mut recv_buf = [0u8; MAX_DATAGRAM];
            loop {
                if Instant::now() >= deadline {
                    return Err(BackplaneError::Timeout);
                }
                if let Some((n, _addr)) = self.transport.recv_from(&mut recv_buf)? {
                    let env = Envelope::from_bytes(&recv_buf[..n])?;
                    if let MessageKind::Response { request_seq } = env.kind {
                        if request_seq == seq && env.type_id == R::TYPE_ID {
                            let payload = &recv_buf[ENVELOPE_SIZE..n];
                            let resp: R = minicbor::decode(payload)
                                .map_err(|_| BackplaneError::DecodeFailed)?;
                            return Ok(resp);
                        }
                    }
                    // Not our response — dispatch to handlers if applicable.
                    self.dispatch_datagram(&recv_buf[..n], &env);
                }
                std::thread::yield_now();
            }
        }

        /// Performs a single non-blocking receive and dispatches any
        /// incoming message to the registered handler.
        ///
        /// Returns `true` if a message was received and dispatched.
        pub fn poll(&mut self) -> Result<bool, BackplaneError> {
            let mut recv_buf = [0u8; MAX_DATAGRAM];
            let Some((n, sender)) = self.transport.recv_from(&mut recv_buf)? else {
                return Ok(false);
            };
            let env = Envelope::from_bytes(&recv_buf[..n])?;

            if let Some((resp_type_id, response_payload)) =
                self.dispatch_datagram(&recv_buf[..n], &env)
            {
                // If the incoming message was a Request, send the response back.
                if matches!(env.kind, MessageKind::Request) {
                    let resp_seq = self.next_seq();
                    let resp_envelope = Envelope {
                        type_id: resp_type_id,
                        seq: resp_seq,
                        source: self.node_id,
                        kind: MessageKind::Response {
                            request_seq: env.seq,
                        },
                    };
                    let mut buf = Vec::with_capacity(ENVELOPE_SIZE + response_payload.len());
                    buf.extend_from_slice(&resp_envelope.to_bytes());
                    buf.extend_from_slice(&response_payload);
                    self.transport.send_to(&buf, sender)?;
                }
            }

            Ok(true)
        }

        /// Runs a blocking event loop, polling until an error occurs.
        pub fn run(&mut self) -> Result<(), BackplaneError> {
            loop {
                self.poll()?;
                std::thread::yield_now();
            }
        }

        /// Dispatches a received datagram to the registered handler, if any.
        /// Returns `(response_type_id, response_payload)` if the handler produced one.
        fn dispatch_datagram(&self, datagram: &[u8], env: &Envelope) -> Option<(u32, Vec<u8>)> {
            let handler = self.handlers.get(&env.type_id)?;
            let payload = &datagram[ENVELOPE_SIZE..];
            handler(payload, env).unwrap_or_default()
        }
    }

    /// Encodes an envelope + CBOR payload into a `Vec<u8>` datagram.
    fn encode_datagram<M: BackplaneMessage>(
        envelope: &Envelope,
        msg: &M,
    ) -> Result<Vec<u8>, BackplaneError> {
        let header = envelope.to_bytes();
        let payload = minicbor::to_vec(msg).map_err(|_| BackplaneError::EncodeFailed)?;
        let mut buf = Vec::with_capacity(header.len() + payload.len());
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&payload);
        Ok(buf)
    }
}

#[cfg(feature = "std")]
pub use imp::Node;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
mod tests {
    use super::*;
    use crate::node_id::NodeId;
    use crate::transport::udp::{UdpTransport, DEFAULT_MULTICAST_ADDR};

    #[test]
    fn node_new_and_node_id() {
        let transport =
            UdpTransport::new(DEFAULT_MULTICAST_ADDR, 0).expect("transport creation failed");
        let node = Node::new(NodeId::new(99), transport);
        assert_eq!(node.node_id(), NodeId::new(99));
    }

    #[test]
    fn node_publish_increments_seq() {
        use crate::message::BackplaneMessage;

        #[derive(Debug)]
        struct Ping;
        impl BackplaneMessage for Ping {
            const TYPE_ID: u32 = 0xDEAD;
        }
        impl<C> minicbor::Encode<C> for Ping {
            fn encode<W: minicbor::encode::Write>(
                &self,
                e: &mut minicbor::Encoder<W>,
                _ctx: &mut C,
            ) -> Result<(), minicbor::encode::Error<W::Error>> {
                e.u8(0)?;
                Ok(())
            }
        }
        impl<'b, C> minicbor::Decode<'b, C> for Ping {
            fn decode(
                d: &mut minicbor::Decoder<'b>,
                _ctx: &mut C,
            ) -> Result<Self, minicbor::decode::Error> {
                d.u8()?;
                Ok(Ping)
            }
        }

        let transport = UdpTransport::with_defaults().expect("transport creation failed");
        let mut node = Node::new(NodeId::new(1), transport);

        // Publish twice -- should not panic (exercises next_seq and dispatch)
        node.publish(&Ping).unwrap();
        node.publish(&Ping).unwrap();
    }
}
