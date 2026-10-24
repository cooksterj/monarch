//! How Monarch captures the host terminal's mouse.

use serde::{Deserialize, Serialize};

/// Mouse capture policy (see the Terminal Fidelity Contract, G3/G10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseMode {
    /// Always capture; Shift+drag falls through to native selection.
    #[default]
    Auto,
    /// Capture only while the focused pane requests mouse reporting.
    Pane,
    /// Never capture; the host terminal owns the mouse.
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Wrapper {
        mouse: MouseMode,
    }

    #[test]
    fn parses_snake_case() {
        let w: Wrapper = toml::from_str("mouse = \"pane\"").expect("parses");
        assert_eq!(w.mouse, MouseMode::Pane);
    }

    #[test]
    fn rejects_unknown_value() {
        assert!(toml::from_str::<Wrapper>("mouse = \"sometimes\"").is_err());
    }
}
