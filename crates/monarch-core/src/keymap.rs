//! What each key after the prefix does: a preset plus `[keys.bind]`.

mod key;
mod keymap_preset;

use std::collections::BTreeMap;

pub use key::Key;
pub use keymap_preset::KeymapPreset;

use crate::{Action, KeysConfig};

/// The value in `[keys.bind]` that removes a preset's binding.
const UNBIND: &str = "none";

/// Bindings from command key to action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: BTreeMap<Key, Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::preset(KeymapPreset::default())
    }
}

impl Keymap {
    /// A preset's bindings, unchanged.
    #[must_use]
    pub fn preset(preset: KeymapPreset) -> Self {
        Self {
            bindings: preset.bindings().into_iter().collect(),
        }
    }

    /// The keymap `[keys]` describes. Entries that cannot be read are
    /// skipped and described in the returned warnings, so a typo never
    /// leaves the user without keys.
    #[must_use]
    pub fn from_config(keys: &KeysConfig) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let preset = keys.preset.parse().unwrap_or_else(|e| {
            warnings.push(format!("[keys].preset: {e}; using monarch"));
            KeymapPreset::default()
        });
        let mut map = Self::preset(preset);
        for (key, action) in &keys.bind {
            if let Err(e) = map.bind(key, action) {
                warnings.push(format!("[keys.bind] {key:?}: {e}"));
            }
        }
        (map, warnings)
    }

    /// Binds `key` to `action` (both as written in config), or unbinds it
    /// when `action` is `none`.
    ///
    /// # Errors
    /// The key or action name is unknown.
    pub fn bind(&mut self, key: &str, action: &str) -> Result<(), crate::CoreError> {
        let key: Key = key.parse()?;
        if action.trim().eq_ignore_ascii_case(UNBIND) {
            self.bindings.remove(&key);
        } else {
            self.bindings.insert(key, action.parse()?);
        }
        Ok(())
    }

    /// What `key` does, if anything.
    #[must_use]
    pub fn action(&self, key: Key) -> Option<Action> {
        self.bindings.get(&key).copied()
    }

    /// The first key bound to `action`, for hints (`ctrl+b d to detach`).
    #[must_use]
    pub fn key_for(&self, action: Action) -> Option<Key> {
        self.bindings().find(|(_, a)| *a == action).map(|(k, _)| k)
    }

    /// Keys whose action repeats without the prefix (see
    /// [`Action::is_repeatable`]).
    pub fn repeatable_keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.bindings()
            .filter(|(_, a)| a.is_repeatable())
            .map(|(k, _)| k)
    }

    /// Every binding, ordered by key.
    pub fn bindings(&self) -> impl Iterator<Item = (Key, Action)> + '_ {
        self.bindings.iter().map(|(k, a)| (*k, *a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitAxis;

    fn config(preset: &str, bind: &[(&str, &str)]) -> KeysConfig {
        KeysConfig {
            preset: preset.into(),
            bind: bind
                .iter()
                .map(|(k, a)| ((*k).into(), (*a).into()))
                .collect(),
            ..KeysConfig::default()
        }
    }

    #[test]
    fn overrides_add_replace_and_remove_bindings() {
        let (map, warnings) = Keymap::from_config(&config(
            "tmux",
            &[("v", "split-right"), ("x", "none"), ("d", "zoom")],
        ));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            map.action(Key::Char('v')),
            Some(Action::Split(SplitAxis::Columns))
        );
        assert_eq!(map.action(Key::Char('x')), None);
        assert_eq!(map.action(Key::Char('d')), Some(Action::ToggleZoom));
        assert_eq!(
            map.action(Key::Char('%')),
            Some(Action::Split(SplitAxis::Columns))
        );
    }

    #[test]
    fn mistakes_are_warnings_and_everything_else_still_works() {
        let (map, warnings) =
            Keymap::from_config(&config("vim", &[("ctrl+x", "zoom"), ("y", "explode")]));
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings[0].contains("using monarch"));
        assert_eq!(map, Keymap::default());
    }

    #[test]
    fn repeatable_keys_follow_the_preset() {
        let monarch: Vec<Key> = Keymap::default().repeatable_keys().collect();
        assert!(monarch.contains(&Key::Char('H')) && monarch.contains(&Key::Char('}')));
        assert!(!monarch.contains(&Key::Char('h')), "focus does not repeat");
    }

    #[test]
    fn hints_find_the_key_for_an_action() {
        let herdr = Keymap::preset(KeymapPreset::Herdr);
        assert_eq!(herdr.key_for(Action::Detach), Some(Key::Char('q')));
        let (none, _) = Keymap::from_config(&config("monarch", &[("d", "none")]));
        assert_eq!(none.key_for(Action::Detach), None);
    }
}
