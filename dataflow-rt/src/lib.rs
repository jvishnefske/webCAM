//! Minimal runtime for generated dataflow code.
//!
//! `no_std` by default — enable the `std` feature for hosted targets.

#![cfg_attr(not(feature = "std"), no_std)]

/// Hardware peripheral abstraction for generated dataflow code.
///
/// Bridges the logic library and target-specific hardware. Only methods
/// actually used by the graph get called at runtime.
pub trait Peripherals {
    /// Read a normalized 0.0–1.0 value from an ADC channel.
    fn adc_read(&mut self, channel: u8) -> f32;
    /// Set PWM duty cycle (0.0–1.0) on a channel.
    fn pwm_write(&mut self, channel: u8, duty: f32);
    /// Read a digital GPIO pin.
    fn gpio_read(&self, pin: u8) -> bool;
    /// Write a digital GPIO pin.
    fn gpio_write(&mut self, pin: u8, high: bool);
    /// Transmit bytes on a UART port.
    fn uart_write(&mut self, port: u8, data: &[u8]);
    /// Receive bytes from a UART port. Returns number of bytes read.
    fn uart_read(&mut self, port: u8, buf: &mut [u8]) -> usize;
    /// Read quadrature encoder accumulated count.
    fn encoder_read(&mut self, channel: u8) -> i64 { 0 }
    /// Write two lines to an SSD1306 OLED display.
    fn display_write(&mut self, bus: u8, addr: u8, line1: &str, line2: &str) {}
    /// Command stepper to move to target position.
    fn stepper_move(&mut self, port: u8, target: i64) {}
    /// Read current stepper position.
    fn stepper_position(&self, port: u8) -> i64 { 0 }
    /// Enable or disable stepper driver.
    fn stepper_enable(&mut self, port: u8, enabled: bool) {}
    /// Read TMC2209 StallGuard value.
    fn stallguard_read(&mut self, port: u8, addr: u8) -> u16 { 0 }
}

/// A processing node in a dataflow graph.
///
/// Generated code implements this trait for each block that has internal
/// state (e.g. integrators, filters). Stateless blocks (gain, add) are
/// emitted as plain functions instead.
pub trait Block {
    /// Input tuple type.
    type Input;
    /// Output tuple type.
    type Output;

    /// Process one tick.
    fn tick(&mut self, input: Self::Input, dt: f32) -> Self::Output;
}

/// Fixed-capacity ring buffer for time-series data.
/// Used by plot/accumulator blocks on embedded targets where `Vec` is unavailable.
pub struct RingBuffer<const N: usize> {
    buf: [f32; N],
    len: usize,
    head: usize,
}

impl<const N: usize> Default for RingBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RingBuffer<N> {
    /// Create a new empty ring buffer.
    pub const fn new() -> Self {
        Self {
            buf: [0.0; N],
            len: 0,
            head: 0,
        }
    }

    /// Push a value, overwriting the oldest if full.
    pub fn push(&mut self, value: f32) {
        self.buf[self.head] = value;
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Number of values stored.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a value by index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<f32> {
        if index >= self.len {
            return None;
        }
        let actual = if self.len < N {
            index
        } else {
            (self.head + index) % N
        };
        Some(self.buf[actual])
    }

    /// Iterate over values from oldest to newest.
    pub fn iter(&self) -> RingBufferIter<'_, N> {
        RingBufferIter {
            buf: self,
            index: 0,
        }
    }
}

/// Iterator over ring buffer values.
pub struct RingBufferIter<'a, const N: usize> {
    buf: &'a RingBuffer<N>,
    index: usize,
}

impl<const N: usize> Iterator for RingBufferIter<'_, N> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.buf.get(self.index)?;
        self.index += 1;
        Some(val)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buf.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<const N: usize> ExactSizeIterator for RingBufferIter<'_, N> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_push_and_get() {
        let mut rb = RingBuffer::<4>::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(1.0));
        assert_eq!(rb.get(2), Some(3.0));
        assert_eq!(rb.get(3), None);
    }

    #[test]
    fn ring_buffer_wraps() {
        let mut rb = RingBuffer::<3>::new();
        for i in 0..5 {
            rb.push(i as f32);
        }
        assert_eq!(rb.len(), 3);
        // Should contain [2.0, 3.0, 4.0]
        assert_eq!(rb.get(0), Some(2.0));
        assert_eq!(rb.get(1), Some(3.0));
        assert_eq!(rb.get(2), Some(4.0));
    }

    #[test]
    fn ring_buffer_iter() {
        let mut rb = RingBuffer::<3>::new();
        rb.push(10.0);
        rb.push(20.0);
        rb.push(30.0);
        rb.push(40.0); // overwrites 10.0
        let vals: Vec<f32> = rb.iter().collect();
        assert_eq!(vals, vec![20.0, 30.0, 40.0]);
    }

    #[test]
    fn ring_buffer_empty() {
        let rb = RingBuffer::<8>::new();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.iter().count(), 0);
    }
}
