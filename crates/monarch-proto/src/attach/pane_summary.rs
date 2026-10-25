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

    /// A non-blank `title` (without its status decoration), else
    /// `command`.
    #[must_use]
    pub fn pick<'a>(title: Option<&'a str>, command: Option<&'a str>) -> Option<&'a str> {
        title
            .map(Self::clean_title)
            .filter(|t| !t.is_empty())
            .or(command)
    }

    /// `title` without the status symbols programs put in front of it,
    /// such as Claude Code's spinner (`✳`, `⠂`, `◐`): leading characters
    /// that are neither letters, digits nor ASCII are dropped, then the
    /// rest is trimmed. Monarch's sidebar shows agent state itself, and a
    /// spinner in a name makes tab labels flicker. ASCII titles
    /// (`[1] vim`, `~/src`) are kept whole.
    #[must_use]
    pub fn clean_title(title: &str) -> &str {
        title
            .trim_start_matches(|c: char| {
                c.is_whitespace() || !(c.is_alphanumeric() || c.is_ascii())
            })
            .trim_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinners_are_dropped_from_titles() {
        for spun in [
            "✳ Claude Code",
            "⠂ Claude Code",
            "◐ Claude Code",
            " ✻  Claude Code ",
        ] {
            assert_eq!(PaneSummary::clean_title(spun), "Claude Code", "{spun:?}");
        }
        assert_eq!(PaneSummary::clean_title("[1] vim"), "[1] vim");
        assert_eq!(PaneSummary::clean_title("~/src"), "~/src");
        assert_eq!(PaneSummary::clean_title("été"), "été", "letters are kept");
        assert_eq!(PaneSummary::clean_title("⠂"), "");
    }

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
        pane.title = Some("✳".into());
        assert_eq!(pane.name(), None, "decoration alone is blank");
        let bytes = postcard::to_allocvec(&pane).expect("encode");
        assert_eq!(postcard::from_bytes::<PaneSummary>(&bytes).ok(), Some(pane));
    }
}
