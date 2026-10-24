//! Errors from encoding and decoding attach-stream frames.

use thiserror::Error;

/// Why a frame could not be encoded or decoded.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProtoError {
    /// The message could not be serialized.
    #[error("cannot encode attach message: {0}")]
    Encode(#[source] postcard::Error),
    /// The frame body is not a valid message.
    #[error("cannot decode attach message: {0}")]
    Decode(#[source] postcard::Error),
    /// A frame announced a length over the limit.
    #[error("attach frame of {len} bytes exceeds the {max}-byte limit")]
    FrameTooLarge {
        /// Announced length.
        len: usize,
        /// Configured limit.
        max: usize,
    },
    /// The underlying stream failed.
    #[error("attach stream I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_limit() {
        let e = ProtoError::FrameTooLarge { len: 20, max: 10 };
        assert_eq!(
            e.to_string(),
            "attach frame of 20 bytes exceeds the 10-byte limit"
        );
    }
}
