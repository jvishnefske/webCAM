/// Errors that can occur during payload encoding / decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadError {
    /// CBOR (or other) encoding failed.
    EncodeFailed,
    /// CBOR (or other) decoding failed.
    DecodeFailed,
    /// Output buffer is too small.
    BufferTooSmall,
}

/// Encode a CBOR-serialisable value into `buf`.
///
/// Returns the number of bytes written on success.
#[cfg(feature = "cbor")]
pub fn encode<T: minicbor::Encode<()>>(val: &T, buf: &mut [u8]) -> Result<usize, PayloadError> {
    let mut writer = SliceWriter::new(buf);
    minicbor::encode(val, &mut writer).map_err(|_| PayloadError::EncodeFailed)?;
    Ok(writer.pos)
}

/// Decode a CBOR-encoded value from `buf`.
#[cfg(feature = "cbor")]
pub fn decode<'b, T: minicbor::Decode<'b, ()>>(buf: &'b [u8]) -> Result<T, PayloadError> {
    minicbor::decode(buf).map_err(|_| PayloadError::DecodeFailed)
}

/// A minimal `minicbor::encode::Write` implementation over a `&mut [u8]`.
#[cfg(feature = "cbor")]
struct SliceWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

#[cfg(feature = "cbor")]
impl<'a> SliceWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }
}

#[cfg(feature = "cbor")]
impl minicbor::encode::Write for SliceWriter<'_> {
    type Error = PayloadError;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let end = self.pos + data.len();
        if end > self.buf.len() {
            return Err(PayloadError::BufferTooSmall);
        }
        self.buf[self.pos..end].copy_from_slice(data);
        self.pos = end;
        Ok(())
    }
}

/// Copy raw bytes into `buf` without any encoding.
///
/// Returns the number of bytes written.
pub fn encode_raw(data: &[u8], buf: &mut [u8]) -> Result<usize, PayloadError> {
    if data.len() > buf.len() {
        return Err(PayloadError::BufferTooSmall);
    }
    buf[..data.len()].copy_from_slice(data);
    Ok(data.len())
}

/// Return raw bytes as-is (identity decode).
pub fn decode_raw<'b>(buf: &'b [u8]) -> &'b [u8] {
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- raw passthrough tests ----

    #[test]
    fn raw_roundtrip() {
        let data = [1u8, 2, 3, 4, 5];
        let mut buf = [0u8; 16];
        let n = encode_raw(&data, &mut buf).unwrap();
        assert_eq!(n, data.len());
        let out = decode_raw(&buf[..n]);
        assert_eq!(out, &data);
    }

    #[test]
    fn raw_empty() {
        let mut buf = [0u8; 8];
        let n = encode_raw(&[], &mut buf).unwrap();
        assert_eq!(n, 0);
        assert_eq!(decode_raw(&buf[..n]), &[]);
    }

    #[test]
    fn raw_buffer_too_small() {
        let data = [0u8; 10];
        let mut buf = [0u8; 5];
        assert_eq!(encode_raw(&data, &mut buf), Err(PayloadError::BufferTooSmall));
    }

    // ---- CBOR tests (only when feature enabled) ----

    #[cfg(feature = "cbor")]
    mod cbor_tests {
        use super::*;

        #[test]
        fn cbor_roundtrip_u32() {
            let val: u32 = 0xCAFE_BABE;
            let mut buf = [0u8; 16];
            let n = encode(&val, &mut buf).unwrap();
            assert!(n > 0);
            let decoded: u32 = decode(&buf[..n]).unwrap();
            assert_eq!(decoded, val);
        }

        #[test]
        fn cbor_roundtrip_bool() {
            let mut buf = [0u8; 4];
            let n = encode(&true, &mut buf).unwrap();
            let decoded: bool = decode(&buf[..n]).unwrap();
            assert!(decoded);
        }

        #[test]
        fn cbor_roundtrip_i16() {
            let val: i16 = -1234;
            let mut buf = [0u8; 16];
            let n = encode(&val, &mut buf).unwrap();
            let decoded: i16 = decode(&buf[..n]).unwrap();
            assert_eq!(decoded, val);
        }

        #[test]
        fn cbor_encode_buffer_too_small() {
            let val: u64 = u64::MAX;
            let mut buf = [0u8; 1]; // too small for a full u64 CBOR
            let result = encode(&val, &mut buf);
            assert!(result.is_err());
        }

        #[test]
        fn cbor_decode_invalid() {
            let garbage = [0xFF, 0xFF, 0xFF];
            let result: Result<u32, _> = decode(&garbage);
            assert_eq!(result, Err(PayloadError::DecodeFailed));
        }
    }
}
