//! Firmware update protocol over WebSocket (CBOR tags 20-23).
//!
//! Implements the A/B OTA firmware update protocol. The firmware image is
//! streamed in chunks over WebSocket with CRC32 integrity verification.
//!
//! # Protocol Flow
//!
//! 1. Client sends `FwBegin` (tag 20) with total size and expected CRC32.
//! 2. Server responds `FwReady` (tag 20) with max chunk size.
//! 3. Client sends `FwChunk` (tag 21) messages with offset and data.
//! 4. Server responds `FwChunkAck` (tag 21) with next expected offset.
//! 5. Client sends `FwFinish` (tag 22) with final CRC32.
//! 6. Server verifies CRC32 and responds `FwFinishAck` (tag 22).
//! 7. Client sends `FwMarkBooted` (tag 23) to confirm after reboot.
//!
//! # Trait Abstraction
//!
//! The [`DfuFlashWriter`] trait abstracts DFU flash operations so the
//! protocol logic is testable on the host with a mock implementation.

/// Maximum chunk size in bytes (fits in a 1500-byte WebSocket frame).
pub const MAX_CHUNK_SIZE: u16 = 1024;

/// Error type for DFU flash operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DfuError {
    /// Flash erase operation failed.
    EraseFailed,
    /// Flash write operation failed.
    WriteFailed,
    /// Flash read operation failed.
    ReadFailed,
    /// Mark-updated or mark-booted operation failed.
    MarkFailed,
}

/// Error type for CBOR encoding operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// Output buffer is too small for the encoded data.
    BufferTooSmall,
}

/// Error type for firmware update request handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FwRequestError {
    /// CBOR decoding failed or message format is invalid.
    MalformedRequest,
    /// Request is invalid for the current firmware update state.
    InvalidState,
    /// A DFU flash operation failed.
    Dfu(DfuError),
    /// CBOR encoding of the response failed.
    Encode(EncodeError),
}

impl From<DfuError> for FwRequestError {
    fn from(e: DfuError) -> Self {
        FwRequestError::Dfu(e)
    }
}

impl From<EncodeError> for FwRequestError {
    fn from(e: EncodeError) -> Self {
        FwRequestError::Encode(e)
    }
}

/// Trait abstracting DFU flash write operations.
///
/// Board-specific implementations wrap embassy-boot-rp's
/// `BlockingFirmwareUpdater`. The mock implementation records
/// operations for host testing.
pub trait DfuFlashWriter {
    /// Erases the DFU partition to prepare for a new firmware image.
    ///
    /// # Errors
    ///
    /// Returns [`DfuError::EraseFailed`] if the erase operation fails.
    fn erase_dfu(&mut self) -> Result<(), DfuError>;

    /// Writes a chunk of firmware data at the given offset in the DFU partition.
    ///
    /// # Errors
    ///
    /// Returns [`DfuError::WriteFailed`] if the write operation fails.
    fn write_dfu(&mut self, offset: u32, data: &[u8]) -> Result<(), DfuError>;

    /// Reads firmware data from the DFU partition at the given offset.
    ///
    /// # Errors
    ///
    /// Returns [`DfuError::ReadFailed`] if the read operation fails.
    fn read_dfu(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), DfuError>;

    /// Marks the DFU image as ready to be swapped on next boot.
    ///
    /// # Errors
    ///
    /// Returns [`DfuError::MarkFailed`] if the mark operation fails.
    fn mark_updated(&mut self) -> Result<(), DfuError>;

    /// Marks the current running firmware as successfully booted.
    ///
    /// Prevents the bootloader from rolling back to the previous image.
    ///
    /// # Errors
    ///
    /// Returns [`DfuError::MarkFailed`] if the mark operation fails.
    fn mark_booted(&mut self) -> Result<(), DfuError>;

    /// Performs a system reset to boot into the new firmware.
    fn system_reset(&mut self) -> !;
}

