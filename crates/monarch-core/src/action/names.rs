//! Action names as written in config (`[keys.bind]`) and the palette.

use std::fmt;
use std::str::FromStr;

use super::Action;
use crate::{CoreError, Direction, SplitAxis};

/// Actions without a parameter, by name.
const PLAIN: [(&str, Action); 13] = [
    ("split-right", Action::Split(SplitAxis::Columns)),
    ("split-down", Action::Split(SplitAxis::Rows)),
    ("close-pane", Action::ClosePane),
    ("focus-next", Action::FocusNext),
    ("focus-prev", Action::FocusPrev),
    ("swap-next", Action::SwapNext),
    ("swap-prev", Action::SwapPrev),
    ("zoom", Action::ToggleZoom),
    ("equalize", Action::Equalize),
    ("new-tab", Action::NewTab),
    ("next-tab", Action::NextTab),
    ("prev-tab", Action::PrevTab),
    ("detach", Action::Detach),
];

/// Client actions without a parameter, by name (kept apart so `PLAIN`
/// stays the session's list).
const LOCAL: [(&str, Action); 13] = [
    ("toggle-mouse", Action::ToggleMouse),
    ("command-palette", Action::CommandPalette),
    ("go-to-pane", Action::GoToPane),
    ("copy-mode", Action::CopyMode),
    ("toggle-sidebar", Action::ToggleSidebar),
    ("reload-config", Action::ReloadConfig),
    ("inbox", Action::Inbox),
    ("next-blocked", Action::NextBlocked),
    ("review", Action::Review),
    ("new-worktree", Action::NewWorktree),
    ("projects", Action::Projects),
    ("sessions", Action::Sessions),
    ("help", Action::Help),
];

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((name, _)) = PLAIN.iter().chain(&LOCAL).find(|(_, a)| a == self) {
            return f.write_str(name);
        }
        match self {
            Self::Focus(d) => write!(f, "focus-{}", d.name()),
            Self::Swap(d) => write!(f, "swap-{}", d.name()),
            Self::Resize(d) => write!(f, "resize-{}", d.name()),
            Self::SelectTab(n) => write!(f, "tab-{n}"),
            Self::FocusPane(pane) => write!(f, "focus-pane-{}", pane.0),
            other => write!(f, "{other:?}"),
        }
    }
}

impl FromStr for Action {
    type Err = CoreError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim().to_ascii_lowercase();
        if let Some((_, action)) = PLAIN.iter().chain(&LOCAL).find(|(n, _)| *n == text) {
            return Ok(*action);
        }
        directional(&text)
            .or_else(|| tab(&text))
            .ok_or(CoreError::UnknownAction(text))
    }
}

impl Action {
    /// Every bindable action except tab numbers, in a stable order: what
    /// the command palette lists.
    #[must_use]
    pub fn catalogue() -> Vec<Self> {
        let mut all: Vec<Self> = PLAIN.iter().chain(&LOCAL).map(|(_, a)| *a).collect();
        for make in [Self::Focus, Self::Swap, Self::Resize] {
            all.extend(Direction::ALL.into_iter().map(make));
        }
        all
    }
}

/// `focus-left`, `swap-up`, `resize-down`…
fn directional(text: &str) -> Option<Action> {
    let (verb, dir) = text.split_once('-')?;
    let dir = Direction::ALL.into_iter().find(|d| d.name() == dir)?;
    match verb {
        "focus" => Some(Action::Focus(dir)),
        "swap" => Some(Action::Swap(dir)),
        "resize" => Some(Action::Resize(dir)),
        _ => None,
    }
}

/// `tab-1` … `tab-99`.
fn tab(text: &str) -> Option<Action> {
    let n: u8 = text.strip_prefix("tab-")?.parse().ok()?;
    (1..100).contains(&n).then_some(Action::SelectTab(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_round_trips_through_its_name() {
        let mut all = Action::catalogue();
        all.extend([Action::SelectTab(1), Action::SelectTab(42)]);
        for action in all {
            let name = action.to_string();
            assert_eq!(name.parse::<Action>().expect(&name), action, "{name}");
        }
    }

    #[test]
    fn the_catalogue_lists_each_action_once_and_no_pane_ids() {
        let all = Action::catalogue();
        let unique: std::collections::BTreeSet<String> =
            all.iter().map(ToString::to_string).collect();
        assert_eq!(unique.len(), all.len());
        assert!(all.contains(&Action::GoToPane));
        assert!(!all.iter().any(|a| matches!(a, Action::FocusPane(_))));
        assert_eq!(
            Action::FocusPane(crate::PaneId(3)).to_string(),
            "focus-pane-3"
        );
        assert!("focus-pane-3".parse::<Action>().is_err(), "not bindable");
    }

    #[test]
    fn names_are_case_insensitive_and_bad_ones_are_errors() {
        assert_eq!(
            "Split-Right".parse::<Action>().ok(),
            Some(Action::Split(SplitAxis::Columns))
        );
        for bad in ["", "split", "focus-sideways", "tab-0", "tab-100", "explode"] {
            assert!(bad.parse::<Action>().is_err(), "{bad}");
        }
    }
}
