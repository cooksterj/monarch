//! `monarch kill <name>`: end a session on purpose.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::bail;
use monarch_proto::KillSession;
use monarch_server::SocketPath;

use crate::KillArgs;
use crate::hook::ApiClient;

/// How long the server may take to hang its panes up and go.
const GONE_WITHIN: Duration = Duration::from_secs(5);

/// Asks the session's server to stop and waits for its socket to go. Its
/// panes are hung up and its state file removed: it won't be restored.
pub(crate) fn run(args: &KillArgs, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    SocketPath::validate(&args.name)?;
    let socket = SocketPath::dir().join(format!("{}.sock", args.name));
    let Ok(mut client) = ApiClient::connect(&socket) else {
        bail!(
            "no session named {:?} is running (see `monarch ls`)",
            args.name
        );
    };
    // The server may be gone before it answers, so only a refusal counts.
    if let Err(e) = client.call(KillSession::METHOD, &KillSession {})
        && e.to_string().contains("unknown method")
    {
        bail!(
            "session {name:?} runs an older Monarch that can't be killed this way; \
             stop it with: pkill -f 'monarch server --daemon --session {name}$'",
            name = args.name
        );
    }
    if !gone(&socket, GONE_WITHIN) {
        bail!(
            "session {:?} did not stop within {}s",
            args.name,
            GONE_WITHIN.as_secs()
        );
    }
    writeln!(out, "killed session {}", args.name)?;
    Ok(ExitCode::SUCCESS)
}

/// Whether `socket` disappears within `limit`.
fn gone(socket: &Path, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while socket.exists() {
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_sessions_and_bad_names_fail() {
        let mut out = Vec::new();
        let missing = KillArgs {
            name: format!("no-such-session-{}", std::process::id()),
        };
        let err = run(&missing, &mut out).expect_err("not running");
        assert!(err.to_string().contains("monarch ls"), "{err}");
        let bad = KillArgs {
            name: "../x".into(),
        };
        assert!(run(&bad, &mut out).is_err());
        assert!(out.is_empty());
    }

    #[test]
    fn a_socket_that_stays_is_not_gone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.sock");
        std::fs::write(&file, b"").expect("write");
        assert!(!gone(&file, Duration::from_millis(60)));
        assert!(gone(&dir.path().join("none.sock"), Duration::ZERO));
    }
}
