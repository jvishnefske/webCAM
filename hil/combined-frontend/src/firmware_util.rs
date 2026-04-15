//! Pure firmware utility functions for chunking and CRC computation.
//!
//! These are separated from the UI component so they can be tested on
//! the host target (non-wasm32) without pulling in `web-sys` or Leptos.

/// Default chunk size used when the device does not report `max_chunk`.
pub const DEFAULT_CHUNK_SIZE: usize = 512;

/// Split firmware data into chunks of the given size.
///
/// Returns a list of `(offset, chunk_data)` pairs covering the entire input.
/// The last chunk may be smaller than `chunk_size`.
///
/// Returns an empty vec for empty input or zero chunk size.
pub fn chunk_firmware(data: &[u8], chunk_size: usize) -> Vec<(u32, Vec<u8>)> {
    if chunk_size == 0 {
        return Vec::new();
    }
    data.chunks(chunk_size)
        .enumerate()
        .map(|(i, chunk)| ((i * chunk_size) as u32, chunk.to_vec()))
        .collect()
}

/// Compute CRC32 of firmware data.
pub fn firmware_crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_firmware_exact() {
        let data = vec![0u8; 1024];
        let chunks = chunk_firmware(&data, 512);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1.len(), 512);
        assert_eq!(chunks[1].0, 512);
        assert_eq!(chunks[1].1.len(), 512);
    }

    #[test]
    fn test_chunk_firmware_remainder() {
        let data = vec![0u8; 1000];
        let chunks = chunk_firmware(&data, 512);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1.len(), 512);
        assert_eq!(chunks[1].0, 512);
        assert_eq!(chunks[1].1.len(), 488);
    }

    #[test]
    fn test_chunk_firmware_empty() {
        let chunks = chunk_firmware(&[], 512);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_firmware_zero_chunk_size() {
        let data = vec![0u8; 100];
        let chunks = chunk_firmware(&data, 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_firmware_single_chunk() {
        let data = vec![0xAB; 100];
        let chunks = chunk_firmware(&data, 512);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1.len(), 100);
        assert!(chunks[0].1.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn test_chunk_firmware_one_byte_chunks() {
        let data = vec![10, 20, 30];
        let chunks = chunk_firmware(&data, 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], (0, vec![10]));
        assert_eq!(chunks[1], (1, vec![20]));
        assert_eq!(chunks[2], (2, vec![30]));
    }

    #[test]
    fn test_firmware_crc32() {
        let data = b"hello firmware";
        let crc = firmware_crc32(data);
        assert_eq!(crc, crc32fast::hash(data));
    }

    #[test]
    fn test_firmware_crc32_empty() {
        let crc = firmware_crc32(&[]);
        assert_eq!(crc, crc32fast::hash(&[]));
    }

    #[test]
    fn test_firmware_crc32_deterministic() {
        let data = vec![0xFF; 4096];
        let crc1 = firmware_crc32(&data);
        let crc2 = firmware_crc32(&data);
        assert_eq!(crc1, crc2);
    }
}
