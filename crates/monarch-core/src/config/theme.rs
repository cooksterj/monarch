//! `[theme]`: which colour theme to use.

use serde::{Deserialize, Serialize};

/// Theme selection. Built-in themes and custom tokens arrive in M6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Theme name used when `auto_switch` is off.
    pub name: String,
    /// Follow the host terminal's light/dark background (OSC 11 probe).
    pub auto_switch: bool,
    /// Theme for dark host backgrounds.
    pub dark: String,
    /// Theme for light host backgrounds.
    pub light: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "monarch-dark".into(),
            auto_switch: true,
            dark: "monarch-dark".into(),
            light: "monarch-light".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_switch_is_on_by_default() {
        assert!(ThemeConfig::default().auto_switch);
    }
}
