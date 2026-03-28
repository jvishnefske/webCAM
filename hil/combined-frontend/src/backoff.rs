//! Exponential backoff logic for WebSocket reconnection.
//!
//! Provides pure functions to compute reconnect delays that double on each
//! attempt, capping at [`MAX_BACKOFF_MS`].

/// Maximum reconnect delay in milliseconds.
pub const MAX_BACKOFF_MS: u32 = 30_000;

/// Initial reconnect delay in milliseconds.
pub const INITIAL_BACKOFF_MS: u32 = 1_000;

/// Compute the next backoff delay, doubling until [`MAX_BACKOFF_MS`].
pub fn next_backoff(current_ms: u32) -> u32 {
    let next = current_ms.saturating_mul(2);
    next.min(MAX_BACKOFF_MS)
}

/// Return the initial backoff delay in milliseconds.
pub fn initial_backoff() -> u32 {
    INITIAL_BACKOFF_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_backoff_is_1000() {
        assert_eq!(initial_backoff(), 1000);
    }

    #[test]
    fn next_backoff_doubles() {
        assert_eq!(next_backoff(1000), 2000);
        assert_eq!(next_backoff(2000), 4000);
    }

    #[test]
    fn next_backoff_caps_at_max() {
        assert_eq!(next_backoff(16_000), 30_000);
    }

    #[test]
    fn next_backoff_at_max_stays() {
        assert_eq!(next_backoff(30_000), 30_000);
    }

    #[test]
    fn next_backoff_saturates() {
        // u32::MAX should not panic, just clamp to MAX_BACKOFF_MS
        assert_eq!(next_backoff(u32::MAX), MAX_BACKOFF_MS);
    }
}
