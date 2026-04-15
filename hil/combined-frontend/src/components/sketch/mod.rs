//! 2D sketch editor with constraint solver integration.
//!
//! Components in this module are `wasm32`-only (they use `web-sys` canvas APIs
//! and the rustcam sketch solver). The data types and pure helpers live in
//! [`crate::sketch`] so they can be unit-tested on the host.

pub mod canvas;
pub mod constraint_bridge;
pub mod editor;
