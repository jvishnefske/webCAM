//! UDP message backplane for HIL (Hardware-in-the-Loop) testing.
//!
//! Provides pub/sub and request/response messaging over UDP multicast.
//! The wire format uses a fixed 17-byte binary envelope header for fast
//! routing, with CBOR-encoded payloads via [`minicbor`].
//!
//! # Features
//!
//! - `std` (default): Enables [`transport`] and [`node`] modules for
//!   host-side UDP networking.
//! - `embassy`: Enables `embassy_node` for async UDP networking on
//!   `no_std` targets using embassy-net.
//! - Without `std` or `embassy`: Only the core types ([`envelope`],
//!   [`message`], [`node_id`], [`error`], [`discovery`], [`codec`],
//!   [`dispatcher`]) are available, suitable for `no_std` environments.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

pub mod codec;
pub mod dhcp;
pub mod dispatcher;
pub mod envelope;
pub mod error;
pub mod message;
pub mod node_id;

pub mod discovery;

#[cfg(feature = "std")]
pub mod node;
#[cfg(feature = "std")]
pub mod transport;

#[cfg(feature = "embassy")]
pub mod embassy_node;
