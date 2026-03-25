//! Tests for fixed-buffer encode/decode helpers.

use hil_backplane::codec::{decode_envelope, encode_to_slice};
use hil_backplane::envelope::{Envelope, MessageKind, ENVELOPE_SIZE};
use hil_backplane::error::BackplaneError;
use hil_backplane::message::{type_id_hash, BackplaneMessage};
use hil_backplane::node_id::NodeId;

/// A simple test message.
#[derive(Debug, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
struct TestMsg {
    #[n(0)]
    value: u32,
}

impl BackplaneMessage for TestMsg {
    const TYPE_ID: u32 = type_id_hash("test::codec::TestMsg");
}

#[test]
fn encode_decode_publish() {
    let envelope = Envelope {
        type_id: TestMsg::TYPE_ID,
        seq: 42,
        source: NodeId::new(7),
        kind: MessageKind::Publish,
    };
    let msg = TestMsg { value: 123 };

    let mut buf = [0u8; 256];
    let n = encode_to_slice(&envelope, &msg, &mut buf).expect("encode");

    assert!(n > ENVELOPE_SIZE);

    let (decoded_env, payload) = decode_envelope(&buf[..n]).expect("decode envelope");
    assert_eq!(decoded_env.type_id, TestMsg::TYPE_ID);
    assert_eq!(decoded_env.seq, 42);
    assert_eq!(decoded_env.source, NodeId::new(7));
    assert_eq!(decoded_env.kind, MessageKind::Publish);

    let decoded_msg: TestMsg = minicbor::decode(payload).expect("decode payload");
    assert_eq!(decoded_msg.value, 123);
}

#[test]
fn encode_decode_request() {
    let envelope = Envelope {
        type_id: TestMsg::TYPE_ID,
        seq: 10,
        source: NodeId::new(3),
        kind: MessageKind::Request,
    };
    let msg = TestMsg { value: 999 };

    let mut buf = [0u8; 256];
    let n = encode_to_slice(&envelope, &msg, &mut buf).expect("encode");

    let (decoded_env, payload) = decode_envelope(&buf[..n]).expect("decode");
    assert_eq!(decoded_env.kind, MessageKind::Request);

    let decoded_msg: TestMsg = minicbor::decode(payload).expect("decode payload");
    assert_eq!(decoded_msg.value, 999);
}

#[test]
fn encode_decode_response() {
    let envelope = Envelope {
        type_id: TestMsg::TYPE_ID,
        seq: 20,
        source: NodeId::new(5),
        kind: MessageKind::Response { request_seq: 10 },
    };
    let msg = TestMsg { value: 555 };

    let mut buf = [0u8; 256];
    let n = encode_to_slice(&envelope, &msg, &mut buf).expect("encode");

    let (decoded_env, payload) = decode_envelope(&buf[..n]).expect("decode");
    assert_eq!(decoded_env.kind, MessageKind::Response { request_seq: 10 });

    let decoded_msg: TestMsg = minicbor::decode(payload).expect("decode payload");
    assert_eq!(decoded_msg.value, 555);
}

#[test]
fn buffer_too_small_for_header() {
    let envelope = Envelope {
        type_id: TestMsg::TYPE_ID,
        seq: 0,
        source: NodeId::new(1),
        kind: MessageKind::Publish,
    };
    let msg = TestMsg { value: 0 };

    let mut buf = [0u8; 10]; // Too small for 17-byte header.
    let result = encode_to_slice(&envelope, &msg, &mut buf);
    assert!(matches!(result, Err(BackplaneError::BufferTooSmall)));
}

#[test]
fn buffer_too_small_for_payload() {
    let envelope = Envelope {
        type_id: TestMsg::TYPE_ID,
        seq: 0,
        source: NodeId::new(1),
        kind: MessageKind::Publish,
    };
    let msg = TestMsg { value: 0 };

    // Header fits (17 bytes) but no room for any payload.
    let mut buf = [0u8; ENVELOPE_SIZE];
    let result = encode_to_slice(&envelope, &msg, &mut buf);
    // minicbor encoding into a zero-length slice will fail.
    assert!(result.is_err());
}
