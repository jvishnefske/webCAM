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
    /// Connect a virtual TCP socket.
    #[allow(clippy::result_unit_err)]
    fn tcp_connect(&mut self, _id: u8, _addr: &str, _port: u16) -> Result<(), ()> {
        Err(())
    }
    /// Send data on a connected TCP socket.
    #[allow(clippy::result_unit_err)]
    fn tcp_send(&mut self, _id: u8, _data: &[u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Receive data from a connected TCP socket.
    #[allow(clippy::result_unit_err)]
    fn tcp_recv(&mut self, _id: u8, _buf: &mut [u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Close a TCP socket.
    fn tcp_close(&mut self, _id: u8) {}
    /// Send a UDP datagram.
    #[allow(clippy::result_unit_err)]
    fn udp_send(&mut self, _id: u8, _addr: &str, _port: u16, _data: &[u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Receive a UDP datagram.
    #[allow(clippy::result_unit_err)]
    fn udp_recv(&mut self, _id: u8, _buf: &mut [u8]) -> Result<usize, ()> {
        Err(())
    }
    /// Write bytes to an I2C device on the given bus.
    #[allow(clippy::result_unit_err)]
    fn i2c_write(&mut self, _bus: u8, _addr: u8, _data: &[u8]) -> Result<(), ()> {
        Err(())
    }
    /// Read bytes from an I2C device on the given bus.
    #[allow(clippy::result_unit_err)]
    fn i2c_read(&mut self, _bus: u8, _addr: u8, _buf: &mut [u8]) -> Result<(), ()> {
        Err(())
    }
    /// Write then read (combined transaction) on an I2C bus.
    #[allow(clippy::result_unit_err)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use alloc::vec;
    use alloc::vec::Vec;

    // --- Mock SimPeripherals ---
    struct MockPeripherals {
        adc_values: [f64; 4],
        pwm_duties: [f64; 4],
        gpio_pins: [bool; 8],
        encoder_counts: [i64; 2],
        stepper_positions: [i64; 2],
    }

    impl MockPeripherals {
        fn new() -> Self {
            Self {
                adc_values: [0.0; 4],
                pwm_duties: [0.0; 4],
                gpio_pins: [false; 8],
                encoder_counts: [0; 2],
                stepper_positions: [0; 2],
            }
        }
    }

    impl SimPeripherals for MockPeripherals {
        fn adc_read(&mut self, ch: u8) -> f64 {
            self.adc_values[ch as usize]
        }
        fn pwm_write(&mut self, ch: u8, duty: f64) {
            self.pwm_duties[ch as usize] = duty;
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
        fn encoder_read(&mut self, ch: u8) -> i64 {
            self.encoder_counts[ch as usize]
        }
        fn display_write(&mut self, _bus: u8, _addr: u8, _l1: &str, _l2: &str) {}
        fn stepper_move(&mut self, port: u8, target: i64) {
            self.stepper_positions[port as usize] = target;
        }
        fn stepper_position(&self, port: u8) -> i64 {
            self.stepper_positions[port as usize]
        }
    }

    // --- Mock SimModel: reads ADC channel 0 and outputs the value ---
    struct AdcReader;

    impl SimModel for AdcReader {
        fn sim_tick(
            &mut self,
            _inputs: &[Option<&Value>],
            _dt: f64,
            peripherals: &mut dyn SimPeripherals,
        ) -> Vec<Option<Value>> {
            let reading = peripherals.adc_read(0);
            vec![Some(Value::Float(reading))]
        }
    }

    #[test]
    fn test_sim_model_adc_read() {
        let mut model = AdcReader;
        let mut periph = MockPeripherals::new();
        periph.adc_values[0] = 3.3;

        let out = model.sim_tick(&[], 0.01, &mut periph);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], Some(Value::Float(3.3)));
    }

    // --- Mock SimModel: reads Float input and writes to PWM channel 1 ---
    struct PwmWriter;

    impl SimModel for PwmWriter {
        fn sim_tick(
            &mut self,
            inputs: &[Option<&Value>],
            _dt: f64,
            peripherals: &mut dyn SimPeripherals,
        ) -> Vec<Option<Value>> {
            if let Some(Some(value)) = inputs.first() {
                if let Some(duty) = value.as_float() {
                    peripherals.pwm_write(1, duty);
                }
            }
            vec![]
        }
    }

    #[test]
    fn test_sim_model_pwm_write() {
        let mut model = PwmWriter;
        let mut periph = MockPeripherals::new();
        let input = Value::Float(0.75);

        let out = model.sim_tick(&[Some(&input)], 0.01, &mut periph);
        assert!(out.is_empty());
        assert!((periph.pwm_duties[1] - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sim_peripherals_gpio_roundtrip() {
        let mut periph = MockPeripherals::new();

        // Initially false
        assert!(!periph.gpio_read(3));

        // Write high, read back
        periph.gpio_write(3, true);
        assert!(periph.gpio_read(3));

        // Write low, read back
        periph.gpio_write(3, false);
        assert!(!periph.gpio_read(3));
    }

    #[test]
    fn test_sim_peripherals_encoder_read() {
        let mut periph = MockPeripherals::new();
        periph.encoder_counts[0] = 4096;
        periph.encoder_counts[1] = -200;

        assert_eq!(periph.encoder_read(0), 4096);
        assert_eq!(periph.encoder_read(1), -200);
    }

    #[test]
    fn test_sim_peripherals_uart_roundtrip() {
        let mut periph = MockPeripherals::new();
        // uart_write is a no-op in mock, uart_read returns 0
        periph.uart_write(0, &[0x01, 0x02]);
        let mut buf = [0u8; 4];
        let n = periph.uart_read(0, &mut buf);
        assert_eq!(n, 0);
    }

    #[test]
    fn test_sim_peripherals_display_write() {
        let mut periph = MockPeripherals::new();
        // display_write is a no-op in mock -- should not panic
        periph.display_write(0, 0x3C, "hello", "world");
    }

    // --- Socket trait method tests ---

    #[test]
    fn test_sim_peripherals_tcp_connect_default_err() {
        let mut periph = MockPeripherals::new();
        assert!(periph.tcp_connect(0, "127.0.0.1", 8080).is_err());
    }

    #[test]
    fn test_sim_peripherals_tcp_send_default_err() {
        let mut periph = MockPeripherals::new();
        assert!(periph.tcp_send(0, b"hello").is_err());
    }

    #[test]
    fn test_sim_peripherals_tcp_recv_default_err() {
        let mut periph = MockPeripherals::new();
        let mut buf = [0u8; 16];
        assert!(periph.tcp_recv(0, &mut buf).is_err());
    }

    #[test]
    fn test_sim_peripherals_tcp_close_default_noop() {
        let mut periph = MockPeripherals::new();
        periph.tcp_close(0); // should not panic
    }

    #[test]
    fn test_sim_peripherals_udp_send_default_err() {
        let mut periph = MockPeripherals::new();
        assert!(periph.udp_send(0, "127.0.0.1", 9000, b"data").is_err());
    }

    #[test]
    fn test_sim_peripherals_udp_recv_default_err() {
        let mut periph = MockPeripherals::new();
        let mut buf = [0u8; 16];
        assert!(periph.udp_recv(0, &mut buf).is_err());
    }

    // --- I2C trait method tests ---

    #[test]
    fn test_sim_peripherals_i2c_write_default_err() {
        let mut periph = MockPeripherals::new();
        assert!(periph.i2c_write(0, 0x50, &[0x00, 0x42]).is_err());
    }

    #[test]
    fn test_sim_peripherals_i2c_read_default_err() {
        let mut periph = MockPeripherals::new();
        let mut buf = [0u8; 4];
        assert!(periph.i2c_read(0, 0x50, &mut buf).is_err());
    }

    #[test]
    fn test_sim_peripherals_i2c_write_read_default_err() {
        let mut periph = MockPeripherals::new();
        let mut buf = [0u8; 4];
        assert!(periph.i2c_write_read(0, 0x50, &[0x00], &mut buf).is_err());
    }

    #[test]
    fn test_sim_peripherals_stepper_position() {
        let mut periph = MockPeripherals::new();

        // Initially at zero
        assert_eq!(periph.stepper_position(0), 0);

        // Move to a target, verify position
        periph.stepper_move(0, 1000);
        assert_eq!(periph.stepper_position(0), 1000);

        // Move to a negative target
        periph.stepper_move(1, -500);
        assert_eq!(periph.stepper_position(1), -500);

        // First stepper unchanged
        assert_eq!(periph.stepper_position(0), 1000);
    }
}