/// State machine for an in-progress firmware update.
#[derive(Debug, Clone)]
pub enum FwUpdateState {
    /// No firmware update in progress.
    Idle,
    /// Receiving firmware chunks.
    Receiving {
        /// Total expected firmware size in bytes.
        total_size: u32,
        /// Expected CRC32 of the complete firmware image.
        expected_crc: u32,
        /// Number of bytes received so far.
        received: u32,
        /// Running CRC32 hasher state.
        crc_state: u32,
    },
    /// All chunks received, CRC32 verified.
    Complete,
}

/// Peeks at the CBOR tag (key 0) of a request without consuming it.
///
/// Returns `Some(tag)` if the request starts with a valid CBOR map
/// containing key 0, or `None` if the format is unexpected.
pub fn peek_tag(request: &[u8]) -> Option<u32> {
    let mut dec = minicbor::Decoder::new(request);
    let _map_len = dec.map().ok()?;
    let _key0 = dec.u32().ok()?;
    dec.u32().ok()
}

/// Checks whether a CBOR tag is a firmware update message (20-23).
pub fn is_fw_tag(tag: u32) -> bool {
    (20..=23).contains(&tag)
}

/// Handles a firmware update request and writes the response.
///
/// Dispatches based on the CBOR tag (20-23) and transitions the
/// [`FwUpdateState`] accordingly. The CBOR response is written into
/// `resp_buf` and the new state is returned.
///
/// # Errors
///
/// Returns a [`FwRequestError`] if CBOR decoding fails, the state machine
/// rejects the request, a DFU operation fails, or the response buffer is
/// too small.
pub fn handle_fw_request(
    state: FwUpdateState,
    writer: &mut impl DfuFlashWriter,
    request: &[u8],
    resp_buf: &mut [u8],
) -> Result<(FwUpdateState, usize), FwRequestError> {
    let mut dec = minicbor::Decoder::new(request);
    let _map_len = dec.map().map_err(|_| FwRequestError::MalformedRequest)?;
    let _key0 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let tag = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;

    match tag {
        // FwBegin: {0:20, 1:total_size(u32), 2:crc32(u32)}
        20 => handle_fw_begin(state, writer, &mut dec, resp_buf),
        // FwChunk: {0:21, 1:offset(u32), 2:data(bstr)}
        21 => handle_fw_chunk(state, writer, &mut dec, resp_buf),
        // FwFinish: {0:22, 1:crc32(u32)}
        22 => handle_fw_finish(state, writer, &mut dec, resp_buf),
        // FwMarkBooted: {0:23}
        23 => handle_fw_mark_booted(state, writer, resp_buf),
        _ => Err(FwRequestError::MalformedRequest),
    }
}

/// Handles FwBegin: erases DFU partition and prepares for receiving chunks.
fn handle_fw_begin(
    state: FwUpdateState,
    writer: &mut impl DfuFlashWriter,
    dec: &mut minicbor::Decoder<'_>,
    resp_buf: &mut [u8],
) -> Result<(FwUpdateState, usize), FwRequestError> {
    // Only accept FwBegin from Idle state
    if !matches!(state, FwUpdateState::Idle) {
        return Err(FwRequestError::InvalidState);
    }

    let _k1 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let total_size = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let _k2 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let expected_crc = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;

    writer.erase_dfu()?;

    let new_state = FwUpdateState::Receiving {
        total_size,
        expected_crc,
        received: 0,
        crc_state: 0,
    };

    // Encode FwReady: {0:20, 1:max_chunk(u16)}
    let n = encode_fw_ready(resp_buf)?;
    Ok((new_state, n))
}

