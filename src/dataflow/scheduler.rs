//! Tick scheduler: configurable dt, supports non-realtime batch execution.

use serde::{Deserialize, Serialize};

/// Controls how the graph advances time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scheduler {
    /// Fixed time step per tick (seconds).
    pub dt: f64,
    /// Speed multiplier: 1.0 = realtime, 2.0 = 2x, 0.0 = paused.
    pub speed: f64,
    /// Accumulated time not yet consumed by ticks (for realtime mode).
    accumulator: f64,
}

impl Scheduler {
    pub fn new(dt: f64) -> Self {
        Self {
            dt,
            speed: 1.0,
            accumulator: 0.0,
        }
    }

    /// Advance by wall-clock `elapsed` seconds.
    /// Returns the number of ticks to execute.
    pub fn advance(&mut self, elapsed: f64) -> u64 {
        self.accumulator += elapsed * self.speed;
        let ticks = (self.accumulator / self.dt).floor() as u64;
        self.accumulator -= ticks as f64 * self.dt;
        ticks
    }

    /// For non-realtime: return a fixed number of ticks to run.
    pub fn batch(steps: u64, dt: f64) -> (u64, f64) {
        (steps, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_counts_ticks() {
        let mut s = Scheduler::new(0.01); // 100 Hz
        assert_eq!(s.advance(0.035), 3); // 35ms = 3 ticks, 5ms remainder
        assert_eq!(s.advance(0.006), 1); // 5+6=11ms = 1 tick, 1ms left
    }

    #[test]
    fn speed_multiplier() {
        let mut s = Scheduler::new(0.01);
        s.speed = 2.0;
        assert_eq!(s.advance(0.015), 3); // 15ms * 2x = 30ms = 3 ticks
    }

    #[test]
    fn paused() {
        let mut s = Scheduler::new(0.01);
        s.speed = 0.0;
        assert_eq!(s.advance(1.0), 0);
    }
}
