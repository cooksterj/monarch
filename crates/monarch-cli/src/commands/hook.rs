//! `monarch hook <agent>`: report an agent's hook event to its pane's
//! session.
//!
//! Hooks run on every agent event, so this is quick and never gets in the
//! agent's way: outside Monarch it does nothing, and any failure is a line
//! on stderr with a successful exit (a failing hook would interrupt the
//! agent).

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use monarch_core::PaneId;
use monarch_proto::AgentReport;
use serde_json::Value;

use crate::HookCommand;
use crate::hook::{AgentHook, ApiClient};

/// Largest hook input read; tool inputs (a whole file for `Write`) can be
/// big, and anything past this is not needed for a report.
const MAX_INPUT: u64 = 8 * 1024 * 1024;

/// Where the hook runs: the pane and socket its environment names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HookEnv {
    /// `MONARCH_PANE`.
    pub pane: Option<String>,
    /// `MONARCH_SOCKET`.
    pub socket: Option<PathBuf>,
}

impl HookEnv {
    /// Reads the process environment.
    pub fn from_env() -> Self {
        Self {
            pane: std::env::var("MONARCH_PANE").ok(),
            socket: std::env::var_os("MONARCH_SOCKET").map(PathBuf::from),
        }
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
    let (Some(pane), Some(socket)) = (&env.pane, &env.socket) else {
        return Ok(());
    };
    let pane: PaneId = pane.parse()?;
    let mut text = Vec::new();
    input.take(MAX_INPUT).read_to_end(&mut text)?;
    let json: Value = serde_json::from_slice(&text)?;
    if let Some(report) = AgentHook::new(cmd.agent(), &json).report(pane) {
        ApiClient::connect(socket)?.call(AgentReport::METHOD, &report)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        assert!(report(HookCommand::Claude, &env, &b"not json"[..]).is_err());
        let bad_pane = HookEnv {
            pane: Some("x".into()),
            ..env.clone()
        };
        assert!(report(HookCommand::Claude, &bad_pane, &b"{}"[..]).is_err());
        let idle = br#"{"hook_event_name":"SubagentStop"}"#;
        assert!(
            report(HookCommand::Claude, &env, &idle[..]).is_ok(),
            "nothing to send"
        );
        assert_eq!(run(HookCommand::Claude, &env, &b"{"[..]), ExitCode::SUCCESS);
    }
}
