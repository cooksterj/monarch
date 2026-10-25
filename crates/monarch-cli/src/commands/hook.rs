//! `monarch hook <agent>`: report an agent's hook event to its pane's
//! session.
//!
//! Hooks run on every agent event, so this is quick and never gets in the
//! agent's way: outside Monarch it does nothing, and any failure is a line
//! on stderr with a successful exit (a failing hook would interrupt the
//! agent).
//!
//! Claude Code runs hooks inside the pane, so its environment names the
//! pane and socket. Codex runs them in a shared background process whose
//! environment belongs to whichever pane started it, so a Codex report
//! names no pane and goes to every session; the one with a Codex pane in
//! that directory (or that session) takes it.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use monarch_core::PaneId;
use monarch_proto::AgentReport;
use monarch_server::SocketPath;
use serde_json::Value;

use crate::HookCommand;
use crate::hook::{AgentHook, ApiClient};

/// Largest hook input read; tool inputs (a whole file for `Write`) can be
/// big, and anything past this is not needed for a report.
const MAX_INPUT: u64 = 8 * 1024 * 1024;

/// Where the hook runs: the pane and socket its environment names, and
/// every session socket for reports that can't trust that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HookEnv {
    /// `MONARCH_PANE`.
    pub pane: Option<String>,
    /// `MONARCH_SOCKET`.
    pub socket: Option<PathBuf>,
    /// Every session's socket.
    pub sockets: Vec<PathBuf>,
}

impl HookEnv {
    /// Reads the process environment and the socket directory.
    pub fn from_env() -> Self {
        Self {
            pane: std::env::var("MONARCH_PANE").ok(),
            socket: std::env::var_os("MONARCH_SOCKET").map(PathBuf::from),
            sockets: SocketPath::all(),
        }
    }

    /// The sockets an unplaced report goes to, each once.
    fn every_socket(&self) -> Vec<PathBuf> {
        let mut all = self.sockets.clone();
        all.extend(self.socket.clone());
        all.sort();
        all.dedup();
        all
    }
}

/// Reads the hook's JSON from `input` and reports it; always succeeds.
pub(crate) fn run(cmd: HookCommand, env: &HookEnv, input: impl Read) -> ExitCode {
    if let Err(e) = report(cmd, env, input) {
        eprintln!("monarch hook: {e:#}");
    }
    ExitCode::SUCCESS
}

fn report(cmd: HookCommand, env: &HookEnv, input: impl Read) -> anyhow::Result<()> {
    match cmd {
        HookCommand::Claude => report_from_pane(cmd, env, input),
        HookCommand::Codex => report_to_every_session(cmd, env, input),
    }
}

/// The hook runs in its pane: report to that pane's session.
fn report_from_pane(cmd: HookCommand, env: &HookEnv, input: impl Read) -> anyhow::Result<()> {
    let (Some(pane), Some(socket)) = (&env.pane, &env.socket) else {
        return Ok(());
    };
    let pane: PaneId = pane.parse()?;
    if let Some(report) = AgentHook::new(cmd.agent(), &read_json(input)?).report(Some(pane)) {
        ApiClient::connect(socket)?.call(AgentReport::METHOD, &report)?;
    }
    Ok(())
}

/// The hook can't know its pane: offer the report to every session.
/// Sessions without a matching pane refuse it, and stale sockets don't
/// answer; neither is worth a message.
fn report_to_every_session(
    cmd: HookCommand,
    env: &HookEnv,
    input: impl Read,
) -> anyhow::Result<()> {
    let sockets = env.every_socket();
    if sockets.is_empty() {
        return Ok(());
    }
    if let Some(report) = AgentHook::new(cmd.agent(), &read_json(input)?).report(None) {
        for socket in sockets {
            let _ =
                ApiClient::connect(&socket).and_then(|mut c| c.call(AgentReport::METHOD, &report));
        }
    }
    Ok(())
}

fn read_json(input: impl Read) -> anyhow::Result<Value> {
    let mut text = Vec::new();
    input.take(MAX_INPUT).read_to_end(&mut text)?;
    Ok(serde_json::from_slice(&text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplaced_reports_go_to_every_session_once() {
        let env = HookEnv {
            pane: Some("p3".into()),
            socket: Some("/s/b.sock".into()),
            sockets: vec!["/s/a.sock".into(), "/s/b.sock".into()],
        };
        let all: Vec<PathBuf> = vec!["/s/a.sock".into(), "/s/b.sock".into()];
        assert_eq!(env.every_socket(), all);
    }

    #[test]
    fn outside_monarch_nothing_is_read_or_sent() {
        let env = HookEnv::default();
        assert!(report(HookCommand::Claude, &env, &b"not json"[..]).is_ok());
    }

    #[test]
    fn bad_input_fails_quietly() {
        let env = HookEnv {
            pane: Some("p1".into()),
            socket: Some("/nonexistent/m.sock".into()),
            sockets: Vec::new(),
        };
        assert!(report(HookCommand::Claude, &env, &b"not json"[..]).is_err());
        let bad_pane = HookEnv {
            pane: Some("x".into()),
            ..env.clone()
        };
        let codex = HookEnv {
            sockets: vec!["/nonexistent/a.sock".into()],
            ..HookEnv::default()
        };
        let stop = br#"{"hook_event_name":"Stop","cwd":"/w"}"#;
        assert!(
            report(HookCommand::Codex, &codex, &stop[..]).is_ok(),
            "dead sockets are fine"
        );
        assert!(
            report(HookCommand::Codex, &HookEnv::default(), &b"junk"[..]).is_ok(),
            "no sessions"
        );
        assert!(report(HookCommand::Claude, &bad_pane, &b"{}"[..]).is_err());
        let idle = br#"{"hook_event_name":"SubagentStop"}"#;
        assert!(
            report(HookCommand::Claude, &env, &idle[..]).is_ok(),
            "nothing to send"
        );
        assert_eq!(run(HookCommand::Claude, &env, &b"{"[..]), ExitCode::SUCCESS);
    }
}