/// Handles FwChunk: writes data to DFU partition and updates CRC.
fn handle_fw_chunk(
    state: FwUpdateState,
    writer: &mut impl DfuFlashWriter,
    dec: &mut minicbor::Decoder<'_>,
    resp_buf: &mut [u8],
) -> Result<(FwUpdateState, usize), FwRequestError> {
    let (total_size, expected_crc, received, crc_state) = match state {
        FwUpdateState::Receiving {
            total_size,
            expected_crc,
            received,
            crc_state,
        } => (total_size, expected_crc, received, crc_state),
        _ => return Err(FwRequestError::InvalidState),
    };

    let _k1 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let offset = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let _k2 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let data = dec.bytes().map_err(|_| FwRequestError::MalformedRequest)?;

    // Verify offset matches expected
    if offset != received {
        return Err(FwRequestError::InvalidState);
    }

    // Verify we won't exceed total size
    let new_received = received + data.len() as u32;
    if new_received > total_size {
        return Err(FwRequestError::InvalidState);
    }

    writer.write_dfu(offset, data)?;

    // Update CRC32
    let new_crc_state = crc32_update(crc_state, data);

    let new_state = FwUpdateState::Receiving {
        total_size,
        expected_crc,
        received: new_received,
        crc_state: new_crc_state,
    };

    // Encode FwChunkAck: {0:21, 1:next_offset(u32)}
    let n = encode_fw_chunk_ack(resp_buf, new_received)?;
    Ok((new_state, n))
}

/// Handles FwFinish: verifies CRC32 and marks firmware as updated.
fn handle_fw_finish(
    state: FwUpdateState,
    writer: &mut impl DfuFlashWriter,
    dec: &mut minicbor::Decoder<'_>,
    resp_buf: &mut [u8],
) -> Result<(FwUpdateState, usize), FwRequestError> {
    let (total_size, expected_crc, received, crc_state) = match state {
        FwUpdateState::Receiving {
            total_size,
            expected_crc,
            received,
            crc_state,
        } => (total_size, expected_crc, received, crc_state),
        _ => return Err(FwRequestError::InvalidState),
    };

    let _k1 = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;
    let final_crc = dec.u32().map_err(|_| FwRequestError::MalformedRequest)?;

    // Verify all bytes received
    if received != total_size {
        return Err(FwRequestError::InvalidState);
    }

    // Finalize CRC and verify
    let computed_crc = crc32_finalize(crc_state);
    if computed_crc != expected_crc || computed_crc != final_crc {
        return Err(FwRequestError::InvalidState);
    }

    writer.mark_updated()?;

    // Encode FwFinishAck: {0:22}
    let n = encode_fw_finish_ack(resp_buf)?;
    Ok((FwUpdateState::Complete, n))
}

/// Handles FwMarkBooted: marks the current firmware as booted.
fn handle_fw_mark_booted(
    state: FwUpdateState,
    writer: &mut impl DfuFlashWriter,
    resp_buf: &mut [u8],
) -> Result<(FwUpdateState, usize), FwRequestError> {
    writer.mark_booted()?;
    let n = encode_fw_mark_booted_ack(resp_buf)?;
    Ok((state, n))
}

// --- CRC32 (IEEE 802.3 / ITU-T V.42) ---

