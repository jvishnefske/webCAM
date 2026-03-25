//! Tests for the Dispatcher trait.

use hil_backplane::dispatcher::Dispatcher;
use hil_backplane::envelope::{Envelope, MessageKind};
use hil_backplane::error::BackplaneError;
use hil_backplane::message::{type_id_hash, BackplaneMessage};
use hil_backplane::node_id::NodeId;

/// A ping request message.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct Ping {
    #[n(0)]
    value: u32,
}

impl BackplaneMessage for Ping {
    const TYPE_ID: u32 = type_id_hash("test::dispatcher::Ping");
}

/// A pong response message.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct Pong {
    #[n(0)]
    value: u32,
}

impl BackplaneMessage for Pong {
    const TYPE_ID: u32 = type_id_hash("test::dispatcher::Pong");
}

/// A publish-only notification.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct Notification {
    #[n(0)]
    msg_id: u32,
}

impl BackplaneMessage for Notification {
    const TYPE_ID: u32 = type_id_hash("test::dispatcher::Notification");
}

/// Test dispatcher that handles Ping (with Pong response) and
/// Notification (no response).
struct TestDispatcher {
    last_notification_id: Option<u32>,
}

impl TestDispatcher {
    fn new() -> Self {
        Self {
            last_notification_id: None,
        }
    }
}

impl Dispatcher for TestDispatcher {
    fn dispatch(
        &mut self,
        type_id: u32,
        payload: &[u8],
        _envelope: &Envelope,
        response_buf: &mut [u8],
    ) -> Result<Option<(u32, usize)>, BackplaneError> {
        if type_id == Ping::TYPE_ID {
            let ping: Ping = minicbor::decode(payload).map_err(|_| BackplaneError::DecodeFailed)?;
            let pong = Pong { value: ping.value };
            let before_len = response_buf.len();
            let mut writer: &mut [u8] = response_buf;
            minicbor::encode(&pong, &mut writer).map_err(|_| BackplaneError::EncodeFailed)?;
            let after_len = writer.len();
            let written = before_len - after_len;
            Ok(Some((Pong::TYPE_ID, written)))
        } else if type_id == Notification::TYPE_ID {
            let notif: Notification =
                minicbor::decode(payload).map_err(|_| BackplaneError::DecodeFailed)?;
            self.last_notification_id = Some(notif.msg_id);
            Ok(None)
        } else {
            Ok(None)
        }
    }
}

fn make_envelope(type_id: u32, kind: MessageKind) -> Envelope {
    Envelope {
        type_id,
        seq: 1,
        source: NodeId::new(100),
        kind,
    }
}

#[test]
fn dispatch_known_request_type() {
    let mut dispatcher = TestDispatcher::new();
    let ping = Ping { value: 42 };
    let payload = minicbor::to_vec(&ping).expect("encode ping");
    let envelope = make_envelope(Ping::TYPE_ID, MessageKind::Request);

    let mut response_buf = [0u8; 256];
    let result = dispatcher
        .dispatch(Ping::TYPE_ID, &payload, &envelope, &mut response_buf)
        .expect("dispatch");

    let (resp_type_id, resp_len) = result.expect("should have response");
    assert_eq!(resp_type_id, Pong::TYPE_ID);

    let pong: Pong = minicbor::decode(&response_buf[..resp_len]).expect("decode pong");
    assert_eq!(pong.value, 42);
}

#[test]
fn dispatch_unknown_type_returns_none() {
    let mut dispatcher = TestDispatcher::new();
    let envelope = make_envelope(0xDEAD_BEEF, MessageKind::Publish);
    let payload = &[];

    let mut response_buf = [0u8; 256];
    let result = dispatcher
        .dispatch(0xDEAD_BEEF, payload, &envelope, &mut response_buf)
        .expect("dispatch");

    assert!(result.is_none());
}

#[test]
fn dispatch_publish_no_response() {
    let mut dispatcher = TestDispatcher::new();
    let notif = Notification { msg_id: 77 };
    let payload = minicbor::to_vec(&notif).expect("encode notification");
    let envelope = make_envelope(Notification::TYPE_ID, MessageKind::Publish);

    let mut response_buf = [0u8; 256];
    let result = dispatcher
        .dispatch(
            Notification::TYPE_ID,
            &payload,
            &envelope,
            &mut response_buf,
        )
        .expect("dispatch");

    assert!(result.is_none());
    assert_eq!(dispatcher.last_notification_id, Some(77));
}

#[test]
fn dispatch_multiple_message_types() {
    let mut dispatcher = TestDispatcher::new();

    // First: handle a Ping.
    let ping = Ping { value: 10 };
    let ping_payload = minicbor::to_vec(&ping).expect("encode");
    let ping_env = make_envelope(Ping::TYPE_ID, MessageKind::Request);
    let mut resp_buf = [0u8; 256];
    let result = dispatcher
        .dispatch(Ping::TYPE_ID, &ping_payload, &ping_env, &mut resp_buf)
        .expect("dispatch ping");
    assert!(result.is_some());

    // Second: handle a Notification.
    let notif = Notification { msg_id: 99 };
    let notif_payload = minicbor::to_vec(&notif).expect("encode");
    let notif_env = make_envelope(Notification::TYPE_ID, MessageKind::Publish);
    let result = dispatcher
        .dispatch(
            Notification::TYPE_ID,
            &notif_payload,
            &notif_env,
            &mut resp_buf,
        )
        .expect("dispatch notification");
    assert!(result.is_none());
    assert_eq!(dispatcher.last_notification_id, Some(99));
}
