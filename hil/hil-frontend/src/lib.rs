//! HIL dashboard library.
//!
//! Provides CBOR message types, hex parsing utilities, and backoff logic
//! for the HIL WebSocket frontend. WASM-specific modules (app, components,
//! WebSocket client) are only compiled for `wasm32` targets.
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod app;
pub mod backoff;
#[cfg(target_arch = "wasm32")]
pub mod components;
pub mod hex;
pub mod messages;
#[cfg(target_arch = "wasm32")]
pub mod ws_client;
