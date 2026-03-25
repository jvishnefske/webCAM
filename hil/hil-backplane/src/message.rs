//! Backplane message trait and type ID hashing.

/// Trait for messages that can be sent over the backplane.
///
/// Each message type has a unique `TYPE_ID` derived from its type path
/// using FNV-1a hashing. Messages are encoded/decoded as CBOR payloads.
pub trait BackplaneMessage: minicbor::Encode<()> + for<'b> minicbor::Decode<'b, ()> {
    /// Unique identifier for this message type, computed via [`type_id_hash`].
    const TYPE_ID: u32;
}

/// Computes a 32-bit FNV-1a hash of a string at compile time.
///
/// Used to derive stable, deterministic type IDs from message type paths.
/// The FNV-1a algorithm provides good distribution for short strings.
pub const fn type_id_hash(path: &str) -> u32 {
    const FNV_OFFSET: u32 = 2_166_136_261;
    const FNV_PRIME: u32 = 16_777_619;

    let bytes = path.as_bytes();
    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}
