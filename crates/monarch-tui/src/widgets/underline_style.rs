//! The kind of underline a cell has, carried through ratatui's buffer.
//!
//! ratatui has one `UNDERLINED` modifier. Its `Modifier` is a `u16` with
//! nine named bits, and every operation on cells and styles (`insert`,
//! `remove`, `==`) keeps the other seven, so the kind rides in three of
//! them as a qualifier on `UNDERLINED`: the buffer diff sees a change of
//! kind, and `HostBackend` writes it back out as the pane wrote it
//! (Terminal Fidelity Contract G1). Without `UNDERLINED` the bits mean
//! nothing, so `Style::reset()` (which only clears named bits) is harmless.

use monarch_proto::CellAttrs;
use ratatui::crossterm::style::Attribute;
use ratatui::style::Modifier;

/// Where the kind sits among the modifier's unnamed bits.
const SHIFT: u16 = 9;
/// The three bits holding the kind.
const MASK: u16 = 0b111 << SHIFT;

/// One of the five underline styles a pane can set (`SGR 4:1`–`4:5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlineStyle {
    /// `SGR 4` / `4:1`.
    Single,
    /// `SGR 4:2` (or `21`).
    Double,
    /// `SGR 4:3`.
    Curly,
    /// `SGR 4:4`.
    Dotted,
    /// `SGR 4:5`.
    Dashed,
}

/// Screen attribute → style, declared once; a plain underline is the
/// fallback when several are set.
const FROM_ATTRS: [(CellAttrs, UnderlineStyle); 4] = [
    (CellAttrs::DOUBLE_UNDERLINE, UnderlineStyle::Double),
    (CellAttrs::UNDERCURL, UnderlineStyle::Curly),
    (CellAttrs::DOTTED_UNDERLINE, UnderlineStyle::Dotted),
    (CellAttrs::DASHED_UNDERLINE, UnderlineStyle::Dashed),
];

impl UnderlineStyle {
    /// The underline `attrs` asks for, if any.
    #[must_use]
    pub fn from_attrs(attrs: CellAttrs) -> Option<Self> {
        if !attrs.intersects(CellAttrs::ANY_UNDERLINE) {
            return None;
        }
        let styled = FROM_ATTRS.iter().find(|(a, _)| attrs.contains(*a));
        Some(styled.map_or(Self::Single, |(_, s)| *s))
    }

    /// The underline a buffer cell's modifier carries, if any.
    #[must_use]
    pub fn of(modifier: Modifier) -> Option<Self> {
        if !modifier.contains(Modifier::UNDERLINED) {
            return None;
        }
        Some(match (modifier.bits() & MASK) >> SHIFT {
            1 => Self::Double,
            2 => Self::Curly,
            3 => Self::Dotted,
            4 => Self::Dashed,
            _ => Self::Single,
        })
    }

    /// The modifier that carries this underline: `UNDERLINED` plus the kind.
    #[must_use]
    pub fn modifier(self) -> Modifier {
        let kind: u16 = match self {
            Self::Single => 0,
            Self::Double => 1,
            Self::Curly => 2,
            Self::Dotted => 3,
            Self::Dashed => 4,
        };
        Modifier::UNDERLINED | Modifier::from_bits_retain(kind << SHIFT)
    }

    /// `modifier` with any kind bits cleared, leaving the named modifiers.
    #[must_use]
    pub fn strip(modifier: Modifier) -> Modifier {
        Modifier::from_bits_retain(modifier.bits() & !MASK)
    }

    /// The SGR attribute that sets this underline on the host.
    #[must_use]
    pub fn attribute(self) -> Attribute {
        match self {
            Self::Single => Attribute::Underlined,
            Self::Double => Attribute::DoubleUnderlined,
            Self::Curly => Attribute::Undercurled,
            Self::Dotted => Attribute::Underdotted,
            Self::Dashed => Attribute::Underdashed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [UnderlineStyle; 5] = [
        UnderlineStyle::Single,
        UnderlineStyle::Double,
        UnderlineStyle::Curly,
        UnderlineStyle::Dotted,
        UnderlineStyle::Dashed,
    ];

    #[test]
    fn every_style_round_trips_through_the_modifier() {
        for style in ALL {
            let m = style.modifier() | Modifier::BOLD;
            assert_eq!(UnderlineStyle::of(m), Some(style));
            assert_eq!(
                UnderlineStyle::strip(m),
                Modifier::UNDERLINED | Modifier::BOLD
            );
        }
    }

    #[test]
    fn kind_bits_stay_clear_of_named_modifiers() {
        assert!(!Modifier::all().intersects(Modifier::from_bits_retain(MASK)));
    }

    #[test]
    fn kind_bits_without_underlined_mean_no_underline() {
        let stale = UnderlineStyle::Curly.modifier() - Modifier::UNDERLINED;
        assert_eq!(UnderlineStyle::of(stale), None);
        assert_eq!(
            UnderlineStyle::of(Modifier::UNDERLINED),
            Some(UnderlineStyle::Single)
        );
    }

    #[test]
    fn attrs_pick_the_styled_underline() {
        assert_eq!(UnderlineStyle::from_attrs(CellAttrs::BOLD), None);
        assert_eq!(
            UnderlineStyle::from_attrs(CellAttrs::UNDERLINE),
            Some(UnderlineStyle::Single)
        );
        assert_eq!(
            UnderlineStyle::from_attrs(CellAttrs::UNDERCURL),
            Some(UnderlineStyle::Curly)
        );
        assert_eq!(
            UnderlineStyle::from_attrs(CellAttrs::UNDERLINE | CellAttrs::DASHED_UNDERLINE),
            Some(UnderlineStyle::Dashed)
        );
    }

    #[test]
    fn styles_map_to_their_sgr() {
        assert_eq!(UnderlineStyle::Curly.attribute().sgr(), "4:3");
        assert_eq!(UnderlineStyle::Double.attribute().sgr(), "4:2");
        assert_eq!(UnderlineStyle::Single.attribute().sgr(), "4");
    }
}
