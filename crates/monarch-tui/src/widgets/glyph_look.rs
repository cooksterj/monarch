//! How one state glyph is drawn in a given frame.

use monarch_core::Rgb;

/// The symbol, colour and weight of an agent-state glyph right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphLook {
    /// The character drawn (one cell wide).
    pub symbol: char,
    /// Its colour; `None` keeps the host's colours (`NO_COLOR`, or a
    /// state without one).
    pub color: Option<Rgb>,
    /// Drawn bold.
    pub bold: bool,
}

impl GlyphLook {
    /// A glyph in the host's own colours.
    #[must_use]
    pub fn plain(symbol: char) -> Self {
        Self {
            symbol,
            color: None,
            bold: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_glyphs_have_no_colour_or_weight() {
        let look = GlyphLook::plain('$');
        assert_eq!((look.symbol, look.color, look.bold), ('$', None, false));
        assert_ne!(look, GlyphLook { bold: true, ..look });
    }
}
