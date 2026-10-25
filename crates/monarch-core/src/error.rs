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
    /// An agent name Monarch does not know.
    #[error("unknown agent {0:?}; expected one of: {choices}", choices = crate::AgentKind::choices())]
    UnknownAgent(String),
    /// An agent state name Monarch does not know.
    #[error("unknown state {0:?}; expected one of: working, blocked, idle, done, error, exited")]
    UnknownState(String),
    /// An action name Monarch does not know.
    #[error("unknown action {0:?} (see `monarch keys` for the list)")]
    UnknownAction(String),
    /// A key name Monarch cannot read.
    #[error(
        "unknown key {0:?}: use a single character or left, right, up, down, tab, enter, esc, space"
    )]
    UnknownKey(String),
    /// An id that is not `<prefix><number>` (e.g. `p3`) or a number.
    #[error("invalid id {0:?}")]
    InvalidId(String),
    /// A keymap preset Monarch does not have.
    #[error("unknown preset {0:?}; expected monarch, tmux or herdr")]
    UnknownPreset(String),
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
