//! Errors raised by the session server.

use std::path::PathBuf;

use thiserror::Error;

/// Boxed error from a dependency that only exposes a dynamic error type.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Why the server could not start or keep going.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServerError {
    /// Socket or filesystem failure.
    #[error("{context}: {source}")]
    Io {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The PTY could not be opened or the program could not be started.
    #[error("cannot start pane: {0}")]
    Pty(#[source] BoxedError),
    /// A worktree could not be created, opened or removed (git's message,
    /// or why it wasn't tried).
    #[error("{0}")]
    Worktree(String),
    /// A server for this session is already listening.
    #[error("a server for session {session:?} is already running at {}", socket.display())]
    AlreadyRunning {
        /// Session name.
        session: String,
        /// Its socket.
        socket: PathBuf,
    },
    /// Session names become file names, so they are restricted.
    #[error("invalid session name {0:?}: use letters, digits, '-', '_' or '.'")]
    InvalidSession(String),
    /// Unix socket paths are short (104 bytes on macOS, 108 on Linux).
    #[error(
        "socket path {path:?} is {len} bytes, over the {max}-byte limit; use a shorter session name"
    )]
    SocketPathTooLong {
        /// The path that would be used.
        path: std::path::PathBuf,
        /// Its length in bytes.
        len: usize,
        /// The limit.
        max: usize,
    },
}

impl ServerError {
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
        let e = ServerError::io("binding socket", std::io::Error::other("busy"));
        assert_eq!(e.to_string(), "binding socket: busy");
    }
}
