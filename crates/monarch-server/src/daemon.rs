//! Detaching a server process from the terminal that started it.

use crate::ServerError;

/// Puts the server in its own session so closing the terminal that
/// auto-started it (SIGHUP) does not take the panes down.
#[derive(Debug, Clone, Copy, Default)]
pub struct Daemon;

impl Daemon {
    /// Starts a new session with no controlling terminal.
    ///
    /// # Errors
    /// The process already leads a process group.
    pub fn detach() -> Result<(), ServerError> {
        rustix::process::setsid().map(drop).map_err(to_error)
    }
}

fn to_error(errno: rustix::io::Errno) -> ServerError {
    ServerError::io("starting a new session", errno.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // `detach` itself is not called here: it would move the test runner
    // out of the terminal's session.
    #[test]
    fn failure_carries_context() {
        let e = to_error(rustix::io::Errno::PERM);
        assert!(e.to_string().starts_with("starting a new session"), "{e}");
    }
}
