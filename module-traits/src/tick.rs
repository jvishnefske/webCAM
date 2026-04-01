//! The `Tick` trait — pure computation executed every tick.

use alloc::vec::Vec;

use crate::value::Value;

/// Pure computation. Works in all three execution modes.
pub trait Tick {
    /// Process one tick. `inputs` is indexed by input port order.
    /// Returns one `Option<Value>` per output port.
    fn tick(&mut self, inputs: &[Option<&Value>], dt: f64) -> Vec<Option<Value>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use alloc::vec;
    use alloc::vec::Vec;

    // --- Mock: passthrough (copies first input to output) ---
    struct Passthrough;

    impl Tick for Passthrough {
        fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
            vec![inputs.first().and_then(|opt| opt.map(Clone::clone))]
        }
    }

    #[test]
    fn test_tick_passthrough() {
        let mut block = Passthrough;
        let input = Value::Float(5.0);
        let outputs = block.tick(&[Some(&input)], 0.01);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], Some(Value::Float(5.0)));
    }

    // --- Mock: integrator (accumulates input * dt) ---
    struct Integrator {
        accumulator: f64,
    }

    impl Integrator {
        fn new() -> Self {
            Self { accumulator: 0.0 }
        }
    }

    impl Tick for Integrator {
        fn tick(&mut self, inputs: &[Option<&Value>], dt: f64) -> Vec<Option<Value>> {
            if let Some(Some(value)) = inputs.first() {
                if let Some(v) = value.as_float() {
                    self.accumulator += v * dt;
                }
            }
            vec![Some(Value::Float(self.accumulator))]
        }
    }

    #[test]
    fn test_tick_with_dt() {
        let mut integrator = Integrator::new();
        let input = Value::Float(10.0);

        // First tick: 10.0 * 0.1 = 1.0
        let out = integrator.tick(&[Some(&input)], 0.1);
        assert_eq!(out[0], Some(Value::Float(1.0)));

        // Second tick: 1.0 + 10.0 * 0.2 = 3.0
        let out = integrator.tick(&[Some(&input)], 0.2);
        assert_eq!(out[0], Some(Value::Float(3.0)));
    }

    // --- Mock: default-on-none (returns 0.0 when input is None) ---
    struct DefaultOnNone;

    impl Tick for DefaultOnNone {
        fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
            let val = inputs
                .first()
                .and_then(|opt| *opt)
                .and_then(|v| v.as_float())
                .unwrap_or(0.0);
            vec![Some(Value::Float(val))]
        }
    }

    #[test]
    fn test_tick_none_input() {
        let mut block = DefaultOnNone;

        // None input yields default 0.0
        let out = block.tick(&[None], 0.01);
        assert_eq!(out[0], Some(Value::Float(0.0)));

        // Empty inputs slice also yields default
        let out = block.tick(&[], 0.01);
        assert_eq!(out[0], Some(Value::Float(0.0)));

        // Some input yields the actual value
        let input = Value::Float(7.5);
        let out = block.tick(&[Some(&input)], 0.01);
        assert_eq!(out[0], Some(Value::Float(7.5)));
    }

    // --- Mock: multiple outputs (splits float into integer and fractional parts) ---
    struct SplitFloat;

    impl Tick for SplitFloat {
        fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
            let val = inputs
                .first()
                .and_then(|opt| *opt)
                .and_then(|v| v.as_float())
                .unwrap_or(0.0);
            let int_part = val.trunc();
            let frac_part = val.fract();
            vec![Some(Value::Float(int_part)), Some(Value::Float(frac_part))]
        }
    }

    #[test]
    fn test_tick_multiple_outputs() {
        let mut block = SplitFloat;
        let input = Value::Float(3.75);
        let out = block.tick(&[Some(&input)], 0.01);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], Some(Value::Float(3.0)));
        assert_eq!(out[1], Some(Value::Float(0.75)));
    }
}
