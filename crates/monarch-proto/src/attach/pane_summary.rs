//! One pane as tab labels and the go-to picker name it.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::{AgentStatus, PanePlace};

/// A pane's names when the layout was sent: the title its program set and
/// the program in the foreground, and where it works. Later changes arrive
/// as `ServerMsg::Title`, `ServerMsg::Command`, `ServerMsg::Place` and
/// `ServerMsg::Agent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSummary {
    /// The pane.
    pub pane: PaneId,
    /// The title its program set (OSC 0/2), if any.
    pub title: Option<String>,
    /// The foreground program's name (`zsh`, `vim`), if known.
    pub command: Option<String>,
    /// Its working directory and git checkout, if known.
    pub place: Option<PanePlace>,
    /// What its agent is doing (`Unknown` for a plain shell).
    pub agent: AgentStatus,
}

impl PaneSummary {
    /// What to call the pane: a non-blank title, else the command.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        PaneSummary::pick(self.title.as_deref(), self.command.as_deref())
    }

    /// A non-blank `title`, else `command`.
    #[must_use]
    pub fn pick<'a>(title: Option<&'a str>, command: Option<&'a str>) -> Option<&'a str> {
        title.map(str::trim).filter(|t| !t.is_empty()).or(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_wins_over_the_command_unless_blank() {
        let mut pane = PaneSummary {
            pane: PaneId(1),
            title: Some(" vim main.rs ".into()),
            command: Some("vim".into()),
            place: None,
            agent: AgentStatus::default(),
        };
        assert_eq!(pane.name(), Some("vim main.rs"));
        pane.title = Some("  ".into());
        assert_eq!(pane.name(), Some("vim"));
        pane.command = None;
        assert_eq!(pane.name(), None);
        let bytes = postcard::to_allocvec(&pane).expect("encode");
        assert_eq!(postcard::from_bytes::<PaneSummary>(&bytes).ok(), Some(pane));
    }
}
