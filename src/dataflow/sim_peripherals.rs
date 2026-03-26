//! WASM-compatible simulated peripherals for browser simulation mode.

use std::collections::HashMap;

use module_traits::SimPeripherals;

/// Simulated peripheral state for browser-based simulation.
///
/// Holds virtual ADC voltages, PWM duties, GPIO pin states, encoder positions,
/// display text, and stepper positions. Users configure input values (e.g. ADC
/// voltages) via the WASM API, and blocks read/write through `SimPeripherals`.
pub struct WasmSimPeripherals {
    adc_voltages: HashMap<u8, f64>,
    pwm_duties: HashMap<u8, f64>,
    gpio_states: HashMap<u8, bool>,
    uart_buffers: HashMap<u8, Vec<u8>>,
    encoder_positions: HashMap<u8, i64>,
    display_lines: HashMap<(u8, u8), (String, String)>,
    stepper_positions: HashMap<u8, i64>,
    stepper_targets: HashMap<u8, i64>,
}

impl WasmSimPeripherals {
    pub fn new() -> Self {
        Self {
            adc_voltages: HashMap::new(),
            pwm_duties: HashMap::new(),
            gpio_states: HashMap::new(),
            uart_buffers: HashMap::new(),
            encoder_positions: HashMap::new(),
            display_lines: HashMap::new(),
            stepper_positions: HashMap::new(),
            stepper_targets: HashMap::new(),
        }
    }

    /// Set a simulated ADC channel voltage (called from WASM API).
    pub fn set_adc_voltage(&mut self, channel: u8, voltage: f64) {
        self.adc_voltages.insert(channel, voltage);
    }

    /// Read the last PWM duty written by a block.
    pub fn get_pwm_duty(&self, channel: u8) -> f64 {
        self.pwm_duties.get(&channel).copied().unwrap_or(0.0)
    }

    /// Set a simulated GPIO pin state (called from WASM API).
    pub fn set_gpio_state(&mut self, pin: u8, high: bool) {
        self.gpio_states.insert(pin, high);
    }

    /// Get the current GPIO pin state.
    pub fn get_gpio_state(&self, pin: u8) -> bool {
        self.gpio_states.get(&pin).copied().unwrap_or(false)
    }

    /// Set a simulated encoder position.
    pub fn set_encoder_position(&mut self, channel: u8, position: i64) {
        self.encoder_positions.insert(channel, position);
    }

    /// Push data into a simulated UART receive buffer.
    pub fn push_uart_data(&mut self, port: u8, data: &[u8]) {
        self.uart_buffers
            .entry(port)
            .or_default()
            .extend_from_slice(data);
    }

    /// Get the display lines for a given bus/address.
    pub fn get_display_lines(&self, bus: u8, addr: u8) -> Option<&(String, String)> {
        self.display_lines.get(&(bus, addr))
    }

    /// Get a stepper's current position.
    pub fn get_stepper_position(&self, port: u8) -> i64 {
        self.stepper_positions.get(&port).copied().unwrap_or(0)
    }
}

impl Default for WasmSimPeripherals {
    fn default() -> Self {
        Self::new()
    }
}

impl SimPeripherals for WasmSimPeripherals {
    fn adc_read(&mut self, channel: u8) -> f64 {
        self.adc_voltages.get(&channel).copied().unwrap_or(0.0)
    }

    fn pwm_write(&mut self, channel: u8, duty: f64) {
        self.pwm_duties.insert(channel, duty);
    }

    fn gpio_read(&self, pin: u8) -> bool {
        self.gpio_states.get(&pin).copied().unwrap_or(false)
    }

    fn gpio_write(&mut self, pin: u8, high: bool) {
        self.gpio_states.insert(pin, high);
    }

    fn uart_write(&mut self, port: u8, data: &[u8]) {
        self.uart_buffers
            .entry(port)
            .or_default()
            .extend_from_slice(data);
    }

    fn uart_read(&mut self, port: u8, buf: &mut [u8]) -> usize {
        if let Some(buffer) = self.uart_buffers.get_mut(&port) {
            let n = buf.len().min(buffer.len());
            buf[..n].copy_from_slice(&buffer[..n]);
            buffer.drain(..n);
            n
        } else {
            0
        }
    }

    fn encoder_read(&mut self, channel: u8) -> i64 {
        self.encoder_positions.get(&channel).copied().unwrap_or(0)
    }

    fn display_write(&mut self, bus: u8, addr: u8, line1: &str, line2: &str) {
        self.display_lines
            .insert((bus, addr), (line1.to_string(), line2.to_string()));
    }

    fn stepper_move(&mut self, port: u8, target: i64) {
        self.stepper_targets.insert(port, target);
        // Simple instant-move simulation for now.
        self.stepper_positions.insert(port, target);
    }

    fn stepper_position(&self, port: u8) -> i64 {
        self.stepper_positions.get(&port).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adc_read_write() {
        let mut p = WasmSimPeripherals::new();
        p.set_adc_voltage(0, 3.3);
        assert_eq!(p.adc_read(0), 3.3);
        assert_eq!(p.adc_read(1), 0.0);
    }

    #[test]
    fn pwm_write_read() {
        let mut p = WasmSimPeripherals::new();
        p.pwm_write(2, 0.75);
        assert_eq!(p.get_pwm_duty(2), 0.75);
        assert_eq!(p.get_pwm_duty(3), 0.0);
    }

    #[test]
    fn gpio_roundtrip() {
        let mut p = WasmSimPeripherals::new();
        p.gpio_write(5, true);
        assert!(p.gpio_read(5));
        assert!(!p.gpio_read(6));
    }

    #[test]
    fn uart_roundtrip() {
        let mut p = WasmSimPeripherals::new();
        p.uart_write(0, b"hello");
        let mut buf = [0u8; 10];
        let n = p.uart_read(0, &mut buf);
        assert_eq!(n, 5);
        assert_eq!(&buf[..5], b"hello");
        // Buffer should be drained
        assert_eq!(p.uart_read(0, &mut buf), 0);
    }

    #[test]
    fn encoder_read() {
        let mut p = WasmSimPeripherals::new();
        p.set_encoder_position(0, 1024);
        assert_eq!(p.encoder_read(0), 1024);
    }

    #[test]
    fn stepper_move() {
        let mut p = WasmSimPeripherals::new();
        p.stepper_move(0, 500);
        assert_eq!(p.stepper_position(0), 500);
    }

    #[test]
    fn set_get_gpio_state() {
        let mut p = WasmSimPeripherals::new();
        p.set_gpio_state(3, true);
        assert!(p.get_gpio_state(3));
        assert!(!p.get_gpio_state(4));
    }

    #[test]
    fn push_uart_data() {
        let mut p = WasmSimPeripherals::new();
        p.push_uart_data(1, b"test");
        let mut buf = [0u8; 10];
        let n = p.uart_read(1, &mut buf);
        assert_eq!(n, 4);
        assert_eq!(&buf[..4], b"test");
    }

    #[test]
    fn get_stepper_position_default() {
        let p = WasmSimPeripherals::new();
        assert_eq!(p.get_stepper_position(0), 0);
    }

    #[test]
    fn display_write() {
        let mut p = WasmSimPeripherals::new();
        p.display_write(0, 0x3C, "Hello", "World");
        let lines = p.get_display_lines(0, 0x3C).unwrap();
        assert_eq!(lines.0, "Hello");
        assert_eq!(lines.1, "World");
    }
}
