//! Simulation mode enum for the DAG editor transport controls.
//!
//! This module is intentionally not gated behind `#[cfg(target_arch = "wasm32")]`
//! so that the pure logic can be tested on the host target.

/// Simulation execution mode.
///
/// Controls how (and when) the DAG simulation ticks:
///
/// - **Stopped** — no automatic ticking; the user must click Step.
/// - **Playing** — timer-driven ticking at a fixed interval (10 Hz).
/// - **Live** — reactive ticking: each `inject_topic` call triggers an
///   immediate tick so widgets get instant feedback.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SimMode {
    /// No auto-ticking.
    #[default]
    Stopped,
    /// Timer-driven ticking (existing interval-based behaviour).
    Playing,
    /// Reactive: tick immediately when a topic is injected.
    Live,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_stopped() {
        assert_eq!(SimMode::default(), SimMode::Stopped);
    }

    #[test]
    fn modes_are_distinct() {
        assert_ne!(SimMode::Live, SimMode::Playing);
        assert_ne!(SimMode::Live, SimMode::Stopped);
        assert_ne!(SimMode::Playing, SimMode::Stopped);
    }

    #[test]
    fn mode_is_copy() {
        let a = SimMode::Live;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn mode_debug_format() {
        // Ensure Debug is derived and produces reasonable output.
        let s = format!("{:?}", SimMode::Playing);
        assert!(s.contains("Playing"), "unexpected Debug output: {s}");
    }
}
