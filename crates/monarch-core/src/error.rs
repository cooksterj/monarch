//! The error type shared by everything in `monarch-core`.

use std::path::PathBuf;

/// Errors produced by `monarch-core`.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The config file exists but could not be read.
    #[error("failed to read config file {path}: {source}")]
    ConfigRead {
        /// Path that was being read.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// The config file is not valid TOML or has values of the wrong type.
    #[error("invalid config file {path}: {message}")]
    ConfigParse {
        /// Path that was being parsed.
        path: PathBuf,
        /// Human-readable parser message, including line and column.
        message: String,
    },
    /// No home directory could be determined to locate the config file.
    #[error("cannot locate a home directory; set MONARCH_CONFIG or HOME")]
    NoHome,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_message_names_the_file() {
        let err = CoreError::ConfigParse {
            path: PathBuf::from("/tmp/x.toml"),
            message: "bad".into(),
        };
        assert_eq!(err.to_string(), "invalid config file /tmp/x.toml: bad");
    }
}
