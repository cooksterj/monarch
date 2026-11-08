//! The style the host currently has set, so only changes are written.

use std::io::{self, Write};

use ratatui::backend::IntoCrossterm;
use ratatui::buffer::Cell;
use ratatui::crossterm::queue;
use ratatui::crossterm::style::{
    Attribute, Color as HostColor, Colors, SetAttribute, SetBackgroundColor, SetColors,
    SetForegroundColor, SetUnderlineColor,
};
use ratatui::style::{Color, Modifier};

/// Colours and attributes as last sent, starting from the host's defaults.
#[derive(Debug)]
pub(super) struct Pen {
    fg: Color,
    bg: Color,
    underline: Color,
    modifier: Modifier,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            fg: Color::Reset,
            bg: Color::Reset,
            underline: Color::Reset,
            modifier: Modifier::empty(),
        }
    }
}

impl Pen {
    /// Writes whatever `cell`'s style changes.
    pub(super) fn apply<W: Write>(&mut self, cell: &Cell, out: &mut W) -> io::Result<()> {
        if cell.modifier != self.modifier {
            modifier_change(self.modifier, cell.modifier, out)?;
            self.modifier = cell.modifier;
        }
        if cell.fg != self.fg || cell.bg != self.bg {
            let colors = Colors::new(cell.fg.into_crossterm(), cell.bg.into_crossterm());
            queue!(out, SetColors(colors))?;
            self.fg = cell.fg;
            self.bg = cell.bg;
        }
        if cell.underline_color != self.underline {
            queue!(
                out,
                SetUnderlineColor(cell.underline_color.into_crossterm())
            )?;
            self.underline = cell.underline_color;
        }
        Ok(())
    }

    /// Puts the host back on its defaults.
    pub(super) fn reset<W: Write>(self, out: &mut W) -> io::Result<()> {
        queue!(
            out,
            SetForegroundColor(HostColor::Reset),
            SetBackgroundColor(HostColor::Reset),
            SetUnderlineColor(HostColor::Reset),
            SetAttribute(Attribute::Reset),
        )
    }
}

/// Writes the attributes taking the host from `from` to `to`.
fn modifier_change<W: Write>(from: Modifier, to: Modifier, out: &mut W) -> io::Result<()> {
    for attribute in attribute_changes(from, to) {
        queue!(out, SetAttribute(attribute))?;
    }
    Ok(())
}

/// What switches a modifier off; the blink pair shares one.
const OFF: [(Modifier, Attribute); 5] = [
    (Modifier::ITALIC, Attribute::NoItalic),
    (Modifier::UNDERLINED, Attribute::NoUnderline),
    (Modifier::CROSSED_OUT, Attribute::NotCrossedOut),
    (Modifier::HIDDEN, Attribute::NoHidden),
    (
        Modifier::SLOW_BLINK.union(Modifier::RAPID_BLINK),
        Attribute::NoBlink,
    ),
];

/// What switches a modifier on; bold and dim are handled apart.
const ON: [(Modifier, Attribute); 7] = [
    (Modifier::REVERSED, Attribute::Reverse),
    (Modifier::ITALIC, Attribute::Italic),
    (Modifier::UNDERLINED, Attribute::Underlined),
    (Modifier::CROSSED_OUT, Attribute::CrossedOut),
    (Modifier::HIDDEN, Attribute::Hidden),
    (Modifier::SLOW_BLINK, Attribute::SlowBlink),
    (Modifier::RAPID_BLINK, Attribute::RapidBlink),
];

/// The attributes taking the host from `from` to `to`, the way crossterm's
/// backend orders them: there is no single "off" for bold or dim that
/// keeps the other, so both are re-sent after `NormalIntensity`.
fn attribute_changes(from: Modifier, to: Modifier) -> Vec<Attribute> {
    let removed = from - to;
    let added = to - from;
    let intensity_reset = removed.intersects(Modifier::BOLD | Modifier::DIM);
    let mut seq = Vec::new();
    if removed.contains(Modifier::REVERSED) {
        seq.push(Attribute::NoReverse);
    }
    if intensity_reset {
        seq.push(Attribute::NormalIntensity);
        seq.extend(intensity(to));
    }
    seq.extend(
        OFF.iter()
            .filter(|(m, _)| removed.intersects(*m))
            .map(|(_, a)| *a),
    );
    seq.extend(
        ON.iter()
            .filter(|(m, _)| added.contains(*m))
            .map(|(_, a)| *a),
    );
    if !intensity_reset {
        seq.extend(intensity(added));
    }
    seq
}

/// Dim and bold, whichever `modifier` has.
fn intensity(modifier: Modifier) -> impl Iterator<Item = Attribute> {
    [
        (Modifier::DIM, Attribute::Dim),
        (Modifier::BOLD, Attribute::Bold),
    ]
    .into_iter()
    .filter(move |(m, _)| modifier.contains(*m))
    .map(|(_, a)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_bold_is_one_attribute() {
        assert_eq!(
            attribute_changes(Modifier::empty(), Modifier::BOLD),
            vec![Attribute::Bold]
        );
    }

    #[test]
    fn dropping_dim_keeps_bold_by_resetting_intensity() {
        assert_eq!(
            attribute_changes(Modifier::BOLD | Modifier::DIM, Modifier::BOLD),
            vec![Attribute::NormalIntensity, Attribute::Bold]
        );
    }

    #[test]
    fn removed_and_added_attributes_both_appear() {
        let seq = attribute_changes(Modifier::REVERSED, Modifier::ITALIC | Modifier::UNDERLINED);
        assert_eq!(
            seq,
            vec![
                Attribute::NoReverse,
                Attribute::Italic,
                Attribute::Underlined
            ]
        );
    }

    #[test]
    fn same_style_writes_nothing() {
        let mut out = Vec::new();
        let mut pen = Pen::default();
        let cell = Cell::new("a");
        pen.apply(&cell, &mut out).expect("write");
        assert!(out.is_empty(), "{out:?}");
    }
}
