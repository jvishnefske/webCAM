//! Envelope encode/decode round-trip tests for all `MessageKind` variants.

use hil_backplane::envelope::{Envelope, MessageKind, ENVELOPE_SIZE};
use hil_backplane::error::BackplaneError;
use hil_backplane::node_id::NodeId;

#[test]
fn publish_roundtrip() {
    let original = Envelope {
        type_id: 0xDEAD_BEEF,
        seq: 42,
        source: NodeId::new(7),
        kind: MessageKind::Publish,
    };
    let bytes = original.to_bytes();
    assert_eq!(bytes.len(), ENVELOPE_SIZE);
    let decoded = Envelope::from_bytes(&bytes).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn request_roundtrip() {
    let original = Envelope {
        type_id: 0x1234_5678,
        seq: 100,
        source: NodeId::new(99),
        kind: MessageKind::Request,
    };
    let bytes = original.to_bytes();
    let decoded = Envelope::from_bytes(&bytes).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn response_roundtrip() {
    let original = Envelope {
        type_id: 0xCAFE_BABE,
        seq: 200,
        source: NodeId::new(3),
        kind: MessageKind::Response { request_seq: 100 },
    };
    let bytes = original.to_bytes();
    let decoded = Envelope::from_bytes(&bytes).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn response_preserves_request_seq() {
    let original = Envelope {
        type_id: 0,
        seq: 0,
        source: NodeId::new(0),
        kind: MessageKind::Response {
            request_seq: 0xFFFF_FFFF,
        },
    };
    let bytes = original.to_bytes();
    let decoded = Envelope::from_bytes(&bytes).expect("decode should succeed");
    assert_eq!(
        decoded.kind,
        MessageKind::Response {
            request_seq: 0xFFFF_FFFF
        }
    );
}

#[test]
fn buffer_too_short_returns_error() {
    let short_buf = [0u8; ENVELOPE_SIZE - 1];
    let result = Envelope::from_bytes(&short_buf);
    assert!(matches!(result, Err(BackplaneError::InvalidEnvelope)));
}

#[test]
fn invalid_kind_byte_returns_error() {
    let mut bytes = Envelope {
        type_id: 0,
        seq: 0,
        source: NodeId::new(0),
        kind: MessageKind::Publish,
    }
    .to_bytes();
    bytes[12] = 3; // invalid kind
    let result = Envelope::from_bytes(&bytes);
    assert!(matches!(result, Err(BackplaneError::InvalidEnvelope)));
}

#[test]
fn envelope_size_is_17() {
    assert_eq!(ENVELOPE_SIZE, 17);
}

#[test]
fn extra_trailing_bytes_are_ignored() {
    let original = Envelope {
        type_id: 0x11,
        seq: 22,
        source: NodeId::new(33),
        kind: MessageKind::Publish,
    };
    let bytes = original.to_bytes();
    let mut extended = Vec::from(bytes.as_slice());
    extended.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // trailing payload
    let decoded = Envelope::from_bytes(&extended).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn publish_request_seq_is_zero() {
    let env = Envelope {
        type_id: 0,
        seq: 0,
        source: NodeId::new(0),
        kind: MessageKind::Publish,
    };
    let bytes = env.to_bytes();
    let request_seq = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
    assert_eq!(request_seq, 0);
}

#[test]
fn request_request_seq_is_zero() {
    let env = Envelope {
        type_id: 0,
        seq: 0,
        source: NodeId::new(0),
        kind: MessageKind::Request,
    };
    let bytes = env.to_bytes();
    let request_seq = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
    assert_eq!(request_seq, 0);
}
