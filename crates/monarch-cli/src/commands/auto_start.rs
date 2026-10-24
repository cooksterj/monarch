//! Starting a session server in the background when none is listening.

use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context;

/// How long a freshly started server gets to open its socket.
const START_TIMEOUT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(20);

/// Makes sure a server for `session` listens on `socket`.
pub(crate) fn ensure_server(session: &str, socket: &Path) -> anyhow::Result<()> {
    if UnixStream::connect(socket).is_ok() {
        return Ok(());
    }
    let log_path = socket.with_extension("log");
    let log =
        File::create(&log_path).with_context(|| format!("creating {}", log_path.display()))?;
    let exe = std::env::current_exe().context("locating the monarch binary")?;
    let mut server = Command::new(exe)
        .args(["server", "--daemon", "--session", session])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .context("starting the session server")?;
    // Reap it if it exits while this client still runs, so it never lingers
    // as a zombie.
    thread::spawn(move || server.wait());
    wait_for_socket(socket, START_TIMEOUT)
        .with_context(|| format!("server did not start; see {}", log_path.display()))
}

/// Polls until something accepts connections on `socket`.
fn wait_for_socket(socket: &Path, timeout: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            socket.display()
        );
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn a_listening_socket_is_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("s.sock");
        let _listener = UnixListener::bind(&path).expect("bind");
        wait_for_socket(&path, Duration::from_millis(100)).expect("found");
    }

    #[test]
    fn a_missing_socket_times_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = wait_for_socket(&dir.path().join("none.sock"), Duration::from_millis(50));
        assert!(err.is_err());
    }
}
