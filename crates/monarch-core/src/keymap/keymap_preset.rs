//! The built-in binding sets a keymap starts from.

use std::str::FromStr;

use super::Key;
use crate::{Action, CoreError, Direction, SplitAxis};

/// A starting set of bindings (`[keys].preset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeymapPreset {
    /// tmux's keys where tmux has one, mnemonic ones otherwise.
    #[default]
    Monarch,
    /// tmux's defaults (tab `0` is the first, as with tmux windows).
    Tmux,
    /// Herdr's defaults.
    Herdr,
}

impl KeymapPreset {
    /// Every preset.
    pub const ALL: [Self; 3] = [Self::Monarch, Self::Tmux, Self::Herdr];

    /// The name used in config.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Monarch => "monarch",
            Self::Tmux => "tmux",
            Self::Herdr => "herdr",
        }
    }

    /// The preset's bindings.
    #[must_use]
    pub fn bindings(self) -> Vec<(Key, Action)> {
        let mut out = match self {
            Self::Monarch => monarch(),
            Self::Tmux => tmux(),
            Self::Herdr => herdr(),
        };
        let first_digit = if self == Self::Tmux { b'0' } else { b'1' };
        out.extend((1..=9).map(|n| {
            (
                Key::Char(char::from(first_digit + n - 1)),
                Action::SelectTab(n),
            )
        }));
        out
    }
}

impl FromStr for KeymapPreset {
    type Err = CoreError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| CoreError::UnknownPreset(text.to_owned()))
    }
}

fn ch(c: char) -> Key {
    Key::Char(c)
}

/// Arrow keys move focus in every preset.
fn arrows() -> impl Iterator<Item = (Key, Action)> {
    Direction::ALL
        .into_iter()
        .map(|d| (Key::Arrow(d), Action::Focus(d)))
}

/// `h j k l` (or `H J K L`) mapped through `make`.
fn vi(upper: bool, make: fn(Direction) -> Action) -> impl Iterator<Item = (Key, Action)> {
    let keys = if upper { "HJKL" } else { "hjkl" };
    let dirs = [
        Direction::Left,
        Direction::Down,
        Direction::Up,
        Direction::Right,
    ];
    keys.chars()
        .zip(dirs)
        .map(move |(c, d)| (Key::Char(c), make(d)))
}

fn monarch() -> Vec<(Key, Action)> {
    let mut out = vec![
        (ch('%'), Action::Split(SplitAxis::Columns)),
        (ch('|'), Action::Split(SplitAxis::Columns)),
        (ch('"'), Action::Split(SplitAxis::Rows)),
        (ch('-'), Action::Split(SplitAxis::Rows)),
        (ch('x'), Action::ClosePane),
        (ch('z'), Action::ToggleZoom),
        (ch('o'), Action::FocusNext),
        (ch('{'), Action::SwapPrev),
        (ch('}'), Action::SwapNext),
        (ch('='), Action::Equalize),
        (ch('c'), Action::NewTab),
        (ch('n'), Action::NextTab),
        (ch('p'), Action::PrevTab),
        (ch('d'), Action::Detach),
        (ch('m'), Action::ToggleMouse),
        (ch(':'), Action::CommandPalette),
        (ch('['), Action::CopyMode),
        (ch('t'), Action::ToggleSidebar),
        (ch('R'), Action::ReloadConfig),
        (ch('g'), Action::GoToPane),
        (ch('w'), Action::GoToPane),
        (ch('a'), Action::Inbox),
        (Key::Tab, Action::NextBlocked),
        (ch('v'), Action::Review),
        (ch('N'), Action::NewWorktree),
    ];
    out.extend(arrows());
    out.extend(vi(false, Action::Focus));
    out.extend(vi(true, Action::Resize));
    out
}

fn tmux() -> Vec<(Key, Action)> {
    let mut out = vec![
        (ch('%'), Action::Split(SplitAxis::Columns)),
        (ch('"'), Action::Split(SplitAxis::Rows)),
        (ch('x'), Action::ClosePane),
        (ch('z'), Action::ToggleZoom),
        (ch('o'), Action::FocusNext),
        (ch('{'), Action::SwapPrev),
        (ch('}'), Action::SwapNext),
        (ch('c'), Action::NewTab),
        (ch('n'), Action::NextTab),
        (ch('p'), Action::PrevTab),
        (ch('d'), Action::Detach),
        (ch('m'), Action::ToggleMouse),
        (ch(':'), Action::CommandPalette),
        (ch('['), Action::CopyMode),
        (ch('t'), Action::ToggleSidebar),
        (ch('R'), Action::ReloadConfig),
        (ch('w'), Action::GoToPane),
        (ch('a'), Action::Inbox),
        (Key::Tab, Action::NextBlocked),
        (ch('v'), Action::Review),
        (ch('N'), Action::NewWorktree),
    ];
    out.extend(arrows());
    out
}

fn herdr() -> Vec<(Key, Action)> {
    let mut out = vec![
        (ch('v'), Action::Split(SplitAxis::Columns)),
        (ch('-'), Action::Split(SplitAxis::Rows)),
        (ch('x'), Action::ClosePane),
        (ch('z'), Action::ToggleZoom),
        (ch('c'), Action::NewTab),
        (ch('n'), Action::NextTab),
        (ch('p'), Action::PrevTab),
        (ch('q'), Action::Detach),
        (ch('m'), Action::ToggleMouse),
        (ch(':'), Action::CommandPalette),
        (ch('['), Action::CopyMode),
        (ch('t'), Action::ToggleSidebar),
        (ch('R'), Action::ReloadConfig),
        (ch('g'), Action::GoToPane),
        (ch('a'), Action::Inbox),
        (Key::Tab, Action::NextBlocked),
        (ch('N'), Action::NewWorktree),
    ];
    out.extend(arrows());
    out.extend(vi(false, Action::Focus));
    out.extend(vi(true, Action::Swap));
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn presets_parse_by_name() {
        for preset in KeymapPreset::ALL {
            assert_eq!(preset.name().parse::<KeymapPreset>().ok(), Some(preset));
        }
        assert!("emacs".parse::<KeymapPreset>().is_err());
    }

    #[test]
    fn no_preset_binds_a_key_twice() {
        for preset in KeymapPreset::ALL {
            let bindings = preset.bindings();
            let keys: BTreeSet<_> = bindings.iter().map(|(k, _)| *k).collect();
            assert_eq!(keys.len(), bindings.len(), "{}", preset.name());
        }
    }

    #[test]
    fn presets_follow_their_namesakes() {
        let has = |p: KeymapPreset, k: Key, a: Action| p.bindings().contains(&(k, a));
        assert!(
            has(KeymapPreset::Tmux, ch('0'), Action::SelectTab(1)),
            "tmux counts from 0"
        );
        assert!(has(KeymapPreset::Monarch, ch('1'), Action::SelectTab(1)));
        assert!(has(KeymapPreset::Herdr, ch('q'), Action::Detach));
        assert!(has(KeymapPreset::Tmux, ch('w'), Action::GoToPane));
        for preset in KeymapPreset::ALL {
            assert!(has(preset, ch(':'), Action::CommandPalette));
            assert!(has(preset, ch('['), Action::CopyMode));
            assert!(has(preset, ch('t'), Action::ToggleSidebar));
            assert!(has(preset, ch('R'), Action::ReloadConfig));
        }
        assert!(has(
            KeymapPreset::Herdr,
            ch('H'),
            Action::Swap(Direction::Left)
        ));
        assert!(has(
            KeymapPreset::Monarch,
            ch('L'),
            Action::Resize(Direction::Right)
        ));
    }
}
