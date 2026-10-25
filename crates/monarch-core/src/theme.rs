//! Colour tokens for Monarch's own chrome. Widgets take colours from here,
//! never literals; built-in themes and user overrides arrive in M6.

use crate::Rgb;

/// The colours agent states and diffs are drawn in, for dark or light
/// hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
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
}

impl Theme {
    /// For dark host backgrounds.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            working: Rgb::new(0x5f, 0xaf, 0xff),
            blocked_from: Rgb::new(0xc8, 0xa0, 0x00),
            blocked_to: Rgb::new(0x7b, 0x4f, 0x1d),
            done_from: Rgb::new(0xa8, 0xe6, 0xa3),
            done_to: Rgb::new(0x2e, 0xe6, 0x5b),
            error: Rgb::new(0xff, 0x5f, 0x5f),
            added: Rgb::new(0x7e, 0xd3, 0x7e),
            removed: Rgb::new(0xf0, 0x71, 0x78),
            hunk: Rgb::new(0x6f, 0xb3, 0xd2),
        }
    }

    /// For light host backgrounds, darker so it stays readable.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            working: Rgb::new(0x1f, 0x6f, 0xd1),
            blocked_from: Rgb::new(0xa6, 0x7c, 0x00),
            blocked_to: Rgb::new(0x5c, 0x3a, 0x12),
            done_from: Rgb::new(0x6c, 0xbf, 0x6a),
            done_to: Rgb::new(0x0a, 0x8a, 0x2a),
            error: Rgb::new(0xc6, 0x28, 0x28),
            added: Rgb::new(0x1a, 0x7f, 0x37),
            removed: Rgb::new(0xb3, 0x1d, 0x28),
            hunk: Rgb::new(0x05, 0x50, 0xae),
        }
    }

    /// The theme for a host whose background is `background`, if known.
    #[must_use]
    pub fn for_background(background: Option<Rgb>) -> Self {
        match background {
            Some(bg) if bg.is_light() => Self::light(),
            _ => Self::dark(),
        }
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
    fn done_settles_brighter_than_it_starts_on_dark() {
        let t = Theme::dark();
        assert!(t.done_to.g >= t.done_from.g);
        assert_ne!(t.blocked_from, t.blocked_to);
    }
}
