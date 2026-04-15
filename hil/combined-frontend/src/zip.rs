//! Minimal ZIP archive builder for in-browser file downloads.
//!
//! Builds a valid ZIP archive from a list of `(path, content)` string pairs.
//! Uses store (no compression) for simplicity since the primary use case is
//! generated Rust source code which compresses poorly at small sizes. The
//! output is a valid ZIP that any standard tool can extract.
//!
//! This module has no platform dependencies and is tested on the host target.

/// Build a ZIP archive from a list of (path, content) pairs.
///
/// Returns the raw bytes of a valid ZIP file using the store method
/// (no compression). Each file entry uses CRC-32 checksums computed
/// via the `crc32fast` crate.
pub fn build_zip(files: &[(String, String)]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut central_directory = Vec::new();
    let mut offset: u32 = 0;

    for (path, content) in files {
        let path_bytes = path.as_bytes();
        let content_bytes = content.as_bytes();
        let crc = crc32fast::hash(content_bytes);
        let compressed_size = content_bytes.len() as u32;
        let uncompressed_size = compressed_size;

        // Local file header
        let local_header_start = offset;
        buf.extend_from_slice(&LOCAL_FILE_HEADER_SIG);
        buf.extend_from_slice(&20u16.to_le_bytes()); // version needed to extract
        buf.extend_from_slice(&0u16.to_le_bytes()); // general purpose bit flag
        buf.extend_from_slice(&0u16.to_le_bytes()); // compression method: store
        buf.extend_from_slice(&0u16.to_le_bytes()); // last mod file time
        buf.extend_from_slice(&0u16.to_le_bytes()); // last mod file date
        buf.extend_from_slice(&crc.to_le_bytes()); // crc-32
        buf.extend_from_slice(&compressed_size.to_le_bytes());
        buf.extend_from_slice(&uncompressed_size.to_le_bytes());
        buf.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes()); // file name length
        buf.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        buf.extend_from_slice(path_bytes);
        buf.extend_from_slice(content_bytes);

        let local_entry_size = 30 + path_bytes.len() as u32 + compressed_size;

        // Central directory entry
        central_directory.extend_from_slice(&CENTRAL_DIR_HEADER_SIG);
        central_directory.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central_directory.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // flags
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // compression: store
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central_directory.extend_from_slice(&crc.to_le_bytes());
        central_directory.extend_from_slice(&compressed_size.to_le_bytes());
        central_directory.extend_from_slice(&uncompressed_size.to_le_bytes());
        central_directory.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // file comment length
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // internal file attrs
        central_directory.extend_from_slice(&0u32.to_le_bytes()); // external file attrs
        central_directory.extend_from_slice(&local_header_start.to_le_bytes());
        central_directory.extend_from_slice(path_bytes);

        offset += local_entry_size;
    }

    let central_dir_offset = offset;
    let central_dir_size = central_directory.len() as u32;
    let entry_count = files.len() as u16;

    buf.extend_from_slice(&central_directory);

    // End of central directory record
    buf.extend_from_slice(&END_OF_CENTRAL_DIR_SIG);
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk number
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir
    buf.extend_from_slice(&entry_count.to_le_bytes()); // entries on this disk
    buf.extend_from_slice(&entry_count.to_le_bytes()); // total entries
    buf.extend_from_slice(&central_dir_size.to_le_bytes());
    buf.extend_from_slice(&central_dir_offset.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment length

    buf
}

/// ZIP local file header signature.
const LOCAL_FILE_HEADER_SIG: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// ZIP central directory file header signature.
const CENTRAL_DIR_HEADER_SIG: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];

