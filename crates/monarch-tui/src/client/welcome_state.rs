//! The welcome screen's content and clock: the keys a newcomer needs,
//! from the keymap in use, and when it opened.

use std::time::{Duration, Instant};

use monarch_core::{Action, Direction, Keymap, SplitAxis};

use crate::WhichKeyEntry;

/// Time between frames of the welcome animation.
pub const WELCOME_STEP: Duration = Duration::from_millis(150);

/// The actions the welcome lists, in order, said for a newcomer and
/// short enough for two columns at 80 (focus stands for its family).
const FIRST_KEYS: [(Action, &str); 10] = [
    (Action::Split(SplitAxis::Columns), "split side by side"),
    (Action::Split(SplitAxis::Rows), "split top and bottom"),
    (Action::Focus(Direction::Left), "move between panes"),
    (Action::NewTab, "new tab"),
    (Action::CommandPalette, "search every command"),
    (Action::Inbox, "agents that need you"),
    (Action::NextBlocked, "jump to a waiting agent"),
    (Action::NewWorktree, "new worktree and agent"),
    (Action::Help, "every key, and more"),
    (Action::Detach, "detach (keeps running)"),
];

/// What the welcome shows and when it opened. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WelcomeState {
    keys: Vec<(String, String)>,
    since: Instant,
}

impl WelcomeState {
    /// The welcome for `keymap`, keys shown after `prefix`, opened `now`.
    #[must_use]
    pub fn new(keymap: &Keymap, prefix: &str, now: Instant) -> Self {
        let entries = WhichKeyEntry::from_keymap(keymap);
        let keys = FIRST_KEYS
            .iter()
            .filter_map(|(wanted, text)| {
                let entry = entries.iter().find(|e| same(e.action, *wanted))?;
                // The first key of a family is enough to start with.
                let key = entry.keys.split(' ').next().unwrap_or(&entry.keys);
                Some((format!("{prefix} {key}"), (*text).to_owned()))
            })
            .collect();
        Self { keys, since: now }
    }

    /// `(keys, what they do)` for each bound action.
    #[must_use]
    pub fn keys(&self) -> &[(String, String)] {
        &self.keys
    }

    /// Animation frames since it opened.
    #[must_use]
    pub fn tick(&self, now: Instant) -> u32 {
        let steps =
            now.saturating_duration_since(self.since).as_millis() / WELCOME_STEP.as_millis();
        u32::try_from(steps).unwrap_or(u32::MAX)
    }
}

/// Focus moves count as one; everything else must match.
fn same(bound: Action, wanted: Action) -> bool {
    match (bound, wanted) {
        (Action::Focus(_), Action::Focus(_)) => true,
        _ => bound == wanted,
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::KeymapPreset;

    use super::*;

    #[test]
    fn keys_come_from_the_keymap_with_the_prefix() {
        let welcome = WelcomeState::new(&Keymap::default(), "ctrl+b", Instant::now());
        let keys = welcome.keys();
        assert_eq!(keys[0].0, "ctrl+b %");
        let focus = ("ctrl+b hjkl".to_owned(), "move between panes".to_owned());
        assert_eq!(keys[2], focus);
        assert!(keys.iter().any(|(k, _)| k == "ctrl+b ?"));
        assert_eq!(keys.len(), FIRST_KEYS.len());
    }

    #[test]
    fn unbound_actions_are_left_out_and_time_ticks() {
        let mut keymap = Keymap::preset(KeymapPreset::Herdr);
        keymap.bind("a", "none").expect("unbind");
        let now = Instant::now();
        let welcome = WelcomeState::new(&keymap, "ctrl+b", now);
        let texts: Vec<&str> = welcome.keys().iter().map(|(_, t)| t.as_str()).collect();
        assert!(!texts.contains(&"agents that need you"), "{texts:?}");
        assert_eq!(welcome.tick(now), 0);
        assert_eq!(welcome.tick(now + WELCOME_STEP * 3), 3);
        assert_eq!(welcome.tick(now - WELCOME_STEP), 0, "clocks never run back");
    }
}
