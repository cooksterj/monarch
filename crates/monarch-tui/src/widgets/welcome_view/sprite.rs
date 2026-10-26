//! The butterflies' pixel art: a monarch seen from above at three sizes,
//! wings open, half closed or closed. Each terminal cell holds two square
//! pixels (a half block), so a row of cells is two rows of art.
//!
//! Art is stored as the left half, outer edge first, ending with the body
//! column; the right half is its mirror, so the sides always match.

use super::swarm::Depth;

/// How far the wings are spread, in the order a wingbeat cycles through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Wings {
    Open,
    Half,
    Closed,
}

impl Wings {
    /// The `n`th frame of a wingbeat: open, half, closed, half, …
    pub(super) fn beat(n: u32) -> Self {
        match n % 4 {
            0 => Self::Open,
            2 => Self::Closed,
            _ => Self::Half,
        }
    }
}

/// One pixel of a butterfly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pixel {
    /// Not part of it; whatever is behind shows.
    Clear,
    /// Orange wing.
    Wing,
    /// Black veins, borders, body and antennae.
    Edge,
    /// A white spot.
    Spot,
}

impl From<u8> for Pixel {
    fn from(c: u8) -> Self {
        match c {
            b'o' => Self::Wing,
            b'k' => Self::Edge,
            b'w' => Self::Spot,
            _ => Self::Clear,
        }
    }
}

const NEAR_OPEN: [&str; 14] = [
    "kkk....k..",
    "kwkkkk..k.",
    "wkkoooookk",
    "kooookookk",
    "wkoookoook",
    ".kkoookook",
    "..kkkooook",
    "...kkkkkkk",
    "...koookkk",
    "..koookook",
    "..wkookook",
    "...wkoookk",
    "....wkkkk.",
    "..........",
];
const NEAR_HALF: [&str; 14] = [
    ".....k.k..",
    ".....kk.k.",
    ".....wkkkk",
    ".....kookk",
    ".....kookk",
    "......kokk",
    "......kkok",
    "......kkkk",
    "......kokk",
    ".....kookk",
    ".....wkokk",
    "......wkkk",
    ".......kk.",
    "..........",
];
const NEAR_CLOSED: [&str; 14] = [
    ".......k..",
    "........k.",
    "........kk",
    "........ok",
    "........ok",
    "........ok",
    "........ok",
    "........kk",
    "........ok",
    "........ok",
    "........kk",
    ".........k",
    "..........",
    "..........",
];
const MID_OPEN: [&str; 6] = ["kkk.k.", "wkookk", "kookok", ".kkkkk", ".kookk", "..wkk."];
const MID_HALF: [&str; 6] = ["....k.", "..kook", "..oook", "...kkk", "...ook", "....k."];
const MID_CLOSED: [&str; 6] = ["....k.", "......", "....ok", "....kk", "....ok", "......"];
const FAR_OPEN: [&str; 2] = ["o.", ".o"];
const FAR_CLOSED: [&str; 2] = [".o", ".o"];

/// The left half of the art for `depth` with `wings`.
fn art(depth: Depth, wings: Wings) -> &'static [&'static str] {
    match (depth, wings) {
        (Depth::Far, Wings::Closed) => &FAR_CLOSED,
        (Depth::Far, _) => &FAR_OPEN,
        (Depth::Mid, Wings::Open) => &MID_OPEN,
        (Depth::Mid, Wings::Half) => &MID_HALF,
        (Depth::Mid, Wings::Closed) => &MID_CLOSED,
        (Depth::Near, Wings::Open) => &NEAR_OPEN,
        (Depth::Near, Wings::Half) => &NEAR_HALF,
        (Depth::Near, Wings::Closed) => &NEAR_CLOSED,
    }
}

/// A butterfly's size at `depth` in cells: `(width, height)`.
pub(super) fn cells(depth: Depth) -> (i32, i32) {
    let rows = art(depth, Wings::Open);
    let half = i32::try_from(rows[0].len()).unwrap_or(1);
    let tall = i32::try_from(rows.len()).unwrap_or(2);
    (half * 2 - 1, tall / 2)
}

/// The pixel at column `x`, art row `y` of a butterfly at `depth` with
/// `wings`; clear outside it.
pub(super) fn pixel(depth: Depth, wings: Wings, x: i32, y: i32) -> Pixel {
    let rows = art(depth, wings);
    let (width, _) = cells(depth);
    let half = (width + 1) / 2;
    let col = if x < half { x } else { width - 1 - x };
    let at = |i: i32| usize::try_from(i).ok();
    at(y)
        .and_then(|y| rows.get(y))
        .zip(at(col))
        .and_then(|(row, col)| row.as_bytes().get(col))
        .map_or(Pixel::Clear, |&c| Pixel::from(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Depth; 3] = [Depth::Far, Depth::Mid, Depth::Near];
    const WINGS: [Wings; 3] = [Wings::Open, Wings::Half, Wings::Closed];

    #[test]
    fn every_frame_is_a_rectangle_of_whole_cells() {
        for depth in ALL {
            let open = art(depth, Wings::Open);
            assert_eq!(open.len() % 2, 0, "{depth:?}: two pixels a cell");
            for wings in WINGS {
                let rows = art(depth, wings);
                assert_eq!(rows.len(), open.len(), "{depth:?} {wings:?}");
                assert!(rows.iter().all(|r| r.len() == open[0].len()));
                assert!(
                    rows.iter()
                        .flat_map(|r| r.bytes())
                        .all(|c| b".okw".contains(&c))
                );
            }
        }
        assert_eq!(cells(Depth::Near), (19, 7));
        assert_eq!(cells(Depth::Far), (3, 1));
    }

    #[test]
    fn wings_mirror_and_beat() {
        let (width, _) = cells(Depth::Near);
        for y in 0..14 {
            for x in 0..width {
                let mirrored = pixel(Depth::Near, Wings::Open, width - 1 - x, y);
                assert_eq!(pixel(Depth::Near, Wings::Open, x, y), mirrored);
            }
        }
        assert_eq!(
            pixel(Depth::Near, Wings::Open, 9, 3),
            Pixel::Edge,
            "the body"
        );
        assert_eq!(pixel(Depth::Near, Wings::Open, 1, 1), Pixel::Spot);
        assert_eq!(pixel(Depth::Near, Wings::Open, -1, 0), Pixel::Clear);
        assert_eq!(pixel(Depth::Near, Wings::Open, 0, 99), Pixel::Clear);
        let beat: Vec<Wings> = (0..5).map(Wings::beat).collect();
        assert_eq!(
            beat,
            [
                Wings::Open,
                Wings::Half,
                Wings::Closed,
                Wings::Half,
                Wings::Open
            ]
        );
    }
}
