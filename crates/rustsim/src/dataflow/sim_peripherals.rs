//! WASM-compatible simulated peripherals for browser simulation mode.

use std::collections::HashMap;

use embedded_hal::i2c::I2c;
use i2c_hil_sim::{Address, RuntimeBus};
use module_traits::{PeripheralError, SimPeripherals};

/// Parity mode for serial communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Parity {
    None,
    Odd,
    Even,
}

impl Parity {
    /// Convert from a `u8` wire value: 0 = None, 1 = Odd, 2 = Even.
    pub fn from_u8(v: u8) -> Result<Self, String> {
        match v {
            0 => Ok(Parity::None),
            1 => Ok(Parity::Odd),
            2 => Ok(Parity::Even),
            other => Err(format!(
                "invalid parity value: {other} (expected 0, 1, or 2)"
            )),
        }
    }
}

/// Configuration for a serial port (baud rate, framing).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: Parity,
    pub stop_bits: u8,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud: 115_200,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
        }
    }
}

/// Simulated peripheral state for browser-based simulation.
///
/// Holds virtual ADC voltages, PWM duties, GPIO pin states, encoder positions,
/// display text, and stepper positions. Users configure input values (e.g. ADC
/// voltages) via the WASM API, and blocks read/write through `SimPeripherals`.
/// Maximum I2C devices per simulated bus.
const I2C_MAX_DEVICES: usize = 8;

/// A virtual TCP socket with send/receive buffers.
struct VirtualTcpSocket {
    send_buf: Vec<u8>,
    recv_buf: Vec<u8>,
}

pub struct WasmSimPeripherals {
    adc_voltages: HashMap<u8, f64>,
    pwm_duties: HashMap<u8, f64>,
    gpio_states: HashMap<u8, bool>,
    uart_buffers: HashMap<u8, Vec<u8>>,
    encoder_positions: HashMap<u8, i64>,
    display_lines: HashMap<(u8, u8), (String, String)>,
    stepper_positions: HashMap<u8, i64>,
    stepper_targets: HashMap<u8, i64>,
    i2c_buses: HashMap<u8, RuntimeBus<I2C_MAX_DEVICES>>,
    tcp_sockets: HashMap<u8, VirtualTcpSocket>,
    udp_send_bufs: HashMap<u8, Vec<u8>>,
    udp_recv_bufs: HashMap<u8, Vec<u8>>,
    serial_configs: HashMap<u8, SerialConfig>,
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
            i2c_buses: HashMap::new(),
            tcp_sockets: HashMap::new(),
            udp_send_bufs: HashMap::new(),
            udp_recv_bufs: HashMap::new(),
            serial_configs: HashMap::new(),
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

    /// Add a simulated I2C device on the given bus.
    pub fn add_i2c_device(&mut self, bus: u8, addr: u8, name: &str) {
        let runtime_bus = self.i2c_buses.entry(bus).or_default();
        if let Some(address) = Address::new(addr) {
            let _ = runtime_bus.add_device(address, name.as_bytes(), &[0u8; 256]);
        }
    }

    /// Remove a simulated I2C device from the given bus.
    pub fn remove_i2c_device(&mut self, bus: u8, addr: u8) {
        if let Some(runtime_bus) = self.i2c_buses.get_mut(&bus) {
            if let Some(address) = Address::new(addr) {
                let _ = runtime_bus.remove_device(address);
            }
        }
    }

    /// Get the register map for an I2C device.
    pub fn i2c_device_registers(&self, bus: u8, addr: u8) -> Option<&[u8; 256]> {
        self.i2c_buses.get(&bus)?.device_registers(addr)
    }

    /// Inject data into a TCP socket's receive buffer (simulates remote peer sending).
    pub fn inject_tcp_data(&mut self, id: u8, data: &[u8]) {
        if let Some(sock) = self.tcp_sockets.get_mut(&id) {
            sock.recv_buf.extend_from_slice(data);
        }
    }

