//! The `Tick` trait — pure computation executed every tick.

use alloc::vec::Vec;

use crate::value::Value;

/// Pure computation. Works in all three execution modes.
pub trait Tick {
    /// Process one tick. `inputs` is indexed by input port order.
    /// Returns one `Option<Value>` per output port.
    fn tick(&mut self, inputs: &[Option<&Value>], dt: f64) -> Vec<Option<Value>>;
}
