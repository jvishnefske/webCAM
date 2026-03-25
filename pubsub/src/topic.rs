//! Topic identifier based on a 32-bit FNV-1a hash of the topic name.
//!
//! Allows `no_std`, no-alloc topic matching using compile-time hashing.

use core::fmt;

/// A topic identifier stored as a 32-bit FNV-1a hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TopicId(u32);

/// FNV-1a 32-bit offset basis.
const FNV_OFFSET: u32 = 0x811c_9dc5;
/// FNV-1a 32-bit prime.
const FNV_PRIME: u32 = 0x0100_0193;

/// Compute the FNV-1a 32-bit hash of a byte slice at compile time.
const fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

impl TopicId {
    /// Create a topic ID from a name string using FNV-1a 32-bit hashing.
    ///
    /// This is `const`-compatible, so it can be evaluated at compile time.
    pub const fn from_name(name: &str) -> Self {
        Self(fnv1a_32(name.as_bytes()))
    }

    /// Create a topic ID from a raw `u32` hash value.
    pub const fn from_raw(v: u32) -> Self {
        Self(v)
    }

    /// Get the raw 32-bit hash value.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TopicId(0x{:08X})", self.0)
    }
}

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

/// Compile-time topic ID from a string literal.
///
/// ```
/// use pubsub::topic;
/// const TEMP: pubsub::topic::TopicId = topic!("motor_temp");
/// ```
#[macro_export]
macro_rules! topic {
    ($name:expr) => {
        $crate::topic::TopicId::from_name($name)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;
    use std::vec::Vec;

    #[test]
    fn from_name_deterministic() {
        let a = TopicId::from_name("motor_temp");
        let b = TopicId::from_name("motor_temp");
        assert_eq!(a, b);
    }

    #[test]
    fn different_names_differ() {
        let a = TopicId::from_name("motor_temp");
        let b = TopicId::from_name("motor_speed");
        assert_ne!(a, b);
    }

    #[test]
    fn from_raw_round_trip() {
        let id = TopicId::from_name("sensor/pressure");
        let raw = id.as_u32();
        let back = TopicId::from_raw(raw);
        assert_eq!(id, back);
    }

    #[test]
    fn known_fnv1a_values() {
        // FNV-1a 32-bit of empty string is the offset basis.
        assert_eq!(TopicId::from_name("").as_u32(), 0x811c_9dc5);
    }

    #[test]
    fn debug_format() {
        let id = TopicId::from_raw(0xDEAD_BEEF);
        assert_eq!(format!("{:?}", id), "TopicId(0xDEADBEEF)");
    }

    #[test]
    fn display_format() {
        let id = TopicId::from_raw(0x0000_0001);
        assert_eq!(format!("{}", id), "0x00000001");
    }

    #[test]
    fn hash_uniqueness_sample() {
        let names = [
            "motor_temp",
            "motor_speed",
            "spindle_rpm",
            "coolant_flow",
            "axis_x_pos",
            "axis_y_pos",
            "axis_z_pos",
            "estop",
            "heartbeat",
            "status",
        ];
        let ids: Vec<u32> = names
            .iter()
            .map(|n| TopicId::from_name(n).as_u32())
            .collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(
                    ids[i], ids[j],
                    "collision between {:?} and {:?}",
                    names[i], names[j]
                );
            }
        }
    }

    #[test]
    fn const_evaluation() {
        const ID: TopicId = TopicId::from_name("motor_temp");
        const RAW: u32 = ID.as_u32();
        assert_ne!(RAW, 0);
        assert_eq!(ID, TopicId::from_name("motor_temp"));
    }

    #[test]
    fn topic_macro() {
        const T: TopicId = topic!("motor_temp");
        assert_eq!(T, TopicId::from_name("motor_temp"));
    }

    #[test]
    fn equality_and_hash() {
        use core::hash::{Hash, Hasher};

        let a = TopicId::from_name("x");
        let b = TopicId::from_name("x");
        let c = TopicId::from_name("y");
        assert_eq!(a, b);
        assert_ne!(a, c);

        let hash_of = |t: TopicId| -> u64 {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash_of(a), hash_of(b));
    }

    #[test]
    fn single_char_names() {
        let a = TopicId::from_name("a");
        let b = TopicId::from_name("b");
        assert_ne!(a, b);
    }
}
