//! Colour tokens for Monarch's own chrome. Widgets take colours from
//! here, never literals. Built-in themes live in `builtin`; a config can
//! pick one and override single tokens.

mod builtin;

use crate::{AgentKind, Rgb};

/// The colours Monarch draws its chrome, agent states and diffs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// The status bar's background.
    pub bar: Rgb,
    /// Text on the status bar.
    pub bar_text: Rgb,
    /// The shown tab, the prefix indicator, the focused pane's border and
    /// sidebar headings.
    pub accent: Rgb,
    /// Text on an `accent` background.
    pub accent_text: Rgb,
    /// A working agent's pulsing dot, at its brightest.
    pub working: Rgb,
    /// A waiting (blocked) agent's diamond fades from this…
    pub blocked_from: Rgb,
    /// …to this and back.
    pub blocked_to: Rgb,
    /// A finished agent's check starts in this colour…
    pub done_from: Rgb,
    /// …and settles, bold, in this one.
    pub done_to: Rgb,
    /// A failed agent's cross.
    pub error: Rgb,
    /// An added line in a diff.
    pub added: Rgb,
    /// A removed line in a diff.
    pub removed: Rgb,
    /// A diff's hunk header (`@@ -1,3 +1,4 @@`).
    pub hunk: Rgb,
    /// Claude Code's icon.
    pub claude: Rgb,
    /// Codex's icon.
    pub codex: Rgb,
    /// The welcome's butterflies: their wings…
    pub wing: Rgb,
    /// …the veins, borders and body…
    pub wing_edge: Rgb,
    /// …and the spots along the borders.
    pub wing_spot: Rgb,
}

/// Every token's name, as `[theme.colors]` spells it.
pub const THEME_TOKENS: [&str; 18] = [
    "bar",
    "bar_text",
    "accent",
    "accent_text",
    "working",
    "blocked_from",
    "blocked_to",
    "done_from",
    "done_to",
    "error",
    "added",
    "removed",
    "hunk",
    "claude",
    "codex",
    "wing",
    "wing_edge",
    "wing_spot",
];

impl Theme {
    /// The default theme for dark host backgrounds (`monarch-dark`).
    #[must_use]
    pub const fn dark() -> Self {
        builtin::MONARCH_DARK
    }

    /// The default theme for light host backgrounds (`monarch-light`).
    #[must_use]
    pub const fn light() -> Self {
        builtin::MONARCH_LIGHT
    }

    /// The built-in theme called `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        builtin::THEMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, theme)| *theme)
    }

    /// Every built-in theme's name.
    pub fn names() -> impl Iterator<Item = &'static str> {
        builtin::THEMES.iter().map(|(name, _)| *name)
    }

    /// The default theme for a host whose background is `background`, if
    /// known.
    #[must_use]
    pub fn for_background(background: Option<Rgb>) -> Self {
        match background {
            Some(bg) if bg.is_light() => Self::light(),
            _ => Self::dark(),
        }
    }

    /// The colour of `agent`'s icon.
    #[must_use]
    pub fn agent(&self, agent: AgentKind) -> Rgb {
        match agent {
            AgentKind::Claude => self.claude,
            AgentKind::Codex => self.codex,
        }
    }

    /// The token called `name` (see [`THEME_TOKENS`]), to override.
    pub fn token_mut(&mut self, name: &str) -> Option<&mut Rgb> {
        Some(match name {
            "bar" => &mut self.bar,
            "bar_text" => &mut self.bar_text,
            "accent" => &mut self.accent,
            "accent_text" => &mut self.accent_text,
            "working" => &mut self.working,
            "blocked_from" => &mut self.blocked_from,
            "blocked_to" => &mut self.blocked_to,
            "done_from" => &mut self.done_from,
            "done_to" => &mut self.done_to,
            "error" => &mut self.error,
            "added" => &mut self.added,
            "removed" => &mut self.removed,
            "hunk" => &mut self.hunk,
            "claude" => &mut self.claude,
            "codex" => &mut self.codex,
            "wing" => &mut self.wing,
            "wing_edge" => &mut self.wing_edge,
            "wing_spot" => &mut self.wing_spot,
            _ => return None,
        })
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_backgrounds_get_the_light_theme() {
        assert_eq!(
            Theme::for_background(Some(Rgb::new(250, 250, 250))),
            Theme::light()
        );
        assert_eq!(
            Theme::for_background(Some(Rgb::new(10, 10, 10))),
            Theme::dark()
        );
        assert_eq!(Theme::for_background(None), Theme::default());
    }

    #[test]
    fn every_token_can_be_overridden_by_name() {
        let mut theme = Theme::dark();
        for name in THEME_TOKENS {
            *theme.token_mut(name).expect(name) = Rgb::new(1, 2, 3);
        }
        assert_eq!(theme.hunk, Rgb::new(1, 2, 3));
        assert_eq!(theme.bar, Rgb::new(1, 2, 3));
        assert!(theme.token_mut("background").is_none());
    }

    #[test]
    fn agents_take_their_own_icon_colours() {
        let theme = Theme::dark();
        assert_eq!(theme.agent(AgentKind::Claude), theme.claude);
        assert_eq!(theme.agent(AgentKind::Codex), theme.codex);
        assert_ne!(theme.claude, theme.codex);
        for name in Theme::names() {
            let t = Theme::named(name).expect(name);
            let dark = name.ends_with("dark") || name.ends_with("mocha") || name.ends_with("night");
            assert_eq!(
                t.codex.is_light(),
                dark,
                "{name}: codex reads on its background"
            );
        }
    }

    #[test]
    fn built_in_themes_are_found_by_name() {
        assert_eq!(Theme::named("monarch-dark"), Some(Theme::dark()));
        assert_eq!(Theme::named("monarch-light"), Some(Theme::light()));
        assert!(Theme::named("solarized").is_none());
        assert_eq!(Theme::names().count(), 8);
    }

    #[test]
    fn every_theme_is_readable() {
        for name in Theme::names() {
            let t = Theme::named(name).expect(name);
            let dark = name.ends_with("dark") || name.ends_with("mocha") || name.ends_with("night");
            assert_eq!(t.bar.is_light(), !dark, "{name}: bar suits its background");
            assert_ne!(
                t.bar_text.is_light(),
                t.bar.is_light(),
                "{name}: bar text contrasts"
            );
            assert_ne!(
                t.accent_text.is_light(),
                t.accent.is_light(),
                "{name}: accent text contrasts"
            );
            assert_ne!(t.blocked_from, t.blocked_to, "{name}: blocked pulses");
        }
    }
}
