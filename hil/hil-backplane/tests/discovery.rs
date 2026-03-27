#![allow(clippy::expect_used)]
//! NodeAnnounce CBOR encode/decode round-trip tests.

use hil_backplane::discovery::NodeAnnounce;
use hil_backplane::message::BackplaneMessage;
use hil_backplane::node_id::NodeId;

#[test]
fn node_announce_roundtrip() {
    let mut publishes = heapless::Vec::new();
    publishes.push(0x1111).expect("push publishes");
    publishes.push(0x2222).expect("push publishes");

    let mut serves = heapless::Vec::new();
    serves.push(0x3333).expect("push serves");

    let original = NodeAnnounce {
        node_id: NodeId::new(42),
        name: heapless::String::try_from("test-node").expect("name fits"),
        publishes,
        serves,
    };

    let encoded = minicbor::to_vec(&original).expect("encode should succeed");
    let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode should succeed");

    assert_eq!(decoded.node_id, original.node_id);
    assert_eq!(decoded.name, original.name);
    assert_eq!(decoded.publishes.as_slice(), original.publishes.as_slice());
    assert_eq!(decoded.serves.as_slice(), original.serves.as_slice());
}

#[test]
fn node_announce_empty_lists() {
    let original = NodeAnnounce {
        node_id: NodeId::new(0),
        name: heapless::String::new(),
        publishes: heapless::Vec::new(),
        serves: heapless::Vec::new(),
    };

    let encoded = minicbor::to_vec(&original).expect("encode should succeed");
    let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode should succeed");

    assert_eq!(decoded.node_id, original.node_id);
    assert!(decoded.name.is_empty());
    assert!(decoded.publishes.is_empty());
    assert!(decoded.serves.is_empty());
}

#[test]
fn node_announce_has_type_id() {
    // Ensure the TYPE_ID is non-zero and deterministic.
    assert_ne!(NodeAnnounce::TYPE_ID, 0);
    assert_eq!(
        NodeAnnounce::TYPE_ID,
        hil_backplane::message::type_id_hash("hil_backplane::NodeAnnounce")
    );
}

#[test]
fn node_announce_max_name_length() {
    let long_name = "a]".repeat(32); // 64 chars
    let name = heapless::String::<64>::try_from(long_name.as_str()).expect("should fit in 64");
    let original = NodeAnnounce {
        node_id: NodeId::new(1),
        name,
        publishes: heapless::Vec::new(),
        serves: heapless::Vec::new(),
    };

    let encoded = minicbor::to_vec(&original).expect("encode should succeed");
    let decoded: NodeAnnounce = minicbor::decode(&encoded).expect("decode should succeed");
    assert_eq!(decoded.name.len(), 64);
}
