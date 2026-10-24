//! A cell colour as the program inside the pane specified it.

use serde::{Deserialize, Serialize};

use super::Rgb;

/// A foreground, background or underline colour. Indexed and default
/// colours are resolved by the *host* terminal, so the user's theme applies
/// inside panes exactly as it does outside Monarch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TermColor {
    /// The host terminal's default foreground/background.
    #[default]
    Default,
    /// One of the 256 palette entries (0–15 are the ANSI colours).
    Indexed(u8),
    /// A 24-bit colour.
    Rgb(Rgb),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_default() {
        assert_eq!(TermColor::default(), TermColor::Default);
        assert_ne!(TermColor::Indexed(0), TermColor::Default);
    }
}
