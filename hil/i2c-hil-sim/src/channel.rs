//! Array-backed I2C transaction channel.
//!
//! [`I2cChannel`] is a fixed-size ring buffer that decouples I2C producers
//! (implementing [`embedded_hal::i2c::I2c`]) from consumers that execute
//! transactions against any bus.

/// A single I2C transaction request.
#[derive(Clone, Copy, Debug)]
pub struct I2cTransaction {
    pub addr: u8,
    pub write_len: u8,
    pub write_buf: [u8; 4],
    pub read_len: u8,
}

impl I2cTransaction {
    pub fn write_read(addr: u8, write: &[u8], read_len: u8) -> Self {
        let mut buf = [0u8; 4];
        let len = write.len().min(4);
        buf[..len].copy_from_slice(&write[..len]);
        Self {
            addr,
            write_len: len as u8,
            write_buf: buf,
            read_len,
        }
    }

    pub fn write(addr: u8, data: &[u8]) -> Self {
        Self::write_read(addr, data, 0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct I2cResponse {
    pub data: [u8; 4],
    pub len: u8,
    pub ok: bool,
}

impl Default for I2cResponse {
    fn default() -> Self {
        Self { data: [0; 4], len: 0, ok: false }
    }
}

impl I2cResponse {
    pub fn ok(data: &[u8]) -> Self {
        let mut buf = [0u8; 4];
        let len = data.len().min(4);
        buf[..len].copy_from_slice(&data[..len]);
        Self { data: buf, len: len as u8, ok: true }
    }

    pub fn err() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_write_read_roundtrip() {
        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        assert_eq!(tx.addr, 0x48);
        assert_eq!(tx.write_buf[0], 0x00);
        assert_eq!(tx.write_len, 1);
        assert_eq!(tx.read_len, 2);
    }

    #[test]
    fn response_default_is_not_ok() {
        let r = I2cResponse::default();
        assert!(!r.ok);
        assert_eq!(r.len, 0);
    }

    #[test]
    fn response_from_data() {
        let r = I2cResponse::ok(&[0xCA, 0xFE]);
        assert!(r.ok);
        assert_eq!(r.len, 2);
        assert_eq!(r.data[0], 0xCA);
        assert_eq!(r.data[1], 0xFE);
    }
}