/// CRC32 lookup table (IEEE 802.3 polynomial 0xEDB88320).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Updates a running CRC32 state with additional data bytes.
///
/// The `state` should be initialized to `0` for the first call.
/// Call [`crc32_finalize`] after all data has been processed.
pub fn crc32_update(state: u32, data: &[u8]) -> u32 {
    let mut crc = state ^ 0xFFFF_FFFF;
    let mut i = 0;
    while i < data.len() {
        let idx = ((crc ^ data[i] as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
        i += 1;
    }
    crc ^ 0xFFFF_FFFF
}

/// Finalizes a CRC32 computation.
///
/// Since `crc32_update` already applies the XOR mask, this function
/// re-applies the mask to get the intermediate state, then finalizes.
/// For simplicity with the incremental API, the state stored between
/// calls is the *finalized* value, so this is an identity function.
pub fn crc32_finalize(state: u32) -> u32 {
    state
}

/// Computes CRC32 of a complete byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_finalize(crc32_update(0, data))
}

// --- CBOR encoders for firmware update responses ---

/// Encodes FwReady: `{0:20, 1:max_chunk(u16)}`.
fn encode_fw_ready(buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(20).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u16(MAX_CHUNK_SIZE)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwChunkAck: `{0:21, 1:next_offset(u32)}`.
fn encode_fw_chunk_ack(buf: &mut [u8], next_offset: u32) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(21).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(next_offset)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwFinishAck: `{0:22}`.
fn encode_fw_finish_ack(buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(22).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwMarkBootedAck: `{0:23}`.
fn encode_fw_mark_booted_ack(buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(23).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

// --- CBOR encoders for firmware update requests (used by tests and frontend) ---

/// Encodes FwBegin request: `{0:20, 1:total_size(u32), 2:crc32(u32)}`.
pub fn encode_fw_begin(buf: &mut [u8], total_size: u32, crc: u32) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(3).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(20).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(total_size)
        .map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(crc).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwChunk request: `{0:21, 1:offset(u32), 2:data(bstr)}`.
pub fn encode_fw_chunk(buf: &mut [u8], offset: u32, data: &[u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(3).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(21).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(offset).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.bytes(data).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwFinish request: `{0:22, 1:crc32(u32)}`.
pub fn encode_fw_finish(buf: &mut [u8], crc: u32) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(2).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(22).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(crc).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Encodes FwMarkBooted request: `{0:23}`.
pub fn encode_fw_mark_booted(buf: &mut [u8]) -> Result<usize, EncodeError> {
    let buf_len = buf.len();
    let mut writer: &mut [u8] = buf;
    let mut enc = minicbor::Encoder::new(&mut writer);

    enc.map(1).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(0).map_err(|_| EncodeError::BufferTooSmall)?;
    enc.u32(23).map_err(|_| EncodeError::BufferTooSmall)?;

    drop(enc);
    let remaining = writer.len();
    Ok(buf_len - remaining)
}

/// Stub DFU flash writer that rejects all firmware operations.
///
/// Boards without OTA support use this to satisfy the [`DfuFlashWriter`]
/// trait bound in `ws_server::run()`. Since all write operations return
/// errors, the firmware update protocol never reaches `system_reset`.
pub struct StubDfuWriter;

impl Default for StubDfuWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl StubDfuWriter {
    /// Creates a new stub DFU writer.
    pub fn new() -> Self {
        Self
    }
}

impl DfuFlashWriter for StubDfuWriter {
    fn erase_dfu(&mut self) -> Result<(), DfuError> {
        Err(DfuError::EraseFailed)
    }

    fn write_dfu(&mut self, _offset: u32, _data: &[u8]) -> Result<(), DfuError> {
        Err(DfuError::WriteFailed)
    }

    fn read_dfu(&mut self, _offset: u32, _buf: &mut [u8]) -> Result<(), DfuError> {
        Err(DfuError::ReadFailed)
    }

    fn mark_updated(&mut self) -> Result<(), DfuError> {
        Err(DfuError::MarkFailed)
    }

    fn mark_booted(&mut self) -> Result<(), DfuError> {
        Ok(())
    }

    #[cfg(not(tarpaulin_include))]
    fn system_reset(&mut self) -> ! {
        // Unreachable: mark_updated() returns Err, so the protocol
        // never transitions to Complete and never calls system_reset.
        loop {
            core::hint::spin_loop();
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;

    /// Maximum size of a mock DFU partition (64 KiB for testing).
    const MOCK_DFU_SIZE: usize = 64 * 1024;

    /// Mock DFU flash writer that records operations for verification.
    struct MockDfuWriter {
        data: [u8; MOCK_DFU_SIZE],
        erased: bool,
        marked_updated: bool,
        marked_booted: bool,
        written_ranges: [(u32, u32); 64],
        range_count: usize,
    }

    impl MockDfuWriter {
        fn new() -> Self {
            Self {
                data: [0xFF; MOCK_DFU_SIZE],
                erased: false,
                marked_updated: false,
                marked_booted: false,
                written_ranges: [(0, 0); 64],
                range_count: 0,
            }
        }
    }

    impl DfuFlashWriter for MockDfuWriter {
        fn erase_dfu(&mut self) -> Result<(), DfuError> {
            self.data = [0xFF; MOCK_DFU_SIZE];
            self.erased = true;
            Ok(())
        }

        fn write_dfu(&mut self, offset: u32, data: &[u8]) -> Result<(), DfuError> {
            let start = offset as usize;
            let end = start + data.len();
            if end > MOCK_DFU_SIZE {
                return Err(DfuError::WriteFailed);
            }
            self.data[start..end].copy_from_slice(data);
            if self.range_count < self.written_ranges.len() {
                self.written_ranges[self.range_count] = (offset, data.len() as u32);
                self.range_count += 1;
            }
            Ok(())
        }

        fn read_dfu(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), DfuError> {
            let start = offset as usize;
            let end = start + buf.len();
            if end > MOCK_DFU_SIZE {
                return Err(DfuError::ReadFailed);
            }
            buf.copy_from_slice(&self.data[start..end]);
            Ok(())
        }

        fn mark_updated(&mut self) -> Result<(), DfuError> {
            self.marked_updated = true;
            Ok(())
        }

        fn mark_booted(&mut self) -> Result<(), DfuError> {
            self.marked_booted = true;
            Ok(())
        }

        fn system_reset(&mut self) -> ! {
            panic!("system_reset called in test");
        }
    }

    fn decode_tag(buf: &[u8]) -> u32 {
        let mut dec = minicbor::Decoder::new(buf);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        dec.u32().unwrap()
    }

    fn decode_u32_field(buf: &[u8], key_idx: u32) -> u32 {
        let mut dec = minicbor::Decoder::new(buf);
        let _map = dec.map().unwrap();
        // Skip through keys until we find our key
        let mut found = 0u32;
        loop {
            let k = dec.u32().unwrap();
            let v = dec.u32().unwrap();
            if k == key_idx {
                return v;
            }
            found += 1;
            if found > 10 {
                panic!("key {key_idx} not found");
            }
        }
    }

    #[test]
    fn test_peek_tag() {
        let mut buf = [0u8; 64];
        let n = encode_fw_begin(&mut buf, 1024, 0x12345678).unwrap();
        assert_eq!(peek_tag(&buf[..n]), Some(20));

        let n = encode_fw_chunk(&mut buf, 0, &[1, 2, 3]).unwrap();
        assert_eq!(peek_tag(&buf[..n]), Some(21));

        let n = encode_fw_finish(&mut buf, 0xDEADBEEF).unwrap();
        assert_eq!(peek_tag(&buf[..n]), Some(22));

        let n = encode_fw_mark_booted(&mut buf).unwrap();
        assert_eq!(peek_tag(&buf[..n]), Some(23));
    }

    #[test]
    fn test_is_fw_tag() {
        assert!(is_fw_tag(20));
        assert!(is_fw_tag(21));
        assert!(is_fw_tag(22));
        assert!(is_fw_tag(23));
        assert!(!is_fw_tag(1));
        assert!(!is_fw_tag(19));
        assert!(!is_fw_tag(24));
    }

    #[test]
    fn test_fw_begin_from_idle() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_begin(&mut req_buf, 2048, 0xAABBCCDD).unwrap();

        let mut resp_buf = [0u8; 64];
        let (state, n) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        assert!(writer.erased);
        assert_eq!(decode_tag(&resp_buf[..n]), 20);

        match state {
            FwUpdateState::Receiving {
                total_size,
                expected_crc,
                received,
                ..
            } => {
                assert_eq!(total_size, 2048);
                assert_eq!(expected_crc, 0xAABBCCDD);
                assert_eq!(received, 0);
            }
            _ => panic!("expected Receiving state"),
        }
    }

    #[test]
    fn test_fw_begin_rejected_when_receiving() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_begin(&mut req_buf, 2048, 0).unwrap();

        let state = FwUpdateState::Receiving {
            total_size: 1024,
            expected_crc: 0,
            received: 512,
            crc_state: 0,
        };

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_chunk_write_and_ack() {
        let mut writer = MockDfuWriter::new();
        let data = [0xAA; 256];
        let mut req_buf = [0u8; 512];
        let req_len = encode_fw_chunk(&mut req_buf, 0, &data).unwrap();

        let state = FwUpdateState::Receiving {
            total_size: 1024,
            expected_crc: 0,
            received: 0,
            crc_state: 0,
        };

        let mut resp_buf = [0u8; 64];
        let (new_state, n) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();

        assert_eq!(decode_tag(&resp_buf[..n]), 21);
        assert_eq!(decode_u32_field(&resp_buf[..n], 1), 256);
        assert_eq!(&writer.data[..256], &data);

        match new_state {
            FwUpdateState::Receiving { received, .. } => assert_eq!(received, 256),
            _ => panic!("expected Receiving"),
        }
    }

    #[test]
    fn test_fw_chunk_wrong_offset_rejected() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 512];
        let req_len = encode_fw_chunk(&mut req_buf, 100, &[0xBB; 10]).unwrap();

        let state = FwUpdateState::Receiving {
            total_size: 1024,
            expected_crc: 0,
            received: 0,
            crc_state: 0,
        };

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_chunk_exceeds_total_size_rejected() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 512];
        let req_len = encode_fw_chunk(&mut req_buf, 0, &[0xCC; 200]).unwrap();

        let state = FwUpdateState::Receiving {
            total_size: 100,
            expected_crc: 0,
            received: 0,
            crc_state: 0,
        };

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_full_update_flow() {
        let mut writer = MockDfuWriter::new();
        let firmware = [0x42u8; 256];
        let expected_crc = crc32(&firmware);

        // Step 1: FwBegin
        let mut req_buf = [0u8; 512];
        let mut resp_buf = [0u8; 64];
        let req_len = encode_fw_begin(&mut req_buf, 256, expected_crc).unwrap();
        let (state, n) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();
        assert_eq!(decode_tag(&resp_buf[..n]), 20);

        // Step 2: FwChunk (all at once)
        let req_len = encode_fw_chunk(&mut req_buf, 0, &firmware).unwrap();
        let (state, n) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();
        assert_eq!(decode_tag(&resp_buf[..n]), 21);
        assert_eq!(decode_u32_field(&resp_buf[..n], 1), 256);

        // Step 3: FwFinish
        let req_len = encode_fw_finish(&mut req_buf, expected_crc).unwrap();
        let (state, n) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();
        assert_eq!(decode_tag(&resp_buf[..n]), 22);
        assert!(matches!(state, FwUpdateState::Complete));
        assert!(writer.marked_updated);

        // Verify written data
        assert_eq!(&writer.data[..256], &firmware);
    }

    #[test]
    fn test_full_update_flow_multi_chunk() {
        let mut writer = MockDfuWriter::new();
        let mut firmware = [0u8; 512];
        let mut i = 0;
        while i < firmware.len() {
            firmware[i] = (i & 0xFF) as u8;
            i += 1;
        }
        let expected_crc = crc32(&firmware);

        let mut req_buf = [0u8; 1200];
        let mut resp_buf = [0u8; 64];

        // FwBegin
        let req_len = encode_fw_begin(&mut req_buf, 512, expected_crc).unwrap();
        let (state, _) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        // FwChunk 0..256
        let req_len = encode_fw_chunk(&mut req_buf, 0, &firmware[..256]).unwrap();
        let (state, n) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();
        assert_eq!(decode_u32_field(&resp_buf[..n], 1), 256);

        // FwChunk 256..512
        let req_len = encode_fw_chunk(&mut req_buf, 256, &firmware[256..]).unwrap();
        let (state, n) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();
        assert_eq!(decode_u32_field(&resp_buf[..n], 1), 512);

        // FwFinish
        let req_len = encode_fw_finish(&mut req_buf, expected_crc).unwrap();
        let (state, _) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();
        assert!(matches!(state, FwUpdateState::Complete));
        assert_eq!(&writer.data[..512], &firmware);
    }

    #[test]
    fn test_fw_finish_wrong_crc_rejected() {
        let mut writer = MockDfuWriter::new();
        let firmware = [0x42u8; 64];
        let expected_crc = crc32(&firmware);

        let mut req_buf = [0u8; 512];
        let mut resp_buf = [0u8; 64];

        // FwBegin
        let req_len = encode_fw_begin(&mut req_buf, 64, expected_crc).unwrap();
        let (state, _) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        // FwChunk
        let req_len = encode_fw_chunk(&mut req_buf, 0, &firmware).unwrap();
        let (state, _) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();

        // FwFinish with wrong CRC
        let req_len = encode_fw_finish(&mut req_buf, 0xBADBAD).unwrap();
        let result = handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_finish_incomplete_transfer_rejected() {
        let mut writer = MockDfuWriter::new();

        let mut req_buf = [0u8; 512];
        let mut resp_buf = [0u8; 64];

        // FwBegin expecting 256 bytes
        let req_len = encode_fw_begin(&mut req_buf, 256, 0).unwrap();
        let (state, _) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        // Only send 64 bytes
        let req_len = encode_fw_chunk(&mut req_buf, 0, &[0xAA; 64]).unwrap();
        let (state, _) =
            handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf).unwrap();

        // Try to finish early
        let req_len = encode_fw_finish(&mut req_buf, 0).unwrap();
        let result = handle_fw_request(state, &mut writer, &req_buf[..req_len], &mut resp_buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_mark_booted() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_mark_booted(&mut req_buf).unwrap();

        let mut resp_buf = [0u8; 64];
        let (_, n) = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        assert!(writer.marked_booted);
        assert_eq!(decode_tag(&resp_buf[..n]), 23);
    }

    #[test]
    fn test_stub_dfu_writer() {
        let mut stub = StubDfuWriter::new();
        assert!(stub.erase_dfu().is_err());
        assert!(stub.write_dfu(0, &[1, 2, 3]).is_err());
        let mut buf = [0u8; 4];
        assert!(stub.read_dfu(0, &mut buf).is_err());
        assert!(stub.mark_updated().is_err());
        // mark_booted succeeds on StubDfuWriter
        assert!(stub.mark_booted().is_ok());
    }

    #[test]
    fn test_stub_dfu_writer_default() {
        let stub = StubDfuWriter;
        // default and new produce the same thing
        let _stub2 = stub;
    }

    #[test]
    fn test_handle_fw_request_invalid_tag() {
        let mut writer = MockDfuWriter::new();
        // Craft a CBOR message with an unsupported tag (99)
        let mut req_buf = [0u8; 64];
        let buf_len = req_buf.len();
        let mut w: &mut [u8] = &mut req_buf;
        let mut enc = minicbor::Encoder::new(&mut w);
        enc.map(1).unwrap();
        enc.u32(0).unwrap();
        enc.u32(99).unwrap();
        drop(enc);
        let remaining = w.len();
        let req_len = buf_len - remaining;

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_chunk_not_from_idle() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 512];
        let req_len = encode_fw_chunk(&mut req_buf, 0, &[0xAA; 10]).unwrap();

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_finish_not_from_idle() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_finish(&mut req_buf, 0).unwrap();

        let mut resp_buf = [0u8; 64];
        let result = handle_fw_request(
            FwUpdateState::Idle,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_fw_mark_booted_from_complete() {
        let mut writer = MockDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_mark_booted(&mut req_buf).unwrap();

        let mut resp_buf = [0u8; 64];
        let (state, n) = handle_fw_request(
            FwUpdateState::Complete,
            &mut writer,
            &req_buf[..req_len],
            &mut resp_buf,
        )
        .unwrap();

        assert!(writer.marked_booted);
        assert_eq!(decode_tag(&resp_buf[..n]), 23);
        assert!(matches!(state, FwUpdateState::Complete));
    }

    #[test]
    fn test_crc32_known_value() {
        // CRC32 of "123456789" is 0xCBF43926
        let data = b"123456789";
        assert_eq!(crc32(data), 0xCBF4_3926);
    }

    #[test]
    fn test_crc32_empty() {
        assert_eq!(crc32(&[]), 0x0000_0000);
    }

    #[test]
    fn test_crc32_incremental() {
        let data = b"Hello, World!";
        let full = crc32(data);

        // Same result computed incrementally
        let state = crc32_update(0, &data[..5]);
        let state = crc32_update(state, &data[5..]);
        assert_eq!(crc32_finalize(state), full);
    }

    #[test]
    fn test_encode_decode_fw_ready() {
        let mut buf = [0u8; 64];
        let n = encode_fw_ready(&mut buf).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 20);

        // Verify max_chunk field
        let mut dec = minicbor::Decoder::new(&buf[..n]);
        let _map = dec.map().unwrap();
        let _k0 = dec.u32().unwrap();
        let _tag = dec.u32().unwrap();
        let _k1 = dec.u32().unwrap();
        let max_chunk = dec.u16().unwrap();
        assert_eq!(max_chunk, MAX_CHUNK_SIZE);
    }

    #[test]
    fn test_encode_decode_fw_chunk_ack() {
        let mut buf = [0u8; 64];
        let n = encode_fw_chunk_ack(&mut buf, 4096).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 21);
        assert_eq!(decode_u32_field(&buf[..n], 1), 4096);
    }

    #[test]
    fn test_encode_decode_fw_finish_ack() {
        let mut buf = [0u8; 64];
        let n = encode_fw_finish_ack(&mut buf).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 22);
    }

    #[test]
    fn test_encode_decode_fw_mark_booted_ack() {
        let mut buf = [0u8; 64];
        let n = encode_fw_mark_booted_ack(&mut buf).unwrap();
        assert_eq!(decode_tag(&buf[..n]), 23);
    }

    #[test]
    fn test_fw_update_state_clone() {
        let receiving = FwUpdateState::Receiving {
            total_size: 1024,
            expected_crc: 0xDEAD,
            received: 512,
            crc_state: 0xBEEF,
        };
        let cloned = receiving.clone();
        match cloned {
            FwUpdateState::Receiving {
                total_size,
                expected_crc,
                received,
                crc_state,
            } => {
                assert_eq!(total_size, 1024);
                assert_eq!(expected_crc, 0xDEAD);
                assert_eq!(received, 512);
                assert_eq!(crc_state, 0xBEEF);
            }
            _ => panic!("expected Receiving state after clone"),
        }

        let idle = FwUpdateState::Idle;
        let idle_clone = idle.clone();
        assert!(matches!(idle_clone, FwUpdateState::Idle));

        let complete = FwUpdateState::Complete;
        let complete_clone = complete.clone();
        assert!(matches!(complete_clone, FwUpdateState::Complete));
    }

    #[test]
    fn test_mock_dfu_read_dfu() {
        let mut writer = MockDfuWriter::new();
        // Write some data first
        writer.write_dfu(0, &[0xAA, 0xBB, 0xCC, 0xDD]).unwrap();

        // Read it back
        let mut buf = [0u8; 4];
        writer.read_dfu(0, &mut buf).unwrap();
        assert_eq!(buf, [0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn test_mock_dfu_read_dfu_out_of_bounds() {
        let mut writer = MockDfuWriter::new();
        let mut buf = [0u8; 4];
        // Reading past the end should fail
        assert!(writer.read_dfu(MOCK_DFU_SIZE as u32, &mut buf).is_err());
    }

    #[test]
    fn test_stub_dfu_writer_fw_begin_fails() {
        let mut stub = StubDfuWriter::new();
        let mut req_buf = [0u8; 64];
        let req_len = encode_fw_begin(&mut req_buf, 1024, 0x12345678).unwrap();

        let mut resp_buf = [0u8; 64];
        // Should fail because StubDfuWriter::erase_dfu returns Err
        let result = handle_fw_request(
            FwUpdateState::Idle,
            &mut stub,
            &req_buf[..req_len],
            &mut resp_buf,
        );
        assert!(result.is_err());
    }
}
