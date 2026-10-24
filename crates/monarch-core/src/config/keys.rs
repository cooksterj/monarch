//! `[keys]`: the prefix chord and which binding preset to start from.

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
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            prefix: "ctrl+b".into(),
            preset: "monarch".into(),
            repeat_ms: 500,
            which_key_delay_ms: 300,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_section_keeps_other_defaults() {
        let keys: KeysConfig = toml::from_str("prefix = \"ctrl+a\"").expect("parses");
        assert_eq!(keys.prefix, "ctrl+a");
        assert_eq!(keys.preset, "monarch");
    }
}
