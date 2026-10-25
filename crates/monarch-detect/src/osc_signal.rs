//! What an OSC notification or progress report from a pane says about the
//! agent in it: the fallback signal for agents without hooks.
//!
//! Notifications (OSC 9 text, OSC 777 `notify`, OSC 99 kitty) mean the
//! agent wants the user: blocked when the text asks for approval, done
//! otherwise. ConEmu progress (OSC 9;4) means working while it runs.

use monarch_core::AgentState;

/// Words in a notification that mean the agent is waiting on a decision.
const ASKS: [&str; 6] = [
    "approv",
    "permission",
    "allow",
    "confirm",
    "needs your",
    "requires your",
];

/// A state an OSC sequence implies, with the text to show for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscSignal {
    /// The implied state.
    pub state: AgentState,
    /// The notification's text, if it had one.
    pub activity: Option<String>,
}

impl OscSignal {
    /// Reads OSC `number` with `payload`; `None` for sequences that say
    /// nothing about state.
    #[must_use]
    pub fn parse(number: u32, payload: &str) -> Option<Self> {
        match number {
            9 => match payload.strip_prefix("4;") {
                Some(progress) => Self::progress(progress),
                // Other ConEmu commands are `9;<n>;…`.
                None if payload.split(';').next()?.parse::<u8>().is_ok() => None,
                None => Self::notice(payload),
            },
            777 => {
                let mut parts = payload.splitn(3, ';');
                (parts.next()? == "notify").then_some(())?;
                let (title, body) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                Self::notice(if body.trim().is_empty() { title } else { body })
            }
            99 => {
                let (meta, text) = payload.split_once(';')?;
                let plain = !meta.contains("e=1");
                let part = meta.split(':').find_map(|kv| kv.strip_prefix("p="));
                let shown = matches!(part, None | Some("title" | "body"));
                (plain && shown).then(|| Self::notice(text))?
            }
            _ => None,
        }
    }

    /// A notification: blocked if it asks for a decision, else done.
    fn notice(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let lower = text.to_lowercase();
        let state = if ASKS.iter().any(|w| lower.contains(w)) {
            AgentState::Blocked
        } else {
            AgentState::Done
        };
        Some(Self {
            state,
            activity: Some(text.to_owned()),
        })
    }

    /// `<state>[;<percent>]`: 0 ends it, 2 is an error, the rest running.
    fn progress(body: &str) -> Option<Self> {
        let state = match body.split(';').next()?.trim() {
            "0" => AgentState::Idle,
            "2" => AgentState::Error,
            "1" | "3" | "4" => AgentState::Working,
            _ => return None,
        };
        Some(Self {
            state,
            activity: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(number: u32, payload: &str) -> Option<AgentState> {
        OscSignal::parse(number, payload).map(|s| s.state)
    }

    #[test]
    fn notifications_are_done_unless_they_ask() {
        let done = OscSignal::parse(9, "Agent turn complete").expect("signal");
        assert_eq!(done.state, AgentState::Done);
        assert_eq!(done.activity.as_deref(), Some("Agent turn complete"));
        assert_eq!(
            state(9, "Approval requested: cargo test"),
            Some(AgentState::Blocked)
        );
        assert_eq!(
            state(9, "Claude needs your permission to use Bash"),
            Some(AgentState::Blocked)
        );
        assert_eq!(
            state(9, "Claude is waiting for your input"),
            Some(AgentState::Done)
        );
        assert_eq!(state(9, "   "), None);
    }

    #[test]
    fn progress_tracks_work() {
        assert_eq!(state(9, "4;1;40"), Some(AgentState::Working));
        assert_eq!(state(9, "4;3"), Some(AgentState::Working));
        assert_eq!(state(9, "4;2;100"), Some(AgentState::Error));
        assert_eq!(state(9, "4;0"), Some(AgentState::Idle));
        assert_eq!(OscSignal::parse(9, "4;1").and_then(|s| s.activity), None);
        assert_eq!(state(9, "4;x"), None);
        assert_eq!(state(9, "9;/home/me"), None, "ConEmu cwd, not a notice");
    }

    #[test]
    fn other_notification_protocols() {
        assert_eq!(
            state(777, "notify;Codex;Approve rm?"),
            Some(AgentState::Blocked)
        );
        let title_only = OscSignal::parse(777, "notify;Build done;").expect("signal");
        assert_eq!(title_only.activity.as_deref(), Some("Build done"));
        assert_eq!(state(777, "preexec;x"), None);
        assert_eq!(state(99, "i=1:d=0;Task finished"), Some(AgentState::Done));
        assert_eq!(
            state(99, "i=1:p=body;Allow edit?"),
            Some(AgentState::Blocked)
        );
        assert_eq!(state(99, "e=1;VGFzaw=="), None, "base64 not decoded");
        assert_eq!(state(99, "p=icon;x"), None);
        assert_eq!(state(7, "file:///tmp"), None);
        assert_eq!(state(133, "A"), None);
    }
}
