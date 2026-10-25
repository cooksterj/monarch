//! One line of the which-key popup: some keys and what they do.

use monarch_core::{Action, Direction, Key, Keymap};

/// Keys that share a meaning (`hjkl ←↓↑→` focus) and that meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyEntry {
    /// The keys, as drawn.
    pub keys: String,
    /// What they do.
    pub label: String,
}

/// Actions that share one popup line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// A single action, ranked so related actions sit together.
    One(u8, Action),
    Focus,
    Swap,
    Resize,
    Tab,
}

impl WhichKeyEntry {
    /// The keymap's bindings, grouped and in a stable reading order.
    #[must_use]
    pub fn from_keymap(keymap: &Keymap) -> Vec<Self> {
        let mut families: Vec<(Family, Vec<Key>)> = Vec::new();
        for (key, action) in keymap.bindings() {
            let family = family(action);
            match families.iter_mut().find(|(f, _)| *f == family) {
                Some((_, keys)) => keys.push(key),
                None => families.push((family, vec![key])),
            }
        }
        families.sort_by_key(|(f, _)| rank(*f));
        families
            .into_iter()
            .map(|(family, keys)| Self {
                keys: keys_text(family, keys),
                label: label(family),
            })
            .collect()
    }
}

fn family(action: Action) -> Family {
    let rank = match action {
        Action::Split(_) => 0,
        Action::ClosePane => 1,
        Action::ToggleZoom => 2,
        Action::FocusNext | Action::FocusPrev => 3,
        Action::SwapNext | Action::SwapPrev => 4,
        Action::Equalize => 5,
        Action::NewTab | Action::NextTab | Action::PrevTab => 6,
        Action::Detach => 8,
        _ => 7,
    };
    match action {
        Action::Focus(_) => Family::Focus,
        Action::Swap(_) => Family::Swap,
        Action::Resize(_) => Family::Resize,
        Action::SelectTab(_) => Family::Tab,
        other => Family::One(rank, other),
    }
}

/// Reading order: panes, then focus and layout, tabs, the rest, detach.
fn rank(family: Family) -> u8 {
    match family {
        Family::One(rank, _) => rank,
        Family::Focus => 3,
        Family::Swap => 4,
        Family::Resize => 5,
        Family::Tab => 6,
    }
}

fn label(family: Family) -> String {
    match family {
        // Short, so three columns still fit an 80-column host.
        Family::One(_, Action::CommandPalette) => "palette".into(),
        Family::One(_, Action::ToggleSidebar) => "sidebar".into(),
        Family::One(_, Action::ReloadConfig) => "reload".into(),
        Family::One(_, action) => action.to_string().replace('-', " "),
        Family::Focus => "focus".into(),
        Family::Swap => "swap".into(),
        Family::Resize => "resize".into(),
        Family::Tab => "go to tab".into(),
    }
}

/// `hjkl ←↓↑→` for directions, `1-9` for a run of tab digits, otherwise
/// the keys separated by spaces.
fn keys_text(family: Family, mut keys: Vec<Key>) -> String {
    match family {
        Family::Focus | Family::Swap | Family::Resize => {
            keys.sort_by_key(|k| match k {
                Key::Arrow(d) => (1, vi_order(*d)),
                _ => (0, 0),
            });
            let (chars, arrows): (Vec<Key>, Vec<Key>) =
                keys.into_iter().partition(|k| matches!(k, Key::Char(_)));
            let join = |ks: Vec<Key>| ks.into_iter().map(glyph).collect::<String>();
            [join(chars), join(arrows)]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
        Family::Tab if is_digit_run(&keys) => {
            format!("{}-{}", glyph(keys[0]), glyph(keys[keys.len() - 1]))
        }
        _ => keys.into_iter().map(glyph).collect::<Vec<_>>().join(" "),
    }
}

fn is_digit_run(keys: &[Key]) -> bool {
    keys.len() > 2
        && keys.windows(2).all(|w| match (w[0], w[1]) {
            (Key::Char(a), Key::Char(b)) => a.is_ascii_digit() && u32::from(b) == u32::from(a) + 1,
            _ => false,
        })
}

/// h j k l order, so arrows line up with the letters.
fn vi_order(d: Direction) -> u8 {
    match d {
        Direction::Left => 0,
        Direction::Down => 1,
        Direction::Up => 2,
        Direction::Right => 3,
    }
}

fn glyph(key: Key) -> String {
    match key {
        Key::Arrow(Direction::Left) => "←".into(),
        Key::Arrow(Direction::Down) => "↓".into(),
        Key::Arrow(Direction::Up) => "↑".into(),
        Key::Arrow(Direction::Right) => "→".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::KeymapPreset;

    use super::*;

    fn line(entries: &[WhichKeyEntry], label: &str) -> String {
        entries
            .iter()
            .find(|e| e.label == label)
            .map(|e| e.keys.clone())
            .unwrap_or_default()
    }

    #[test]
    fn families_collapse_into_one_line() {
        let entries = WhichKeyEntry::from_keymap(&Keymap::default());
        assert_eq!(line(&entries, "focus"), "hjkl ←↓↑→");
        assert_eq!(line(&entries, "resize"), "HJKL");
        assert_eq!(line(&entries, "go to tab"), "1-9");
        assert_eq!(line(&entries, "split right"), "% |");
        assert_eq!(entries[0].label, "split down", "splits come first");
    }

    #[test]
    fn tmux_tabs_count_from_zero_and_gaps_are_listed() {
        let tmux = WhichKeyEntry::from_keymap(&Keymap::preset(KeymapPreset::Tmux));
        assert_eq!(line(&tmux, "go to tab"), "0-8");
        let mut gappy = Keymap::default();
        gappy.bind("5", "none").expect("bind");
        let entries = WhichKeyEntry::from_keymap(&gappy);
        assert_eq!(line(&entries, "go to tab"), "1 2 3 4 6 7 8 9");
    }

    #[test]
    fn an_empty_keymap_has_no_entries() {
        let mut empty = Keymap::preset(KeymapPreset::Herdr);
        let keys: Vec<String> = empty.bindings().map(|(k, _)| k.to_string()).collect();
        for key in keys {
            empty.bind(&key, "none").expect("unbind");
        }
        assert!(WhichKeyEntry::from_keymap(&empty).is_empty());
    }
}
