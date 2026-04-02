//! Backplane error types.

/// Errors that can occur during backplane operations.
#[derive(Debug)]
pub enum BackplaneError {
    /// Envelope header too short or invalid kind byte.
    InvalidEnvelope,
    /// CBOR encoding failed.
    EncodeFailed,
    /// CBOR decoding failed.
    DecodeFailed,
    /// Encode buffer too small for envelope + payload.
    BufferTooSmall,
    /// Request timed out waiting for a response.
    Timeout,
    /// Socket I/O error (std feature only).
    #[cfg(feature = "std")]
    Transport(std::io::Error),
    /// Embassy UDP socket send failed.
    #[cfg(feature = "embassy")]
    SendFailed,
    /// Embassy UDP socket receive failed.
    #[cfg(feature = "embassy")]
    RecvFailed,
    /// Embassy UDP socket bind failed.
    #[cfg(feature = "embassy")]
    BindFailed,
}

impl core::fmt::Display for BackplaneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidEnvelope => write!(f, "invalid envelope header"),
            Self::EncodeFailed => write!(f, "CBOR encode failed"),
            Self::DecodeFailed => write!(f, "CBOR decode failed"),
            Self::BufferTooSmall => write!(f, "encode buffer too small"),
            Self::Timeout => write!(f, "request timed out"),
            #[cfg(feature = "std")]
            Self::Transport(e) => write!(f, "transport error: {e}"),
            #[cfg(feature = "embassy")]
            Self::SendFailed => write!(f, "embassy UDP send failed"),
            #[cfg(feature = "embassy")]
            Self::RecvFailed => write!(f, "embassy UDP recv failed"),
            #[cfg(feature = "embassy")]
            Self::BindFailed => write!(f, "embassy UDP bind failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BackplaneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for BackplaneError {
    fn from(e: std::io::Error) -> Self {
        Self::Transport(e)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn display_all_variants() {
        assert_eq!(
            BackplaneError::InvalidEnvelope.to_string(),
            "invalid envelope header"
        );
        assert_eq!(
            BackplaneError::EncodeFailed.to_string(),
            "CBOR encode failed"
        );
        assert_eq!(
            BackplaneError::DecodeFailed.to_string(),
            "CBOR decode failed"
        );
        assert_eq!(
            BackplaneError::BufferTooSmall.to_string(),
            "encode buffer too small"
        );
        assert_eq!(BackplaneError::Timeout.to_string(), "request timed out");

        let io_err = std::io::Error::other("test");
        let transport_err = BackplaneError::Transport(io_err);
        assert!(transport_err.to_string().contains("transport error"));
    }

    #[test]
    fn error_source_transport() {
        let io_err = std::io::Error::other("inner");
        let bp_err = BackplaneError::Transport(io_err);
        assert!(std::error::Error::source(&bp_err).is_some());
    }

    #[test]
    fn error_source_non_transport() {
        assert!(std::error::Error::source(&BackplaneError::Timeout).is_none());
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::other("broken");
        let bp_err: BackplaneError = io_err.into();
        assert!(matches!(bp_err, BackplaneError::Transport(_)));
    }
}
