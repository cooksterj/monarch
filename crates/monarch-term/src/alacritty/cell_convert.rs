//! Converts alacritty cells and colours into the engine-neutral screen model.

use alacritty_terminal::term::cell::{Cell as AlacCell, Flags};
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use monarch_proto::{Cell, CellAttrs, CellStyle, Rgb, TermColor};

/// alacritty attribute flag → Monarch attribute, applied as a table so the
/// mapping is declared once.
const ATTRS: [(Flags, CellAttrs); 11] = [
    (Flags::BOLD, CellAttrs::BOLD),
    (Flags::DIM, CellAttrs::DIM),
    (Flags::ITALIC, CellAttrs::ITALIC),
    (Flags::UNDERLINE, CellAttrs::UNDERLINE),
    (Flags::DOUBLE_UNDERLINE, CellAttrs::DOUBLE_UNDERLINE),
    (Flags::UNDERCURL, CellAttrs::UNDERCURL),
    (Flags::DOTTED_UNDERLINE, CellAttrs::DOTTED_UNDERLINE),
    (Flags::DASHED_UNDERLINE, CellAttrs::DASHED_UNDERLINE),
    (Flags::INVERSE, CellAttrs::INVERSE),
    (Flags::HIDDEN, CellAttrs::HIDDEN),
    (Flags::STRIKEOUT, CellAttrs::STRIKEOUT),
];

/// Stateless conversion functions for alacritty grid content.
pub(crate) struct CellConverter;

impl CellConverter {
    /// Converts one cell.
    pub(crate) fn cell(cell: &AlacCell) -> Cell {
        let flags = cell.flags;
        let width = if flags.contains(Flags::WIDE_CHAR_SPACER) {
            0
        } else if flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };
        let ch = if flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            ' '
        } else {
            cell.c
        };
        Cell {
            ch,
            combining: cell.zerowidth().map(<[char]>::to_vec).unwrap_or_default(),
            width,
            style: Self::style(cell),
        }
    }

    fn style(cell: &AlacCell) -> CellStyle {
        let attrs = ATTRS
            .iter()
            .filter(|(f, _)| cell.flags.contains(*f))
            .fold(CellAttrs::empty(), |acc, (_, a)| acc | *a);
        CellStyle {
            fg: Self::color(cell.fg),
            bg: Self::color(cell.bg),
            underline: cell.underline_color().map(Self::color),
            attrs,
        }
    }

    /// Converts a colour. Named colours stay symbolic so the host terminal's
    /// own theme renders them.
    pub(crate) fn color(color: Color) -> TermColor {
        match color {
            Color::Spec(rgb) => TermColor::Rgb(Rgb::new(rgb.r, rgb.g, rgb.b)),
            Color::Indexed(i) => TermColor::Indexed(i),
            Color::Named(named) => Self::named(named),
        }
    }

    fn named(named: NamedColor) -> TermColor {
        let index = named as usize;
        let dim_base = NamedColor::DimBlack as usize;
        match index {
            0..=15 => TermColor::Indexed(index as u8),
            i if (dim_base..dim_base + 8).contains(&i) => TermColor::Indexed((i - dim_base) as u8),
            _ => TermColor::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::Rgb as AlacRgb;

    #[test]
    fn named_colours_map_to_palette_or_default() {
        assert_eq!(
            CellConverter::color(Color::Named(NamedColor::Red)),
            TermColor::Indexed(1)
        );
        assert_eq!(
            CellConverter::color(Color::Named(NamedColor::BrightWhite)),
            TermColor::Indexed(15)
        );
        assert_eq!(
            CellConverter::color(Color::Named(NamedColor::DimGreen)),
            TermColor::Indexed(2)
        );
        assert_eq!(
            CellConverter::color(Color::Named(NamedColor::Foreground)),
            TermColor::Default
        );
        assert_eq!(
            CellConverter::color(Color::Named(NamedColor::Background)),
            TermColor::Default
        );
    }

    #[test]
    fn spec_and_indexed_pass_through() {
        let rgb = Color::Spec(AlacRgb { r: 1, g: 2, b: 3 });
        assert_eq!(CellConverter::color(rgb), TermColor::Rgb(Rgb::new(1, 2, 3)));
        assert_eq!(
            CellConverter::color(Color::Indexed(200)),
            TermColor::Indexed(200)
        );
    }

    #[test]
    fn wide_and_spacer_widths() {
        let wide = AlacCell {
            c: '界',
            flags: Flags::WIDE_CHAR | Flags::BOLD,
            ..AlacCell::default()
        };
        let converted = CellConverter::cell(&wide);
        assert_eq!(converted.width, 2);
        assert_eq!(converted.style.attrs, CellAttrs::BOLD);

        let spacer = AlacCell {
            flags: Flags::WIDE_CHAR_SPACER,
            ..AlacCell::default()
        };
        assert_eq!(CellConverter::cell(&spacer).width, 0);
    }
}
