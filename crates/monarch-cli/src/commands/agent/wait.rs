//! `monarch agent wait`: ask the server five times a second whether the
//! pane has reached a state, passing back its mark so a state that came
//! and went between two questions still counts.

use std::io::Write;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use monarch_core::{AgentState, PaneId};
use monarch_proto::{AgentStatus, AgentWait, AgentWaitReply};

use super::older;
use crate::WaitArgs;
use crate::hook::ApiClient;

/// How often the server is asked.
const PAUSE: Duration = Duration::from_millis(200);

/// Exit status when `--timeout` runs out.
const TIMED_OUT: u8 = 2;

/// How a wait ended.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// The pane reached this state; its status now.
    Reached(AgentState, AgentStatus),
    /// Time ran out; its status then.
    TimedOut(AgentStatus),
}

/// Waits as `args` says, printing how it ended.
pub(super) fn run(
    args: &WaitArgs,
    client: &mut ApiClient,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let timeout = args.timeout.map(Duration::from_secs_f64);
    let ask = |w: &AgentWait| older(client.request(AgentWait::METHOD, w), "wait");
    match wait_for(ask, args.pane, &args.states(), timeout, PAUSE)? {
        Outcome::Reached(state, status) => {
            writeln!(out, "{}", line(args.pane, state, &status))?;
            Ok(ExitCode::SUCCESS)
        }
        Outcome::TimedOut(status) => {
            let secs = args.timeout.unwrap_or_default();
            let now = line(args.pane, status.state, &status);
            writeln!(out, "timed out after {secs}s: {now}")?;
            Ok(ExitCode::from(TIMED_OUT))
        }
    }
}

/// Asks with `ask` every `pause` until a state in `until` is reached or
/// `timeout` passes.
fn wait_for(
    mut ask: impl FnMut(&AgentWait) -> anyhow::Result<AgentWaitReply>,
    pane: PaneId,
    until: &[AgentState],
    timeout: Option<Duration>,
    pause: Duration,
) -> anyhow::Result<Outcome> {
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut question = AgentWait {
        pane,
        until: until.to_vec(),
        since: None,
    };
    loop {
        let reply = ask(&question)?;
        if let Some(state) = reply.reached {
            return Ok(Outcome::Reached(state, reply.status));
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Ok(Outcome::TimedOut(reply.status));
        }
        question.since = question.since.or(Some(reply.mark));
        std::thread::sleep(pause);
    }
}

/// `p3 ◆ Blocked: Allow Bash: rm -rf target?`.
fn line(pane: PaneId, state: AgentState, status: &AgentStatus) -> String {
    let activity = status
        .activity
        .as_deref()
        .map(|a| format!(": {a}"))
        .unwrap_or_default();
    format!("{pane} {} {}{activity}", state.glyph(), state.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(reached: Option<AgentState>, state: AgentState, mark: u64) -> AgentWaitReply {
        AgentWaitReply {
            reached,
            status: AgentStatus {
                state,
                ..AgentStatus::default()
            },
            mark,
        }
    }

    #[test]
    fn asks_again_with_the_first_mark_until_reached() {
        let mut seen = Vec::new();
        let mut replies = vec![
            reply(None, AgentState::Working, 4),
            reply(None, AgentState::Working, 6),
            reply(Some(AgentState::Blocked), AgentState::Working, 9),
        ]
        .into_iter();
        let ask = |w: &AgentWait| {
            seen.push(w.since);
            Ok(replies.next().expect("asked too often"))
        };
        let outcome = wait_for(ask, PaneId(3), &[AgentState::Blocked], None, Duration::ZERO);
        let working = AgentStatus {
            state: AgentState::Working,
            ..AgentStatus::default()
        };
        assert_eq!(
            outcome.ok(),
            Some(Outcome::Reached(AgentState::Blocked, working))
        );
        assert_eq!(seen, [None, Some(4), Some(4)], "the first mark is kept");
    }

    #[test]
    fn timeouts_and_errors_end_the_wait() {
        let ask = |_: &AgentWait| Ok(reply(None, AgentState::Working, 1));
        let outcome = wait_for(
            ask,
            PaneId(1),
            &[AgentState::Done],
            Some(Duration::ZERO),
            Duration::ZERO,
        );
        assert!(matches!(outcome, Ok(Outcome::TimedOut(s)) if s.state == AgentState::Working));
        let gone = |_: &AgentWait| anyhow::bail!("no pane p1 in this session");
        let failed = wait_for(gone, PaneId(1), &[AgentState::Done], None, Duration::ZERO);
        assert!(failed.is_err());
    }

    #[test]
    fn lines_name_the_pane_state_and_activity() {
        let status = AgentStatus {
            state: AgentState::Blocked,
            activity: Some("Allow Bash: rm?".into()),
            agent: None,
        };
        assert_eq!(
            line(PaneId(3), AgentState::Blocked, &status),
            "p3 ◆ Blocked: Allow Bash: rm?"
        );
        assert_eq!(
            line(PaneId(1), AgentState::Idle, &AgentStatus::default()),
            "p1 ○ Idle"
        );
    }
}
