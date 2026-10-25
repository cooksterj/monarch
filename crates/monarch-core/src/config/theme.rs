//! `[theme]`: which colour theme to use, and per-token overrides.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Rgb, THEME_TOKENS, Theme};

/// Theme selection: a built-in theme by name (or one per host background),
/// plus `[theme.colors]` overrides such as `accent = "#ff8800"`.
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
    /// Token overrides, `token = "#rrggbb"`, applied on top of the theme.
    pub colors: BTreeMap<String, String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "monarch-dark".into(),
            auto_switch: true,
            dark: "monarch-dark".into(),
            light: "monarch-light".into(),
            colors: BTreeMap::new(),
        }
    }
}

impl ThemeConfig {
    /// The theme to draw with on a host whose background is `background`,
    /// and a warning for each name or colour that could not be used. A bad
    /// value never fails: it falls back to the default for that host.
    #[must_use]
    pub fn resolve(&self, background: Option<Rgb>) -> (Theme, Vec<String>) {
        let mut warnings = Vec::new();
        let fallback = Theme::for_background(background);
        let name = match (self.auto_switch, background) {
            (false, _) => &self.name,
            (true, Some(bg)) if bg.is_light() => &self.light,
            (true, _) => &self.dark,
        };
        let mut theme = Theme::named(name).unwrap_or_else(|| {
            let known = Theme::names().collect::<Vec<_>>().join(", ");
            warnings.push(format!("theme: unknown theme {name:?} (known: {known})"));
            fallback
        });
        for (token, value) in &self.colors {
            let Some(slot) = theme.token_mut(token) else {
                let known = THEME_TOKENS.join(", ");
                warnings.push(format!(
                    "theme.colors: unknown token {token:?} (known: {known})"
                ));
                continue;
            };
            match value.parse() {
                Ok(rgb) => *slot = rgb,
                Err(e) => warnings.push(format!("theme.colors.{token}: {e}")),
            }
        }
        (theme, warnings)
    }

    /// Every problem with this config, whichever background the host has.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        let (_, mut warnings) = self.resolve(None);
        for warning in self.resolve(Some(Rgb::new(255, 255, 255))).1 {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Option<Rgb> = Some(Rgb::new(255, 255, 255));
    const BLACK: Option<Rgb> = Some(Rgb::new(0, 0, 0));

    #[test]
    fn auto_switch_follows_the_host_background() {
        let config = ThemeConfig::default();
        assert!(config.auto_switch);
        assert_eq!(config.resolve(WHITE), (Theme::light(), vec![]));
        assert_eq!(config.resolve(BLACK), (Theme::dark(), vec![]));
        assert_eq!(config.resolve(None), (Theme::dark(), vec![]));
    }

    #[test]
    fn a_fixed_name_ignores_the_background() {
        let config = ThemeConfig {
            auto_switch: false,
            name: "gruvbox-dark".into(),
            ..ThemeConfig::default()
        };
        let (theme, warnings) = config.resolve(WHITE);
        assert_eq!(Some(theme), Theme::named("gruvbox-dark"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn colours_override_single_tokens() {
        let mut config = ThemeConfig::default();
        config.colors.insert("accent".into(), "#ff8800".into());
        let (theme, _) = config.resolve(BLACK);
        assert_eq!(theme.accent, Rgb::new(0xff, 0x88, 0));
        assert_eq!(theme.bar, Theme::dark().bar);
    }

    #[test]
    fn bad_values_warn_and_fall_back() {
        let mut config = ThemeConfig {
            dark: "solarized".into(),
            ..ThemeConfig::default()
        };
        config.colors.insert("accent".into(), "orange".into());
        config.colors.insert("background".into(), "#000000".into());
        let (theme, warnings) = config.resolve(BLACK);
        assert_eq!(theme, Theme::dark());
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings[0].contains("\"solarized\""));
        assert!(warnings.iter().any(|w| w.contains("\"background\"")));
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("\"orange\" is not a #rrggbb"))
        );
        let all = ThemeConfig {
            light: "nope".into(),
            ..config
        }
        .warnings();
        assert_eq!(all.len(), 4, "both variants are checked once: {all:?}");
    }

    #[test]
    fn colors_parse_from_toml() {
        let config: ThemeConfig =
            toml::from_str("name = \"x\"\n[colors]\naccent = \"#010203\"").expect("parses");
        assert_eq!(config.colors["accent"], "#010203");
        assert!(config.auto_switch, "defaults fill the rest");
    }
}
