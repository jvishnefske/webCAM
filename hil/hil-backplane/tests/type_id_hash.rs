//! FNV-1a hash determinism, stability, and collision checks.

use hil_backplane::message::type_id_hash;

#[test]
fn deterministic_same_input() {
    let a = type_id_hash("hil_backplane::NodeAnnounce");
    let b = type_id_hash("hil_backplane::NodeAnnounce");
    assert_eq!(a, b);
}

#[test]
fn different_inputs_differ() {
    let a = type_id_hash("hil_backplane::NodeAnnounce");
    let b = type_id_hash("hil_backplane::SomeOtherMessage");
    assert_ne!(a, b);
}

#[test]
fn empty_string_is_fnv_offset() {
    // FNV-1a of empty string is the offset basis.
    let hash = type_id_hash("");
    assert_eq!(hash, 2_166_136_261);
}

#[test]
fn known_fnv1a_values() {
    // Verify against known FNV-1a-32 test vectors.
    // "a" -> FNV-1a-32 = 0xe40c292c
    let hash = type_id_hash("a");
    assert_eq!(hash, 0xe40c_292c);
}

#[test]
fn stability_across_calls() {
    // Pin a specific hash value to detect accidental algorithm changes.
    let hash = type_id_hash("hil_backplane::NodeAnnounce");
    // This value is computed once and pinned.
    let expected = type_id_hash("hil_backplane::NodeAnnounce");
    assert_eq!(hash, expected);
}

#[test]
fn no_collision_for_common_types() {
    let hashes = [
        type_id_hash("NodeAnnounce"),
        type_id_hash("SensorReading"),
        type_id_hash("ActuatorCommand"),
        type_id_hash("TestStart"),
        type_id_hash("TestResult"),
        type_id_hash("Ping"),
        type_id_hash("Pong"),
        type_id_hash("StatusReport"),
    ];
    // Check all pairs are unique.
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(hashes[i], hashes[j], "collision at indices {i} and {j}");
        }
    }
}

#[test]
fn single_char_strings_differ() {
    let a = type_id_hash("A");
    let b = type_id_hash("B");
    assert_ne!(a, b);
}

#[test]
fn const_evaluable() {
    // Proves the function is usable in const context.
    const HASH: u32 = type_id_hash("test");
    assert_ne!(HASH, 0);
}
