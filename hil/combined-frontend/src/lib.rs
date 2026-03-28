//! Combined Leptos frontend: HIL dashboard + DAG editor + deployment.
//!
//! Consolidates three separate frontends into one Leptos 0.7 CSR application
//! served from the Pico2 MCU's HTTP server. Shares `module-traits` and
//! `dag-core` types directly in Rust — no TypeScript mirror types.

// Note: unsafe impl Send/Sync for SendWrapper in app.rs (WASM is single-threaded)

#[cfg(target_arch = "wasm32")]
pub mod app;
pub mod backoff;
#[cfg(target_arch = "wasm32")]
pub mod components;
pub mod hex;
pub mod messages;
pub mod types;
#[cfg(target_arch = "wasm32")]
pub mod ws_client;
