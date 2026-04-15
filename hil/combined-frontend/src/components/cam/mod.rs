//! CAM panel — file upload, machine configuration, and G-code generation.
//!
//! Uses `rustcam` as a direct Rust dependency (both crates target
//! `wasm32-unknown-unknown`) to process STL and SVG files into G-code.
//!
//! The configuration builder lives in [`crate::cam_config`] so it can be
//! tested on native targets (not behind `#[cfg(target_arch = "wasm32")]`).

mod panel;
pub mod preview;
pub mod simulation;

pub use crate::cam_config::{build_cam_config, CamParams};
pub use panel::CamPanel;
