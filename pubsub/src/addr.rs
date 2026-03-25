//! Hierarchical node address for pub/sub nodes.
//!
//! Layout in u32: `0x00_BB_DD_EE` where BB=bus, DD=device, EE=endpoint.

use core::fmt;

/// A globally unique node address: `bus:device:endpoint`, packed into a `u32`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeAddr {
    bus: u8,
    device: u8,
    endpoint: u8,
}

impl NodeAddr {
    /// Broadcast address (`0xFF:0xFF:0xFF`). Received by all nodes.
    pub const BROADCAST: Self = Self {
        bus: 0xFF,
        device: 0xFF,
        endpoint: 0xFF,
    };

    /// Create a new address from bus, device, and endpoint components.
    pub const fn new(bus: u8, device: u8, endpoint: u8) -> Self {
        Self {
            bus,
            device,
            endpoint,
        }
    }

    /// Return the bus component.
    pub const fn bus(self) -> u8 {
        self.bus
    }

    /// Return the device component.
    pub const fn device(self) -> u8 {
        self.device
    }

    /// Return the endpoint component.
    pub const fn endpoint(self) -> u8 {
        self.endpoint
    }

    /// Pack into a `u32` as `0x00_BB_DD_EE`.
    pub const fn to_u32(self) -> u32 {
        (self.bus as u32) << 16 | (self.device as u32) << 8 | (self.endpoint as u32)
    }

    /// Unpack from a `u32`. The top byte is ignored.
    pub const fn from_u32(v: u32) -> Self {
        Self {
            bus: (v >> 16) as u8,
            device: (v >> 8) as u8,
            endpoint: v as u8,
        }
    }

    /// Check if this is the broadcast address.
    pub const fn is_broadcast(self) -> bool {
        self.bus == 0xFF && self.device == 0xFF && self.endpoint == 0xFF
    }

    /// Check if a frame addressed to `dest` should be received by this node.
    ///
    /// Returns `true` on exact match or if `dest` is the broadcast address.
    pub fn accepts(&self, dest: NodeAddr) -> bool {
        dest.is_broadcast() || *self == dest
    }
}

impl fmt::Debug for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NodeAddr({:02X}:{:02X}:{:02X})",
            self.bus, self.device, self.endpoint
        )
    }
}

impl fmt::Display for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}",
            self.bus, self.device, self.endpoint
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;

    #[test]
    fn construction_and_accessors() {
        let addr = NodeAddr::new(0x01, 0x02, 0x03);
        assert_eq!(addr.bus(), 0x01);
        assert_eq!(addr.device(), 0x02);
        assert_eq!(addr.endpoint(), 0x03);
    }

    #[test]
    fn round_trip_u32() {
        let addr = NodeAddr::new(0xAB, 0xCD, 0xEF);
        let packed = addr.to_u32();
        assert_eq!(packed, 0x00AB_CDEF);
        let unpacked = NodeAddr::from_u32(packed);
        assert_eq!(unpacked, addr);
    }

    #[test]
    fn from_u32_ignores_top_byte() {
        let addr = NodeAddr::from_u32(0xFF_01_02_03);
        assert_eq!(addr, NodeAddr::new(0x01, 0x02, 0x03));
    }

    #[test]
    fn broadcast_constant() {
        assert_eq!(NodeAddr::BROADCAST, NodeAddr::new(0xFF, 0xFF, 0xFF));
        assert!(NodeAddr::BROADCAST.is_broadcast());
        assert_eq!(NodeAddr::BROADCAST.to_u32(), 0x00FF_FFFF);
    }

    #[test]
    fn non_broadcast_is_not_broadcast() {
        assert!(!NodeAddr::new(0x01, 0x02, 0x03).is_broadcast());
        assert!(!NodeAddr::new(0xFF, 0xFF, 0x00).is_broadcast());
    }

    #[test]
    fn accepts_exact_match() {
        let node = NodeAddr::new(0x01, 0x02, 0x03);
        assert!(node.accepts(NodeAddr::new(0x01, 0x02, 0x03)));
    }

    #[test]
    fn accepts_broadcast() {
        let node = NodeAddr::new(0x01, 0x02, 0x03);
        assert!(node.accepts(NodeAddr::BROADCAST));
    }

    #[test]
    fn rejects_different_address() {
        let node = NodeAddr::new(0x01, 0x02, 0x03);
        assert!(!node.accepts(NodeAddr::new(0x01, 0x02, 0x04)));
        assert!(!node.accepts(NodeAddr::new(0x01, 0x03, 0x03)));
        assert!(!node.accepts(NodeAddr::new(0x02, 0x02, 0x03)));
    }

    #[test]
    fn debug_format() {
        let addr = NodeAddr::new(0x01, 0x0A, 0xFF);
        let s = format!("{:?}", addr);
        assert_eq!(s, "NodeAddr(01:0A:FF)");
    }

    #[test]
    fn display_format() {
        let addr = NodeAddr::new(0x01, 0x0A, 0xFF);
        let s = format!("{}", addr);
        assert_eq!(s, "01:0A:FF");
    }

    #[test]
    fn equality_and_hash() {
        use core::hash::{Hash, Hasher};

        let a = NodeAddr::new(1, 2, 3);
        let b = NodeAddr::new(1, 2, 3);
        let c = NodeAddr::new(1, 2, 4);
        assert_eq!(a, b);
        assert_ne!(a, c);

        let hash_of = |addr: NodeAddr| -> u64 {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            addr.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash_of(a), hash_of(b));
    }

    #[test]
    fn const_construction() {
        const ADDR: NodeAddr = NodeAddr::new(0x10, 0x20, 0x30);
        const PACKED: u32 = ADDR.to_u32();
        const BACK: NodeAddr = NodeAddr::from_u32(PACKED);
        assert_eq!(ADDR, BACK);
        assert_eq!(PACKED, 0x00_10_20_30);
    }

    #[test]
    fn zero_address() {
        let addr = NodeAddr::new(0, 0, 0);
        assert_eq!(addr.to_u32(), 0);
        assert!(!addr.is_broadcast());
    }
}
