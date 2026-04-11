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
    fn encoder_read(&mut self, _channel: u8) -> i64 {
        0
    }
    /// Write two lines to an SSD1306 OLED display.
    fn display_write(&mut self, _bus: u8, _addr: u8, _line1: &str, _line2: &str) {}
    /// Command stepper to move to target position.
    fn stepper_move(&mut self, _port: u8, _target: i64) {}
    /// Read current stepper position.
    fn stepper_position(&self, _port: u8) -> i64 {
        0
    }
    /// Enable or disable stepper driver.
    fn stepper_enable(&mut self, _port: u8, _enabled: bool) {}
    /// Read TMC2209 StallGuard value.
    fn stallguard_read(&mut self, _port: u8, _addr: u8) -> u16 {
        0
    }
    /// Connect a virtual TCP socket.
    fn tcp_connect(&mut self, _id: u8, _addr: &str, _port: u16) -> Result<(), ()> {
        Err(())
    }
    /// Send data on a connected TCP socket.
    fn tcp_send(&mut self, _id: u8, _data: &[u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Receive data from a connected TCP socket.
    fn tcp_recv(&mut self, _id: u8, _buf: &mut [u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Close a TCP socket.
    fn tcp_close(&mut self, _id: u8) {}
    /// Send a UDP datagram.
    fn udp_send(&mut self, _id: u8, _addr: &str, _port: u16, _data: &[u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Receive a UDP datagram.
    fn udp_recv(&mut self, _id: u8, _buf: &mut [u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Write bytes to an I2C device on the given bus.
    fn i2c_write(&mut self, _bus: u8, _addr: u8, _data: &[u8]) -> Result<(), ()> {
        Err(())
    }
    /// Read bytes from an I2C device on the given bus.
    fn i2c_read(&mut self, _bus: u8, _addr: u8, _buf: &mut [u8]) -> Result<(), ()> {
        Err(())
    }
    /// Write then read (combined transaction) on an I2C bus.
    fn i2c_write_read(
        &mut self,
        _bus: u8,
        _addr: u8,
        _write: &[u8],
        _read: &mut [u8],
    ) -> Result<(), ()> {
        Err(())
    }
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

    // --- Mock Peripherals ---

    struct MockPeripherals {
        adc_values: [f32; 4],
        pwm_duties: [f32; 4],
        gpio_pins: [bool; 8],
    }

    impl MockPeripherals {
        fn new() -> Self {
            Self {
                adc_values: [0.0; 4],
                pwm_duties: [0.0; 4],
                gpio_pins: [false; 8],
            }
        }
    }

    impl Peripherals for MockPeripherals {
        fn adc_read(&mut self, channel: u8) -> f32 {
            self.adc_values[channel as usize]
        }
        fn pwm_write(&mut self, channel: u8, duty: f32) {
            self.pwm_duties[channel as usize] = duty;
        }
        fn gpio_read(&self, pin: u8) -> bool {
            self.gpio_pins[pin as usize]
        }
        fn gpio_write(&mut self, pin: u8, high: bool) {
            self.gpio_pins[pin as usize] = high;
        }
        fn uart_write(&mut self, _port: u8, _data: &[u8]) {}
        fn uart_read(&mut self, _port: u8, _buf: &mut [u8]) -> usize {
            0
        }
    }

    // --- Peripherals trait tests ---

    #[test]
    fn test_peripherals_mock_adc_read() {
        let mut p = MockPeripherals::new();
        p.adc_values[0] = 0.5;
        p.adc_values[2] = 0.75;
        assert!((p.adc_read(0) - 0.5).abs() < f32::EPSILON);
        assert!((p.adc_read(1) - 0.0).abs() < f32::EPSILON);
        assert!((p.adc_read(2) - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_peripherals_mock_pwm_write() {
        let mut p = MockPeripherals::new();
        p.pwm_write(1, 0.33);
        p.pwm_write(3, 0.99);
        assert!((p.pwm_duties[1] - 0.33).abs() < f32::EPSILON);
        assert!((p.pwm_duties[3] - 0.99).abs() < f32::EPSILON);
        // Channels not written remain at zero.
        assert!((p.pwm_duties[0] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_peripherals_mock_gpio() {
        let mut p = MockPeripherals::new();
        assert!(!p.gpio_read(3));
        p.gpio_write(3, true);
        assert!(p.gpio_read(3));
        p.gpio_write(3, false);
        assert!(!p.gpio_read(3));
    }

    #[test]
    fn test_peripherals_default_encoder_read() {
        let mut p = MockPeripherals::new();
        assert_eq!(p.encoder_read(0), 0);
        assert_eq!(p.encoder_read(1), 0);
    }

    #[test]
    fn test_peripherals_default_stepper_position() {
        let p = MockPeripherals::new();
        assert_eq!(p.stepper_position(0), 0);
        assert_eq!(p.stepper_position(2), 0);
    }

    #[test]
    fn test_peripherals_default_stallguard_read() {
        let mut p = MockPeripherals::new();
        assert_eq!(p.stallguard_read(0, 0), 0);
        assert_eq!(p.stallguard_read(1, 3), 0);
    }

    #[test]
    fn test_peripherals_default_display_write() {
        let mut p = MockPeripherals::new();
        // display_write has a default no-op implementation -- should not panic
        p.display_write(0, 0x3C, "line1", "line2");
    }

    #[test]
    fn test_peripherals_default_stepper_move() {
        let mut p = MockPeripherals::new();
        // stepper_move has a default no-op implementation -- should not panic
        p.stepper_move(0, 1000);
    }

    #[test]
    fn test_peripherals_default_stepper_enable() {
        let mut p = MockPeripherals::new();
        // stepper_enable has a default no-op implementation -- should not panic
        p.stepper_enable(0, true);
        p.stepper_enable(0, false);
    }

    #[test]
    fn test_peripherals_tcp_connect_default_err() {
        let mut p = MockPeripherals::new();
        assert!(p.tcp_connect(0, "127.0.0.1", 8080).is_err());
    }

    #[test]
    fn test_peripherals_tcp_send_default_err() {
        let mut p = MockPeripherals::new();
        assert!(p.tcp_send(0, b"hello").is_err());
    }

    #[test]
    fn test_peripherals_tcp_recv_default_err() {
        let mut p = MockPeripherals::new();
        let mut buf = [0u8; 16];
        assert!(p.tcp_recv(0, &mut buf).is_err());
    }

    #[test]
    fn test_peripherals_tcp_close_default_noop() {
        let mut p = MockPeripherals::new();
        p.tcp_close(0); // should not panic
    }

    #[test]
    fn test_peripherals_udp_send_default_err() {
        let mut p = MockPeripherals::new();
        assert!(p.udp_send(0, "127.0.0.1", 9000, b"data").is_err());
    }

    #[test]
    fn test_peripherals_udp_recv_default_err() {
        let mut p = MockPeripherals::new();
        let mut buf = [0u8; 16];
        assert!(p.udp_recv(0, &mut buf).is_err());
    }

    #[test]
    fn test_peripherals_i2c_write_default_err() {
        let mut p = MockPeripherals::new();
        assert!(p.i2c_write(0, 0x50, &[0x00, 0x42]).is_err());
    }

    #[test]
    fn test_peripherals_i2c_read_default_err() {
        let mut p = MockPeripherals::new();
        let mut buf = [0u8; 4];
        assert!(p.i2c_read(0, 0x50, &mut buf).is_err());
    }

    #[test]
    fn test_peripherals_i2c_write_read_default_err() {
        let mut p = MockPeripherals::new();
        let mut buf = [0u8; 4];
        assert!(p.i2c_write_read(0, 0x50, &[0x00], &mut buf).is_err());
    }

    #[test]
    fn test_peripherals_uart_ops() {
        let mut p = MockPeripherals::new();
        p.uart_write(0, &[0x01, 0x02]);
        let mut buf = [0u8; 4];
        let n = p.uart_read(0, &mut buf);
        assert_eq!(n, 0);
    }

    // --- Block trait tests ---

    struct Gain {
        factor: f32,
    }

    impl Block for Gain {
        type Input = f32;
        type Output = f32;

        fn tick(&mut self, input: Self::Input, _dt: f32) -> Self::Output {
            input * self.factor
        }
    }

    #[test]
    fn test_block_gain() {
        let mut g = Gain { factor: 2.5 };
        let out = g.tick(4.0, 0.01);
        assert!((out - 10.0).abs() < f32::EPSILON);
        let out2 = g.tick(0.0, 0.01);
        assert!((out2 - 0.0).abs() < f32::EPSILON);
        let out3 = g.tick(-3.0, 0.01);
        assert!((out3 - -7.5).abs() < f32::EPSILON);
    }

    struct Integrator {
        accumulator: f32,
    }

    impl Block for Integrator {
        type Input = f32;
        type Output = f32;

        fn tick(&mut self, input: Self::Input, dt: f32) -> Self::Output {
            self.accumulator += input * dt;
            self.accumulator
        }
    }

    #[test]
    fn test_block_integrator() {
        let mut integ = Integrator { accumulator: 0.0 };
        // Constant input of 10.0 over 0.1s per tick for 5 ticks = 5.0
        let dt = 0.1;
        for _ in 0..5 {
            integ.tick(10.0, dt);
        }
        assert!((integ.accumulator - 5.0).abs() < 1e-5);
        // One more tick with different input
        let out = integ.tick(-20.0, 0.1);
        assert!((out - 3.0).abs() < 1e-5);
    }

    // --- Additional RingBuffer tests ---

    #[test]
    fn test_ring_buffer_single_element() {
        let mut rb = RingBuffer::<1>::new();
        assert!(rb.is_empty());
        rb.push(42.0);
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.get(0), Some(42.0));
        // Pushing again overwrites the single slot.
        rb.push(99.0);
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.get(0), Some(99.0));
        // Old value is gone.
        assert_eq!(rb.get(1), None);
    }

    #[test]
    fn test_ring_buffer_exact_capacity() {
        let mut rb = RingBuffer::<5>::new();
        for i in 0..5 {
            rb.push(i as f32);
        }
        assert_eq!(rb.len(), 5);
        for i in 0..5 {
            assert_eq!(rb.get(i), Some(i as f32));
        }
        assert_eq!(rb.get(5), None);
    }

    #[test]
    fn test_ring_buffer_iter_exact_size() {
        let mut rb = RingBuffer::<4>::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        let mut it = rb.iter();
        assert_eq!(it.len(), 3);
        it.next();
        assert_eq!(it.len(), 2);
        it.next();
        assert_eq!(it.len(), 1);
        it.next();
        assert_eq!(it.len(), 0);
        assert!(it.next().is_none());
    }

    #[test]
    fn test_ring_buffer_default_trait() {
        let from_default = RingBuffer::<4>::default();
        let from_new = RingBuffer::<4>::new();
        assert_eq!(from_default.len(), from_new.len());
        assert_eq!(from_default.is_empty(), from_new.is_empty());
        assert_eq!(from_default.buf, from_new.buf);
        assert_eq!(from_default.head, from_new.head);
    }
}
