//! Combined Leptos frontend: HIL dashboard + DAG editor + deployment.
//!
//! Consolidates three separate frontends into one Leptos 0.7 CSR application
//! served from the Pico2 MCU's HTTP server. Shares `module-traits` and
//! `dag-core` types directly in Rust — no TypeScript mirror types.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod app;
pub mod backoff;
// Storage pure-logic is always compiled (tests run on host).
// The canonical file lives under components/dag/ and is re-exported here
// for non-wasm targets so `cargo test` works without the full component tree.
#[cfg(not(target_arch = "wasm32"))]
#[path = "components/dag/storage.rs"]
pub mod storage;

#[cfg(target_arch = "wasm32")]
pub mod components;
pub mod graph_engine;
pub mod hex;
pub mod messages;
pub mod theme;
pub mod types;
#[cfg(target_arch = "wasm32")]
pub mod ws_client;