/// ZIP end of central directory record signature.
const END_OF_CENTRAL_DIR_SIG: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_empty() {
        let data = build_zip(&[]);
        // An empty ZIP has just the end-of-central-directory record (22 bytes).
        assert_eq!(data.len(), 22);
        // Starts with the EOCD signature.
        assert_eq!(&data[0..4], &END_OF_CENTRAL_DIR_SIG);
        // Entry count is 0.
        assert_eq!(u16::from_le_bytes([data[8], data[9]]), 0);
    }

    #[test]
    fn test_zip_single_file() {
        let files = vec![("hello.txt".to_string(), "Hello, world!".to_string())];
        let data = build_zip(&files);

        // Must start with a local file header.
        assert_eq!(&data[0..4], &LOCAL_FILE_HEADER_SIG);

        // Verify CRC-32 in the local header (offset 14).
        let expected_crc = crc32fast::hash(b"Hello, world!");
        let stored_crc = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
        assert_eq!(stored_crc, expected_crc);

        // File name should appear at offset 30.
        let name_len = u16::from_le_bytes([data[26], data[27]]) as usize;
        assert_eq!(name_len, 9);
        assert_eq!(&data[30..30 + name_len], b"hello.txt");

        // Content follows the filename.
        let content_start = 30 + name_len;
        let content_len = u32::from_le_bytes([data[18], data[19], data[20], data[21]]) as usize;
        assert_eq!(content_len, 13);
        assert_eq!(
            &data[content_start..content_start + content_len],
            b"Hello, world!"
        );

        // Verify the EOCD says 1 entry.
        let eocd_start = data.len() - 22;
        assert_eq!(&data[eocd_start..eocd_start + 4], &END_OF_CENTRAL_DIR_SIG);
        assert_eq!(
            u16::from_le_bytes([data[eocd_start + 8], data[eocd_start + 9]]),
            1
        );
    }

    #[test]
    fn test_zip_multiple_files() {
        let files = vec![
            ("src/main.rs".to_string(), "fn main() {}".to_string()),
            (
                "Cargo.toml".to_string(),
                "[package]\nname = \"test\"".to_string(),
            ),
            ("README.md".to_string(), "# Test".to_string()),
        ];
        let data = build_zip(&files);

        // Must start with local file header.
        assert_eq!(&data[0..4], &LOCAL_FILE_HEADER_SIG);

        // EOCD should report 3 entries.
        let eocd_start = data.len() - 22;
        assert_eq!(&data[eocd_start..eocd_start + 4], &END_OF_CENTRAL_DIR_SIG);
        assert_eq!(
            u16::from_le_bytes([data[eocd_start + 8], data[eocd_start + 9]]),
            3
        );
    }

    #[test]
    fn test_zip_roundtrip_header() {
        // Verify that the central directory offset stored in EOCD points to
        // the actual central directory.
        let files = vec![
            ("a.txt".to_string(), "aaa".to_string()),
            ("b.txt".to_string(), "bbb".to_string()),
        ];
        let data = build_zip(&files);

        let eocd_start = data.len() - 22;
        let cd_offset = u32::from_le_bytes([
            data[eocd_start + 16],
            data[eocd_start + 17],
            data[eocd_start + 18],
            data[eocd_start + 19],
        ]) as usize;

        // Central directory starts with the CD header signature.
        assert_eq!(&data[cd_offset..cd_offset + 4], &CENTRAL_DIR_HEADER_SIG);

        // The offset stored in the first CD entry for the local file header
        // should be 0 (first file starts at beginning of archive).
        let local_offset = u32::from_le_bytes([
            data[cd_offset + 42],
            data[cd_offset + 43],
            data[cd_offset + 44],
            data[cd_offset + 45],
        ]);
        assert_eq!(local_offset, 0);
    }

    #[test]
    fn test_zip_unicode_content() {
        let files = vec![("unicode.txt".to_string(), "Hello, world! Rust".to_string())];
        let data = build_zip(&files);

        // Should still produce a valid ZIP structure.
        assert_eq!(&data[0..4], &LOCAL_FILE_HEADER_SIG);

        // CRC should match the UTF-8 bytes.
        let expected_crc = crc32fast::hash("Hello, world! Rust".as_bytes());
        let stored_crc = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
        assert_eq!(stored_crc, expected_crc);
    }

    #[test]
    fn test_zip_empty_content_file() {
        let files = vec![("empty.txt".to_string(), String::new())];
        let data = build_zip(&files);

        // Compressed and uncompressed size should be 0.
        let compressed = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        let uncompressed = u32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        assert_eq!(compressed, 0);
        assert_eq!(uncompressed, 0);

        // CRC of empty data.
        let expected_crc = crc32fast::hash(b"");
        let stored_crc = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
        assert_eq!(stored_crc, expected_crc);
    }
}
