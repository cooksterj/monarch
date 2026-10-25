//! The keys the input thread must recognise itself: the prefix, the keys
//! that repeat without it and the keys that open a pop-up.

use std::time::Duration;

use monarch_core::{Action, Key};

use super::AttachOptions;
use crate::PrefixParser;

/// What the input thread needs from the keymap; sent again on reload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputKeys {
    /// The prefix control byte.
    pub prefix: u8,
    /// Keys that repeat without the prefix.
    pub repeat: Vec<Key>,
    /// How long after its last use a repeatable key keeps working.
    pub repeat_window: Duration,
    /// Keys that open a pop-up: what follows them goes to the main loop
    /// even before it has drawn the pop-up.
    pub overlay_keys: Vec<Key>,
}

impl InputKeys {
    /// A prefix parser for these keys.
    #[must_use]
    pub fn parser(&self) -> PrefixParser {
        PrefixParser::new(self.prefix).with_repeat(self.repeat.iter().copied(), self.repeat_window)
    }
}

impl From<&AttachOptions> for InputKeys {
    fn from(opts: &AttachOptions) -> Self {
        let pop_up = |a: &Action| {
            matches!(
                a,
                Action::CommandPalette
                    | Action::GoToPane
                    | Action::CopyMode
                    | Action::Inbox
                    | Action::Review
                    | Action::NewWorktree
                    | Action::Projects
                    | Action::Sessions
            )
        };
        Self {
            prefix: opts.prefix,
            repeat: opts.keymap.repeatable_keys().collect(),
            repeat_window: opts.repeat,
            overlay_keys: opts
                .keymap
                .bindings()
                .filter(|(_, a)| pop_up(a))
                .map(|(k, _)| k)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{Config, LoadedConfig};

    use super::*;

    #[test]
    fn taken_from_the_keymap() {
        let loaded = LoadedConfig {
            config: Config::default(),
            path: "/c.toml".into(),
            found: false,
            unknown_keys: Vec::new(),
        };
        let opts = AttachOptions::new("/s".into(), "m".into(), &loaded).expect("valid");
        let keys = InputKeys::from(&opts);
        assert_eq!(keys.prefix, 0x02);
        assert!(keys.overlay_keys.contains(&Key::Char(':')));
        assert!(keys.overlay_keys.contains(&Key::Char('[')));
        assert!(
            !keys.overlay_keys.contains(&Key::Char('d')),
            "detach opens nothing"
        );
        assert!(!keys.repeat.is_empty(), "resizes repeat");
        assert_eq!(InputKeys::default().overlay_keys, []);
    }
}
