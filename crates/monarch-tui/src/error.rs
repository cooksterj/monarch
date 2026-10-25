//! Errors raised by the terminal client.

use thiserror::Error;

/// Why attaching failed or ended abnormally.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TuiError {
    /// Terminal or socket I/O failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A frame could not be encoded or decoded.
    #[error(transparent)]
    Proto(#[from] monarch_proto::ProtoError),
    /// The server refused the connection.
    #[error("server refused to attach: {0}")]
    Refused(String),
    /// The config cannot drive the client (e.g. a prefix that is not a
    /// control chord).
    #[error("{0}")]
    Config(String),
}

impl TuiError {
    /// Wraps an I/O error with what was being attempted.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_carry_context() {
        let e = TuiError::io("connecting", std::io::Error::other("no server"));
        assert_eq!(e.to_string(), "connecting: no server");
        assert_eq!(TuiError::Config("bad".into()).to_string(), "bad");
    }
}
