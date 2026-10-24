//! The host terminal's colours, used to answer colour queries from panes.

use monarch_proto::Rgb;

/// Index of the default foreground in alacritty's colour table.
pub(crate) const FOREGROUND: usize = 256;
/// Index of the default background.
pub(crate) const BACKGROUND: usize = 257;
/// Index of the cursor colour.
pub(crate) const CURSOR: usize = 258;

/// Foreground, background, cursor and the 256-colour palette. Defaults to
/// xterm's until the client probes the real host values (OSC 4/10/11/12),
/// so programs such as Claude Code pick the right light/dark theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// Default foreground.
    pub foreground: Rgb,
    /// Default background.
    pub background: Rgb,
    /// Cursor colour.
    pub cursor: Rgb,
    /// The 256 indexed colours.
    pub indexed: [Rgb; 256],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            foreground: Rgb::new(0xe5, 0xe5, 0xe5),
            background: Rgb::new(0x00, 0x00, 0x00),
            cursor: Rgb::new(0xe5, 0xe5, 0xe5),
            indexed: std::array::from_fn(xterm_color),
        }
    }
}

impl Palette {
    /// The colour for an alacritty colour-table index (0–255 palette,
    /// 256 fg, 257 bg, 258 cursor; dim/bright variants fall back to fg).
    #[must_use]
    pub fn get(&self, index: usize) -> Rgb {
        match index {
            0..=255 => self.indexed[index],
            BACKGROUND => self.background,
            CURSOR => self.cursor,
            FOREGROUND => self.foreground,
            _ => self.foreground,
        }
    }
}

/// xterm's default 256-colour palette.
fn xterm_color(i: usize) -> Rgb {
    const ANSI: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x00, 0x00),
        (0x00, 0xcd, 0x00),
        (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee),
        (0xcd, 0x00, 0xcd),
        (0x00, 0xcd, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];
    let level = |v: usize| if v == 0 { 0 } else { (55 + v * 40) as u8 };
    match i {
        0..=15 => Rgb::new(ANSI[i].0, ANSI[i].1, ANSI[i].2),
        16..=231 => {
            let n = i - 16;
            Rgb::new(level(n / 36), level((n / 6) % 6), level(n % 6))
        }
        _ => {
            let g = (8 + (i - 232) * 10) as u8;
            Rgb::new(g, g, g)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_cube_and_greys() {
        let p = Palette::default();
        assert_eq!(p.indexed[16], Rgb::new(0, 0, 0));
        assert_eq!(p.indexed[196], Rgb::new(0xff, 0, 0));
        assert_eq!(p.indexed[231], Rgb::new(0xff, 0xff, 0xff));
        assert_eq!(p.indexed[232], Rgb::new(8, 8, 8));
        assert_eq!(p.indexed[255], Rgb::new(0xee, 0xee, 0xee));
    }

    #[test]
    fn special_indices() {
        let p = Palette {
            background: Rgb::new(1, 2, 3),
            ..Palette::default()
        };
        assert_eq!(p.get(BACKGROUND), Rgb::new(1, 2, 3));
        assert_eq!(p.get(FOREGROUND), p.foreground);
        assert_eq!(p.get(268), p.foreground);
    }
}