    /// Drain the TCP socket's send buffer (read what the block sent).
    pub fn drain_tcp_data(&mut self, id: u8) -> Vec<u8> {
        if let Some(sock) = self.tcp_sockets.get_mut(&id) {
            sock.send_buf.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Configure a serial port's baud rate and framing parameters.
    pub fn configure_serial(
        &mut self,
        port: u8,
        baud: u32,
        data_bits: u8,
        parity: Parity,
        stop_bits: u8,
    ) {
        self.serial_configs.insert(
            port,
            SerialConfig {
                baud,
                data_bits,
                parity,
                stop_bits,
            },
        );
    }

    /// Read back the serial configuration for a given port.
    pub fn get_serial_config(&self, port: u8) -> Option<&SerialConfig> {
        self.serial_configs.get(&port)
    }

    /// List all configured serial ports and their configurations.
    pub fn serial_ports(&self) -> Vec<(u8, SerialConfig)> {
        let mut ports: Vec<(u8, SerialConfig)> = self
            .serial_configs
            .iter()
            .map(|(&port, config)| (port, config.clone()))
            .collect();
        ports.sort_by_key(|(port, _)| *port);
        ports
    }

    /// Inject data into a UDP socket's receive buffer.
    pub fn inject_udp_data(&mut self, id: u8, data: &[u8]) {
        self.udp_recv_bufs
            .entry(id)
            .or_default()
            .extend_from_slice(data);
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

    fn tcp_connect(&mut self, id: u8, _addr: &str, _port: u16) -> Result<(), PeripheralError> {
        self.tcp_sockets.insert(
            id,
            VirtualTcpSocket {
                send_buf: Vec::new(),
                recv_buf: Vec::new(),
            },
        );
        Ok(())
    }

    fn tcp_send(&mut self, id: u8, data: &[u8]) -> Result<usize, PeripheralError> {
        let sock = self
            .tcp_sockets
            .get_mut(&id)
            .ok_or(PeripheralError::NotConnected)?;
        sock.send_buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn tcp_recv(&mut self, id: u8, buf: &mut [u8]) -> Result<usize, PeripheralError> {
        let sock = self
            .tcp_sockets
            .get_mut(&id)
            .ok_or(PeripheralError::NotConnected)?;
        let n = buf.len().min(sock.recv_buf.len());
        buf[..n].copy_from_slice(&sock.recv_buf[..n]);
        sock.recv_buf.drain(..n);
        Ok(n)
    }

    fn tcp_close(&mut self, id: u8) {
        self.tcp_sockets.remove(&id);
    }

    fn udp_send(
        &mut self,
        id: u8,
        _addr: &str,
        _port: u16,
        data: &[u8],
    ) -> Result<usize, PeripheralError> {
        self.udp_send_bufs
            .entry(id)
            .or_default()
            .extend_from_slice(data);
        Ok(data.len())
    }

    fn udp_recv(&mut self, id: u8, buf: &mut [u8]) -> Result<usize, PeripheralError> {
        let recv_buf = self
            .udp_recv_bufs
            .get_mut(&id)
            .ok_or(PeripheralError::NotConnected)?;
        let n = buf.len().min(recv_buf.len());
        buf[..n].copy_from_slice(&recv_buf[..n]);
        recv_buf.drain(..n);
        Ok(n)
    }

    fn i2c_write(&mut self, bus: u8, addr: u8, data: &[u8]) -> Result<(), PeripheralError> {
        let runtime_bus = self
            .i2c_buses
            .get_mut(&bus)
            .ok_or(PeripheralError::NotConnected)?;
        runtime_bus
            .write(addr, data)
            .map_err(|_| PeripheralError::Nack)
    }

    fn i2c_read(&mut self, bus: u8, addr: u8, buf: &mut [u8]) -> Result<(), PeripheralError> {
        let runtime_bus = self
            .i2c_buses
            .get_mut(&bus)
            .ok_or(PeripheralError::NotConnected)?;
        runtime_bus
            .read(addr, buf)
            .map_err(|_| PeripheralError::Nack)
    }

    fn i2c_write_read(
        &mut self,
        bus: u8,
        addr: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), PeripheralError> {
        let runtime_bus = self
            .i2c_buses
            .get_mut(&bus)
            .ok_or(PeripheralError::NotConnected)?;
        runtime_bus
            .write_read(addr, write, read)
            .map_err(|_| PeripheralError::Nack)
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

    #[test]
    fn wasm_sim_peripherals_default() {
        let mut p = WasmSimPeripherals::default();
        assert_eq!(p.adc_read(0), 0.0);
    }

    // --- I2C mock tests ---

    #[test]
    fn i2c_add_device_and_read_registers() {
        let mut p = WasmSimPeripherals::new();
        p.add_i2c_device(0, 0x48, "TMP1075");
        let regs = p.i2c_device_registers(0, 0x48);
        assert!(regs.is_some());
    }

    #[test]
    fn i2c_write_read_roundtrip() {
        let mut p = WasmSimPeripherals::new();
        p.add_i2c_device(0, 0x48, "test");
        // Write register 0x05 = [0xAB, 0xCD]
        assert!(p.i2c_write(0, 0x48, &[0x05, 0xAB, 0xCD]).is_ok());
        // Read back: set pointer to 0x05, then read 2 bytes
        let mut buf = [0u8; 2];
        assert!(p.i2c_write_read(0, 0x48, &[0x05], &mut buf).is_ok());
        assert_eq!(buf, [0xAB, 0xCD]);
    }

    #[test]
    fn i2c_multi_bus() {
        let mut p = WasmSimPeripherals::new();
        p.add_i2c_device(0, 0x48, "bus0-dev");
        p.add_i2c_device(1, 0x48, "bus1-dev");
        // Same address on different buses should work independently
        assert!(p.i2c_write(0, 0x48, &[0x00, 0x11]).is_ok());
        assert!(p.i2c_write(1, 0x48, &[0x00, 0x22]).is_ok());
        let mut buf = [0u8; 1];
        p.i2c_write_read(0, 0x48, &[0x00], &mut buf).unwrap();
        assert_eq!(buf[0], 0x11);
        p.i2c_write_read(1, 0x48, &[0x00], &mut buf).unwrap();
        assert_eq!(buf[0], 0x22);
    }

    #[test]
    fn i2c_remove_device() {
        let mut p = WasmSimPeripherals::new();
        p.add_i2c_device(0, 0x48, "test");
        assert!(p.i2c_device_registers(0, 0x48).is_some());
        p.remove_i2c_device(0, 0x48);
        assert!(p.i2c_device_registers(0, 0x48).is_none());
    }

    #[test]
    fn i2c_error_on_missing_device() {
        let mut p = WasmSimPeripherals::new();
        assert!(p.i2c_write(0, 0x48, &[0x00]).is_err());
        let mut buf = [0u8; 1];
        assert!(p.i2c_read(0, 0x48, &mut buf).is_err());
    }

    // --- Socket mock tests ---

    #[test]
    fn tcp_connect_send_recv_roundtrip() {
        let mut p = WasmSimPeripherals::new();
        assert!(p.tcp_connect(0, "127.0.0.1", 8080).is_ok());
        // Inject data into receive buffer (simulates remote sending to us)
        p.inject_tcp_data(0, b"hello");
        let mut buf = [0u8; 10];
        let n = p.tcp_recv(0, &mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn tcp_send_and_drain() {
        let mut p = WasmSimPeripherals::new();
        p.tcp_connect(0, "127.0.0.1", 8080).unwrap();
        let n = p.tcp_send(0, b"world").unwrap();
        assert_eq!(n, 5);
        let drained = p.drain_tcp_data(0);
        assert_eq!(drained, b"world");
    }

    #[test]
    fn tcp_close_clears_state() {
        let mut p = WasmSimPeripherals::new();
        p.tcp_connect(0, "127.0.0.1", 8080).unwrap();
        p.tcp_send(0, b"data").unwrap();
        p.tcp_close(0);
        // After close, operations fail
        assert!(p.tcp_send(0, b"more").is_err());
    }

    #[test]
    fn tcp_error_on_unconnected() {
        let mut p = WasmSimPeripherals::new();
        assert!(p.tcp_send(0, b"data").is_err());
        let mut buf = [0u8; 4];
        assert!(p.tcp_recv(0, &mut buf).is_err());
    }

    #[test]
    fn udp_send_recv_stateless() {
        let mut p = WasmSimPeripherals::new();
        // UDP doesn't require connect, but we use inject for recv simulation
        let n = p.udp_send(0, "127.0.0.1", 9000, b"packet").unwrap();
        assert_eq!(n, 6);
        // Inject incoming UDP data
        p.inject_udp_data(0, b"response");
        let mut buf = [0u8; 16];
        let n = p.udp_recv(0, &mut buf).unwrap();
        assert_eq!(n, 8);
        assert_eq!(&buf[..8], b"response");
    }

    // --- Serial config tests ---

    #[test]
    fn configure_serial_and_read_back() {
        let mut p = WasmSimPeripherals::new();
        p.configure_serial(0, 9600, 8, Parity::None, 1);
        let config = p.get_serial_config(0).unwrap();
        assert_eq!(config.baud, 9600);
        assert_eq!(config.data_bits, 8);
        assert_eq!(config.parity, Parity::None);
        assert_eq!(config.stop_bits, 1);
        // Unconfigured port returns None
        assert!(p.get_serial_config(5).is_none());
    }

    #[test]
    fn serial_ports_lists_configured() {
        let mut p = WasmSimPeripherals::new();
        p.configure_serial(2, 9600, 8, Parity::None, 1);
        p.configure_serial(0, 115_200, 8, Parity::Odd, 1);
        let ports = p.serial_ports();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].0, 0);
        assert_eq!(ports[0].1.baud, 115_200);
        assert_eq!(ports[1].0, 2);
    }

    #[test]
    fn serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud, 115_200);
        assert_eq!(config.data_bits, 8);
        assert_eq!(config.parity, Parity::None);
        assert_eq!(config.stop_bits, 1);
    }

    #[test]
    fn multiple_sockets_independent() {
        let mut p = WasmSimPeripherals::new();
        p.tcp_connect(0, "host-a", 80).unwrap();
        p.tcp_connect(1, "host-b", 443).unwrap();
        p.inject_tcp_data(0, b"aaa");
        p.inject_tcp_data(1, b"bbb");
        let mut buf = [0u8; 3];
        p.tcp_recv(0, &mut buf).unwrap();
        assert_eq!(&buf, b"aaa");
        p.tcp_recv(1, &mut buf).unwrap();
        assert_eq!(&buf, b"bbb");
    }
}
