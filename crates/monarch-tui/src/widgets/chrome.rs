//! The styles Monarch's own chrome is drawn in: theme colours when there
//! is a theme, reverse video and bold otherwise (`NO_COLOR`), so it still
//! reads in the host's own colours.

use monarch_core::{Rgb, Theme};
use monarch_proto::BadgePart;
use ratatui::style::{Color, Modifier, Style};

/// Chrome styles from an optional theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chrome<'a> {
    /// The colours; `None` under `NO_COLOR`.
    pub theme: Option<&'a Theme>,
}

impl<'a> Chrome<'a> {
    /// Chrome in `theme`'s colours, or in modifiers only.
    #[must_use]
    pub const fn new(theme: Option<&'a Theme>) -> Self {
        Self { theme }
    }

    /// A theme colour as a ratatui colour.
    #[must_use]
    pub const fn color(rgb: Rgb) -> Color {
        Color::Rgb(rgb.r, rgb.g, rgb.b)
    }

    /// The status bar.
    #[must_use]
    pub fn bar(&self) -> Style {
        match self.theme {
            Some(t) => Style::new()
                .fg(Self::color(t.bar_text))
                .bg(Self::color(t.bar)),
            None => Style::new().add_modifier(Modifier::REVERSED),
        }
    }

    /// What stands out on the bar: the shown tab, the waiting prefix.
    #[must_use]
    pub fn accent(&self) -> Style {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        match self.theme {
            Some(t) => bold
                .fg(Self::color(t.accent_text))
                .bg(Self::color(t.accent)),
            None => bold.remove_modifier(Modifier::REVERSED),
        }
    }

    /// One part of a checkout's badge: lines added green, removed red,
    /// untracked files dim, commits to push in the accent, commits to pull
    /// amber. Without colours, commits to pull are bold.
    #[must_use]
    pub fn badge(&self, part: BadgePart) -> Style {
        let dim = Style::new().add_modifier(Modifier::DIM);
        let Some(t) = self.theme else {
            return match part {
                BadgePart::Untracked => dim,
                BadgePart::Behind => Style::new().add_modifier(Modifier::BOLD),
                BadgePart::Added | BadgePart::Removed | BadgePart::Ahead => Style::new(),
            };
        };
        match part {
            BadgePart::Added => Style::new().fg(Self::color(t.added)),
            BadgePart::Removed => Style::new().fg(Self::color(t.removed)),
            BadgePart::Untracked => dim,
            BadgePart::Ahead => Style::new().fg(Self::color(t.accent)),
            BadgePart::Behind => Style::new().fg(Self::color(t.blocked_from)),
        }
    }

    /// Lines next to the focused pane, and sidebar headings.
    #[must_use]
    pub fn highlight(&self) -> Style {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        match self.theme {
            Some(t) => bold.fg(Self::color(t.accent)),
            None => bold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_gives_colours() {
        let theme = Theme::dark();
        let chrome = Chrome::new(Some(&theme));
        assert_eq!(chrome.bar().bg, Some(Chrome::color(theme.bar)));
        assert_eq!(chrome.accent().bg, Some(Chrome::color(theme.accent)));
        assert_eq!(chrome.highlight().fg, Some(Chrome::color(theme.accent)));
    }

    #[test]
    fn badge_parts_take_their_tokens() {
        let theme = Theme::dark();
        let chrome = Chrome::new(Some(&theme));
        assert_eq!(
            chrome.badge(BadgePart::Added).fg,
            Some(Chrome::color(theme.added))
        );
        assert_eq!(
            chrome.badge(BadgePart::Removed).fg,
            Some(Chrome::color(theme.removed))
        );
        assert_eq!(
            chrome.badge(BadgePart::Ahead).fg,
            Some(Chrome::color(theme.accent))
        );
        let behind = Chrome::color(theme.blocked_from);
        assert_eq!(chrome.badge(BadgePart::Behind).fg, Some(behind));
        let plain = Chrome::default();
        assert!(
            plain
                .badge(BadgePart::Untracked)
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert!(
            plain
                .badge(BadgePart::Behind)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(plain.badge(BadgePart::Added).fg, None);
    }

    #[test]
    fn no_theme_falls_back_to_modifiers() {
        let chrome = Chrome::default();
        assert_eq!(chrome.bar(), Style::new().add_modifier(Modifier::REVERSED));
        assert!(chrome.accent().sub_modifier.contains(Modifier::REVERSED));
        assert_eq!(chrome.highlight().fg, None);
    }
}
