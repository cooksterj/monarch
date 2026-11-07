//! One pane as the go-to picker lists it.

use monarch_core::{AgentState, PaneId};

/// A pane: where it is, what it is called, what its agent is doing and
/// whether it is the focused one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoToPane {
    /// The tab it is in, counting from 0.
    pub tab: usize,
    /// The pane.
    pub pane: PaneId,
    /// Its name in rows and lists; blank when it has none.
    pub name: String,
    /// Its agent's state; `Unknown` for a plain shell.
    pub state: AgentState,
    /// Whether it is the focused pane.
    pub here: bool,
}

impl GoToPane {
    /// `2 claude`, or `2 pane 7` for an unnamed pane.
    #[must_use]
    pub fn label(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            format!("{} pane {}", self.tab + 1, self.pane.0)
        } else {
            format!("{} {name}", self.tab + 1)
        }
    }

    /// `here`, else the agent's state in lower case, else nothing.
    #[must_use]
    pub fn detail(&self) -> String {
        if self.here {
            "here".into()
        } else if self.state == AgentState::Unknown {
            String::new()
        } else {
            self.state.label().to_lowercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_name_the_tab_and_say_the_state_or_here() {
        let pane = GoToPane {
            tab: 1,
            pane: PaneId(7),
            name: " claude ".into(),
            state: AgentState::Blocked,
            here: false,
        };
        assert_eq!(
            (pane.label(), pane.detail()),
            ("2 claude".into(), "blocked".into())
        );
        let unnamed = GoToPane {
            name: String::new(),
            state: AgentState::Unknown,
            here: true,
            ..pane
        };
        assert_eq!(
            (unnamed.label(), unnamed.detail()),
            ("2 pane 7".into(), "here".into())
        );
        let shell = GoToPane {
            state: AgentState::Unknown,
            ..pane
        };
        assert_eq!(shell.detail(), "");
    }
}
