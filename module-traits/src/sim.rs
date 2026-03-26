//! The `SimModel` trait and `SimPeripherals` interface for simulated hardware.

use alloc::vec::Vec;

use crate::value::Value;

/// Simulated hardware interaction. For peripheral blocks in simulation mode.
pub trait SimModel {
    /// Process one simulation tick with access to simulated peripherals.
    fn sim_tick(
        &mut self,
        inputs: &[Option<&Value>],
        dt: f64,
        peripherals: &mut dyn SimPeripherals,
    ) -> Vec<Option<Value>>;
}

/// Simulated hardware environment providing access to virtual peripherals.
pub trait SimPeripherals {
    fn adc_read(&mut self, channel: u8) -> f64;
    fn pwm_write(&mut self, channel: u8, duty: f64);
    fn gpio_read(&self, pin: u8) -> bool;
    fn gpio_write(&mut self, pin: u8, high: bool);
    fn uart_write(&mut self, port: u8, data: &[u8]);
    fn uart_read(&mut self, port: u8, buf: &mut [u8]) -> usize;
    fn encoder_read(&mut self, channel: u8) -> i64;
    fn display_write(&mut self, bus: u8, addr: u8, line1: &str, line2: &str);
    fn stepper_move(&mut self, port: u8, target: i64);
    fn stepper_position(&self, port: u8) -> i64;
}
