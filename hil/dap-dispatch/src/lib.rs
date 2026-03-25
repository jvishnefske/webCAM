//! CMSIS-DAP protocol dispatch library.
//!
//! Provides a transport-agnostic [`DapProcessor`](protocol::DapProcessor) trait
//! for CMSIS-DAP command processing, CBOR tag 40 encode/decode helpers for
//! WebSocket transport, and an optional embassy-usb vendor class for USB bulk
//! transport.
//!
//! # Architecture
//!
//! ```text
//! Board crate (owns Dap<...>)
//!    │
//!    ├── implements DapProcessor
//!    │
//!    └── dap-dispatch
//!         ├── cbor_dispatch: Tag 40 CBOR encode/decode
//!         └── usb_class: embassy-usb vendor interface (feature-gated)
//! ```
//!
//! The library does not depend on any specific CMSIS-DAP implementation.
//! Board crates provide the concrete [`DapProcessor`](protocol::DapProcessor)
//! impl by wrapping their chosen DAP backend.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_in_result)]

pub mod cbor_dispatch;
pub mod protocol;
pub mod stub;
#[cfg(feature = "embassy-usb")]
pub mod usb_class;
