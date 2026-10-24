//! Converts screen-model styles into ratatui styles.

use monarch_proto::{CellAttrs, CellStyle, TermColor};
use ratatui::style::{Color, Modifier, Style};

/// Screen attribute → ratatui modifier, declared once.
///
/// ratatui has a single underline modifier, so curly/dotted/dashed/double
/// underlines render as plain underlines for now (tracked in Progress as a
/// fidelity gap to close with a custom backend writer).
const MODIFIERS: [(CellAttrs, Modifier); 7] = [
    (CellAttrs::BOLD, Modifier::BOLD),
    (CellAttrs::DIM, Modifier::DIM),
    (CellAttrs::ITALIC, Modifier::ITALIC),
    (CellAttrs::ANY_UNDERLINE, Modifier::UNDERLINED),
    (CellAttrs::INVERSE, Modifier::REVERSED),
    (CellAttrs::HIDDEN, Modifier::HIDDEN),
    (CellAttrs::STRIKEOUT, Modifier::CROSSED_OUT),
];

/// Stateless style conversion.
pub struct StyleConverter;

impl StyleConverter {
    /// Converts a colour. Default and indexed colours stay symbolic so the
    /// host terminal's theme renders them exactly as outside Monarch.
    #[must_use]
    pub fn color(color: TermColor) -> Color {
        match color {
            TermColor::Default => Color::Reset,
            TermColor::Indexed(i) => Color::Indexed(i),
            TermColor::Rgb(c) => Color::Rgb(c.r, c.g, c.b),
        }
    }

    /// Converts a full cell style.
    #[must_use]
    pub fn style(style: &CellStyle) -> Style {
        let modifiers = MODIFIERS
            .iter()
            .filter(|(attr, _)| style.attrs.intersects(*attr))
            .fold(Modifier::empty(), |acc, (_, m)| acc | *m);
        let mut out = Style::new()
            .fg(Self::color(style.fg))
            .bg(Self::color(style.bg))
            .add_modifier(modifiers);
        if let Some(color) = style.underline {
            out = out.underline_color(Self::color(color));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monarch_proto::Rgb;

    #[test]
    fn colours_stay_symbolic() {
        assert_eq!(StyleConverter::color(TermColor::Default), Color::Reset);
        assert_eq!(
            StyleConverter::color(TermColor::Indexed(4)),
            Color::Indexed(4)
        );
        let rgb = TermColor::Rgb(Rgb::new(1, 2, 3));
        assert_eq!(StyleConverter::color(rgb), Color::Rgb(1, 2, 3));
    }

    #[test]
    fn every_underline_style_underlines() {
        let s = CellStyle {
            attrs: CellAttrs::UNDERCURL | CellAttrs::BOLD,
            ..CellStyle::default()
        };
        let out = StyleConverter::style(&s);
        assert!(
            out.add_modifier
                .contains(Modifier::UNDERLINED | Modifier::BOLD)
        );
        assert!(!out.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn plain_style_resets_colours() {
        let out = StyleConverter::style(&CellStyle::default());
        assert_eq!((out.fg, out.bg), (Some(Color::Reset), Some(Color::Reset)));
        assert!(out.add_modifier.is_empty());
    }
}
