//! `[keys]`: the prefix chord and which binding preset to start from.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Keyboard configuration. Individual bindings are layered on top of the
/// chosen preset by the keymap (M2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeysConfig {
    /// The prefix chord, e.g. `"ctrl+b"`. Must be a single control byte so
    /// it can be recognised without an escape timeout.
    pub prefix: String,
    /// Binding preset: `"monarch"`, `"tmux"` or `"herdr"`.
    pub preset: String,
    /// Milliseconds during which repeatable keys work without the prefix.
    pub repeat_ms: u64,
    /// Milliseconds after the prefix before the which-key popup appears.
    pub which_key_delay_ms: u64,
    /// Bindings layered on the preset: key → action name, or `"none"` to
    /// remove one (`"v" = "split-right"`).
    pub bind: BTreeMap<String, String>,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            prefix: "ctrl+b".into(),
            preset: "monarch".into(),
            repeat_ms: 500,
            which_key_delay_ms: 300,
            bind: BTreeMap::new(),
        }
    }
}

impl KeysConfig {
    /// The control byte the prefix produces (`"ctrl+b"` → `0x02`), or
    /// `None` if `prefix` is not a single control chord. Accepts
    /// `ctrl+<letter>`, `ctrl+space`, `ctrl+@`, `ctrl+[`, `ctrl+\`,
    /// `ctrl+]`, `ctrl+^` and `ctrl+_`, case-insensitively.
    #[must_use]
    pub fn prefix_byte(&self) -> Option<u8> {
        let lower = self.prefix.trim().to_ascii_lowercase();
        let key = lower
            .strip_prefix("ctrl+")
            .or_else(|| lower.strip_prefix("c-"))?;
        match key.as_bytes() {
            b"space" | b"@" => Some(0x00),
            [c @ b'a'..=b'z'] => Some(c - b'a' + 1),
            [c @ (b'[' | b'\\' | b']' | b'^' | b'_')] => Some(c & 0x1f),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix(p: &str) -> Option<u8> {
        KeysConfig {
            prefix: p.into(),
            ..KeysConfig::default()
        }
        .prefix_byte()
    }

    #[test]
    fn prefix_chords_map_to_control_bytes() {
        assert_eq!(KeysConfig::default().prefix_byte(), Some(0x02));
        assert_eq!(prefix("Ctrl+A"), Some(0x01));
        assert_eq!(prefix("C-space"), Some(0x00));
        assert_eq!(prefix("ctrl+]"), Some(0x1d));
    }

    #[test]
    fn non_control_prefixes_are_rejected() {
        for bad in ["b", "alt+b", "ctrl+1", "ctrl+bb", ""] {
            assert_eq!(prefix(bad), None, "{bad}");
        }
    }

    #[test]
    fn partial_section_keeps_other_defaults() {
        let keys: KeysConfig = toml::from_str("prefix = \"ctrl+a\"").expect("parses");
        assert_eq!(keys.prefix, "ctrl+a");
        assert_eq!(keys.preset, "monarch");
    }
}
