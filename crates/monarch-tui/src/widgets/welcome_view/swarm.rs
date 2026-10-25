//! The welcome screen's swarm: small butterflies taking off from the
//! bottom at random intervals and fluttering up out of sight. Each is at
//! one of three depths; nearer ones are bigger, brighter, faster and sway
//! more, which with the card between the layers reads as depth.
//!
//! Where every butterfly is follows from the seed and the tick alone, so
//! frames are pure and tests can pin them.

use std::f32::consts::TAU;

/// Ticks already flown when the welcome opens, so it starts in flight.
const WARM_UP: u32 = 80;
/// Fewest and most ticks between take-offs over 80 columns; wider areas
/// launch proportionally more often.
const GAP: (u32, u32) = (2, 9);
/// The width `GAP` is for.
const BASE_WIDTH: f32 = 80.0;

/// How far away a butterfly is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Depth {
    /// A dim speck, slow, drawn behind the card.
    Far,
    /// Three cells wide, behind the card.
    Mid,
    /// Two rows tall, fast, in front of the card.
    Near,
}

impl Depth {
    /// Rows climbed per tick.
    fn speed(self) -> f32 {
        match self {
            Self::Far => 0.16,
            Self::Mid => 0.3,
            Self::Near => 0.55,
        }
    }

    /// How far it sways either side, in columns, and over how many ticks.
    fn sway(self) -> (f32, f32) {
        match self {
            Self::Far => (1.0, 44.0),
            Self::Mid => (2.0, 32.0),
            Self::Near => (3.5, 24.0),
        }
    }

    /// Ticks per wingbeat.
    fn beat(self) -> u32 {
        match self {
            Self::Far => 6,
            Self::Mid => 4,
            Self::Near => 3,
        }
    }

    /// How much its colour fades toward the background (0 is full).
    pub(super) fn fade(self) -> f32 {
        match self {
            Self::Far => 0.6,
            Self::Mid => 0.3,
            Self::Near => 0.0,
        }
    }

    /// Its rows, wings open or closed. Every row is as wide as the first.
    pub(super) fn sprite(self, open: bool) -> &'static [&'static str] {
        match (self, open) {
            (Self::Far, true) => &["v"],
            (Self::Far, false) => &["|"],
            (Self::Mid, true) => &["}|{"],
            (Self::Mid, false) => &[")|("],
            (Self::Near, true) => &["/\\|/\\", "\\/|\\/"],
            (Self::Near, false) => &[" \\|/ ", " /|\\ "],
        }
    }
}

/// One butterfly at one tick: its sprite's top-left cell (which may be off
/// the area) and how it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Flyer {
    pub x: i32,
    pub y: i32,
    pub depth: Depth,
    pub open: bool,
}

/// Every butterfly in sight at `tick` over a `width` × `height` area,
/// farthest first (the drawing order).
pub(super) fn flyers(seed: u64, tick: u32, width: u16, height: u16) -> Vec<Flyer> {
    let now = tick.saturating_add(WARM_UP) as f32;
    let scale = BASE_WIDTH / f32::from(width.max(1));
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    let mut born = 0.0;
    loop {
        born += (GAP.0 + rng.below(GAP.1 - GAP.0 + 1)) as f32 * scale;
        if born > now {
            break;
        }
        // Every draw happens for every butterfly, seen or not, so one
        // butterfly's draws never depend on whether another was seen.
        let x = rng.unit() * f32::from(width);
        let depth = match rng.below(10) {
            0..=4 => Depth::Far,
            5..=7 => Depth::Mid,
            _ => Depth::Near,
        };
        let phase = rng.unit();
        if let Some(flyer) = place(depth, x, phase, now - born, height) {
            out.push(flyer);
        }
    }
    out.sort_by_key(|f| f.depth);
    out
}

/// Where a butterfly `age` ticks after take-off is, if still in sight.
fn place(depth: Depth, x: f32, phase: f32, age: f32, height: u16) -> Option<Flyer> {
    let rows = depth.sprite(true);
    let tall = i32::try_from(rows.len()).unwrap_or(1);
    let wide = i32::try_from(rows[0].len()).unwrap_or(1);
    let y = round(f32::from(height) - age * depth.speed());
    if y + tall < 0 {
        return None;
    }
    let (reach, period) = depth.sway();
    let sway = ((age / period + phase) * TAU).sin() * reach;
    let beat = depth.beat();
    let wingbeat = u32::try_from(round(age + phase * beat as f32)).unwrap_or(0);
    Some(Flyer {
        x: round(x + sway) - wide / 2,
        y,
        depth,
        open: wingbeat % beat < beat.div_ceil(2),
    })
}

/// `v` to the nearest whole cell.
fn round(v: f32) -> i32 {
    // Cells are counted in the thousands at most.
    #[allow(clippy::cast_possible_truncation)]
    let cell = v.round() as i32;
    cell
}

/// SplitMix64: small, fast, and the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: u32) -> u32 {
        u32::try_from(self.next() % u64::from(n.max(1))).unwrap_or(0)
    }

    /// A number in `0.0..1.0`.
    fn unit(&mut self) -> f32 {
        f32::from(u16::try_from(self.next() >> 48).unwrap_or(0)) / 65536.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_and_tick_give_the_same_frame() {
        assert_eq!(flyers(7, 30, 80, 24), flyers(7, 30, 80, 24));
        assert_ne!(flyers(7, 30, 80, 24), flyers(8, 30, 80, 24), "seeds differ");
        assert_ne!(flyers(7, 30, 80, 24), flyers(7, 31, 80, 24), "they move");
    }

    #[test]
    fn it_starts_busy_and_stays_in_sight_far_first() {
        let swarm = flyers(1, 0, 80, 24);
        assert!(swarm.len() >= 6, "warmed up: {}", swarm.len());
        assert!(swarm.len() <= 40, "not a blizzard: {}", swarm.len());
        assert!(swarm.windows(2).all(|w| w[0].depth <= w[1].depth));
        assert!(swarm.iter().all(|f| f.y <= 24 && f.y >= -2));
        let wide = flyers(1, 0, 240, 24).len();
        assert!(wide > swarm.len() * 2, "wider is busier: {wide}");
        for depth in [Depth::Far, Depth::Mid, Depth::Near] {
            assert!(
                flyers(3, 200, 80, 24).iter().any(|f| f.depth == depth),
                "{depth:?}"
            );
        }
    }

    #[test]
    fn butterflies_climb_and_beat_their_wings() {
        let start = place(Depth::Near, 40.0, 0.0, 0.0, 24).expect("at the bottom");
        let later = place(Depth::Near, 40.0, 0.0, 10.0, 24).expect("climbing");
        assert!(later.y < start.y);
        let beats: Vec<bool> = (0..6u8)
            .map(|a| {
                place(Depth::Near, 40.0, 0.0, f32::from(a), 24)
                    .expect("seen")
                    .open
            })
            .collect();
        assert!(beats.contains(&true) && beats.contains(&false), "{beats:?}");
        assert_eq!(
            place(Depth::Far, 40.0, 0.0, 1000.0, 24),
            None,
            "gone off the top"
        );
    }

    #[test]
    fn sprites_are_rectangles() {
        for depth in [Depth::Far, Depth::Mid, Depth::Near] {
            for open in [true, false] {
                let rows = depth.sprite(open);
                assert!(rows.iter().all(|r| r.len() == rows[0].len()), "{depth:?}");
                assert_eq!(rows.len(), depth.sprite(!open).len());
            }
        }
    }
}
