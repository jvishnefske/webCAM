//! Network configuration constants for CDC NCM Ethernet.
//!
//! These constants define the MAC addresses and IP configuration used by
//! the board-support-pico firmware for its CDC NCM Ethernet adapter.

/// Device-side MAC address for the CDC NCM interface.
pub const DEVICE_MAC: [u8; 6] = [0x02, 0x03, 0x04, 0x05, 0x06, 0x07];

/// Host-side MAC address for the CDC NCM interface.
pub const HOST_MAC: [u8; 6] = [0x02, 0x03, 0x04, 0x05, 0x06, 0x08];

/// IPv4 address octets (169.254.1.61 link-local).
pub const IPV4_ADDR: [u8; 4] = [169, 254, 1, 61];

/// IPv4 subnet prefix length.
pub const IPV4_PREFIX: u8 = 16;

/// IPv6 address segments (fe80::3:4ff:fe05:607 — EUI-64 from device MAC).
pub const IPV6_SEGMENTS: [u16; 8] = [0xfe80, 0, 0, 0, 0x0003, 0x04ff, 0xfe05, 0x0607];

/// IPv6 subnet prefix length.
pub const IPV6_PREFIX: u8 = 64;
