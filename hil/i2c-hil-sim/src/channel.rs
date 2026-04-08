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

/// Fixed-size ring buffer for I2C transactions.
///
/// Producer enqueues transactions via [`enqueue`](Self::enqueue).
/// Consumer drains via [`dequeue`](Self::dequeue) and posts results
/// via [`complete`](Self::complete). Producer retrieves results
/// via [`take_response`](Self::take_response).
pub struct I2cChannel<const N: usize> {
    requests: [Option<I2cTransaction>; N],
    responses: [Option<I2cResponse>; N],
    head: usize,
    tail: usize,
    count: usize,
}

impl<const N: usize> I2cChannel<N> {
    /// Creates an empty channel.
    pub const fn new() -> Self {
        Self {
            requests: [None; N],
            responses: [None; N],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Returns true if no transactions are pending.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Enqueues a transaction. Returns the slot index.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the channel is full.
    #[allow(clippy::result_unit_err)]
    pub fn enqueue(&mut self, tx: I2cTransaction) -> Result<usize, ()> {
        if self.count >= N {
            return Err(());
        }
        let idx = self.head;
        self.requests[idx] = Some(tx);
        self.responses[idx] = None;
        self.head = (self.head + 1) % N;
        self.count += 1;
        Ok(idx)
    }

    /// Dequeues the next pending transaction for the consumer.
    ///
    /// Returns `(slot_index, transaction)`. The consumer must call
    /// [`complete`](Self::complete) with the same slot index when done.
    pub fn dequeue(&mut self) -> Option<(usize, I2cTransaction)> {
        if self.count == 0 {
            return None;
        }
        let idx = self.tail;
        let tx = self.requests[idx].take()?;
        self.tail = (self.tail + 1) % N;
        self.count -= 1;
        Some((idx, tx))
    }

    /// Posts a response for a completed transaction.
    pub fn complete(&mut self, idx: usize, response: I2cResponse) {
        if idx < N {
            self.responses[idx] = Some(response);
        }
    }

    /// Takes the response for a completed transaction.
    ///
    /// Returns `None` if no response is ready yet.
    pub fn take_response(&mut self, idx: usize) -> Option<I2cResponse> {
        if idx < N {
            self.responses[idx].take()
        } else {
            None
        }
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

    #[test]
    fn channel_enqueue_dequeue() {
        let mut ch = I2cChannel::<4>::new();
        assert!(ch.is_empty());

        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        let idx = ch.enqueue(tx).unwrap();

        assert!(!ch.is_empty());
        let (di, dtx) = ch.dequeue().unwrap();
        assert_eq!(di, idx);
        assert_eq!(dtx.addr, 0x48);
    }

    #[test]
    fn channel_complete_returns_response() {
        let mut ch = I2cChannel::<4>::new();
        let tx = I2cTransaction::write_read(0x48, &[0x00], 2);
        let idx = ch.enqueue(tx).unwrap();

        ch.complete(idx, I2cResponse::ok(&[0xCA, 0xFE]));

        let resp = ch.take_response(idx).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.data[0], 0xCA);
        assert_eq!(resp.data[1], 0xFE);
    }

    #[test]
    fn channel_full_returns_err() {
        let mut ch = I2cChannel::<2>::new();
        ch.enqueue(I2cTransaction::write(0x10, &[0x00])).unwrap();
        ch.enqueue(I2cTransaction::write(0x20, &[0x00])).unwrap();
        assert!(ch.enqueue(I2cTransaction::write(0x30, &[0x00])).is_err());
    }

    #[test]
    fn channel_empty_dequeue_returns_none() {
        let mut ch = I2cChannel::<4>::new();
        assert!(ch.dequeue().is_none());
    }

    #[test]
    fn channel_wraps_around() {
        let mut ch = I2cChannel::<2>::new();
        for round in 0..2u8 {
            let idx = ch.enqueue(I2cTransaction::write(0x10 + round, &[0x00])).unwrap();
            let (di, _) = ch.dequeue().unwrap();
            assert_eq!(di, idx);
            ch.complete(idx, I2cResponse::ok(&[0xAA]));
            ch.take_response(idx);
        }
    }
}
