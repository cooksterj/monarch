//! The welcome screen's butterfly: four wingbeat frames, each drawn from
//! its left half mirrored, so the two sides always match.

/// Width of one half, the body column excluded.
const HALF: usize = 14;
/// Rows of one frame.
pub(super) const ROWS: usize = 8;
/// Columns of one frame.
pub(super) const WIDTH: usize = HALF * 2 + 1;

/// The body, top to bottom: the antennae meet at the head.
const BODY: &str = "  Y||||v";

/// Wings fully open.
const OPEN: [&str; ROWS] = [
    "   .-~~-.  \\ ",
    " .'  o   `-.\\",
    "/  .--.    `.",
    "\\  `--'  _.-'",
    " `-.   .'    ",
    "  .'   `-._  ",
    " (   o    `. ",
    "  `-.___.-'  ",
];

/// Wings half raised.
const HALF_UP: [&str; ROWS] = [
    "  .-~-.  \\ ",
    " / o   `.\\",
    "| .-.   `.",
    " \\`-' _.-'",
    "  `-.'    ",
    "  .'`-._  ",
    " (  o   `.",
    "  `-.__.' ",
];

/// Wings together above the back, seen edge on.
const FOLDED: [&str; ROWS] = [
    "  \\ ", "/` \\", "|  `.", "|  .'", " `.  ", " .'  ", "(  `.", " `-' ",
];

/// One wingbeat: open, rising, folded, falling.
const BEAT: [&[&str; ROWS]; 4] = [&OPEN, &HALF_UP, &FOLDED, &HALF_UP];

/// Frame `tick` of the flight: its rows, and how far it has risen (the
/// downstroke lifts it a row).
pub(super) fn frame(tick: u32) -> ([String; ROWS], u16) {
    let step = usize::try_from(tick).unwrap_or(0) % BEAT.len();
    let left = BEAT[step];
    let rows = std::array::from_fn(|i| {
        let half = format!("{:>HALF$}", left[i]);
        let body = BODY.chars().nth(i).unwrap_or(' ');
        let right: String = half.chars().rev().map(mirror).collect();
        format!("{half}{body}{right}")
    });
    (rows, u16::from(step == 2))
}

/// What a column is drawing, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    /// Wing outline and veins.
    Wing,
    /// The white spots.
    Spot,
    /// Head, body and antennae.
    Body,
}

/// The part the character `c` at column `col` of a row belongs to.
pub(super) fn part(c: char, col: usize) -> Part {
    match c {
        'o' => Part::Spot,
        _ if col.abs_diff(HALF) <= 1 && "\\/Y|'".contains(c) => Part::Body,
        _ => Part::Wing,
    }
}

/// The character facing the other way.
fn mirror(c: char) -> char {
    match c {
        '/' => '\\',
        '\\' => '/',
        '(' => ')',
        ')' => '(',
        '`' => '\'',
        '\'' => '`',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frame_is_symmetric_and_the_same_size() {
        for tick in 0..4 {
            let (rows, _) = frame(tick);
            for row in &rows {
                assert_eq!(row.chars().count(), WIDTH, "{row:?}");
                let back: String = row.chars().rev().map(mirror).collect();
                assert_eq!(&back, row, "mirror image");
            }
        }
    }

    #[test]
    fn the_wings_beat_and_the_downstroke_lifts() {
        let (open, lift) = frame(0);
        assert_eq!(lift, 0);
        assert_eq!(open[1], "  .'  o   `-.\\ /.-'   o  `.  ");
        let (folded, lift) = frame(2);
        assert_eq!(lift, 1);
        assert_ne!(open, folded);
        assert_eq!(frame(4).0, open, "the beat repeats");
    }

    #[test]
    fn spots_body_and_wings_are_told_apart() {
        assert_eq!(part('o', 6), Part::Spot);
        assert_eq!(part('|', HALF), Part::Body);
        assert_eq!(part('\\', HALF - 1), Part::Body, "antenna");
        assert_eq!(part('\\', 0), Part::Wing);
    }
}
