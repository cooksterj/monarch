//! The welcome screen's swarm: monarchs taking off from the bottom at
//! random intervals and fluttering up out of sight. Each is at one of
//! three depths; nearer ones are bigger, brighter, faster and sway more.
//! Far and middle ones pass behind the card; near ones keep to the sides
//! of it, so they never hide what it says.
//!
//! Where every butterfly is follows from the seed and the tick alone, so
//! frames are pure and tests can pin them.

use std::f32::consts::TAU;

use super::sprite::{self, Wings};

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
    /// A faded speck, slow, behind the card.
    Far,
    /// Eleven cells wide, behind the card.
    Mid,
    /// Big, bright and fast, beside the card.
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
            Self::Near => (3.0, 24.0),
        }
    }

    /// Ticks each frame of a wingbeat lasts.
    fn beat(self) -> f32 {
        match self {
            Self::Far => 3.0,
            Self::Mid => 2.0,
            Self::Near => 1.0,
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
}

/// One butterfly at one tick: its top-left cell (which may be off the
/// area) and how far its wings are spread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Flyer {
    pub x: i32,
    pub y: i32,
    pub depth: Depth,
    pub wings: Wings,
}

/// Every butterfly in sight at `tick` over a `width` × `height` area,
/// farthest first (the drawing order). Near ones keep out of the columns
/// `card` spans, when given; with no room beside it they fly further off.
pub(super) fn flyers(
    seed: u64,
    tick: u32,
    width: u16,
    height: u16,
    card: Option<(u16, u16)>,
) -> Vec<Flyer> {
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
        let column = rng.unit();
        let depth = match rng.below(10) {
            0..=4 => Depth::Far,
            5..=7 => Depth::Mid,
            _ => Depth::Near,
        };
        let phase = rng.unit();
        let (depth, x) = match (depth, card) {
            (Depth::Near, Some(card)) => beside(column, width, card)
                .map_or((Depth::Mid, column * f32::from(width)), |x| {
                    (Depth::Near, x)
                }),
            _ => (depth, column * f32::from(width)),
        };
        if let Some(flyer) = place(depth, x, phase, now - born, height) {
            out.push(flyer);
        }
    }
    out.sort_by_key(|f| f.depth);
    out
}

/// The column `at` (0–1) of the way across the room left and right of
/// `card`, where a near butterfly, swaying, never touches it.
fn beside(at: f32, width: u16, card: (u16, u16)) -> Option<f32> {
    let (wide, _) = sprite::cells(Depth::Near);
    let margin = (wide / 2) as f32 + Depth::Near.sway().0 + 1.0;
    let left = (f32::from(card.0) - 2.0 * margin).max(0.0);
    let right = (f32::from(width) - f32::from(card.1) - 2.0 * margin).max(0.0);
    let v = at * (left + right);
    if left + right < 1.0 {
        None
    } else if v < left {
        Some(margin + v)
    } else {
        Some(f32::from(card.1) + margin + v - left)
    }
}

/// Where a butterfly centred on column `x`, `age` ticks after take-off,
/// is, if still in sight.
fn place(depth: Depth, x: f32, phase: f32, age: f32, height: u16) -> Option<Flyer> {
    let (wide, tall) = sprite::cells(depth);
    let y = round(f32::from(height) - age * depth.speed());
    if y + tall < 0 {
        return None;
    }
    let (reach, period) = depth.sway();
    let sway = ((age / period + phase) * TAU).sin() * reach;
    let frame = round(age / depth.beat() + phase * 4.0);
    Some(Flyer {
        x: round(x + sway) - wide / 2,
        y,
        depth,
        wings: Wings::beat(u32::try_from(frame).unwrap_or(0)),
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

    fn sky(seed: u64, tick: u32, width: u16) -> Vec<Flyer> {
        flyers(seed, tick, width, 24, None)
    }

    #[test]
    fn the_same_seed_and_tick_give_the_same_frame() {
        assert_eq!(sky(7, 30, 80), sky(7, 30, 80));
        assert_ne!(sky(7, 30, 80), sky(8, 30, 80), "seeds differ");
        assert_ne!(sky(7, 30, 80), sky(7, 31, 80), "they move");
    }

    #[test]
    fn it_starts_busy_and_stays_in_sight_far_first() {
        let swarm = sky(1, 0, 80);
        assert!(swarm.len() >= 6, "warmed up: {}", swarm.len());
        assert!(swarm.len() <= 40, "not a blizzard: {}", swarm.len());
        assert!(swarm.windows(2).all(|w| w[0].depth <= w[1].depth));
        assert!(swarm.iter().all(|f| f.y <= 24 && f.y >= -7));
        let wide = sky(1, 0, 240).len();
        assert!(wide > swarm.len() * 2, "wider is busier: {wide}");
        for depth in [Depth::Far, Depth::Mid, Depth::Near] {
            assert!(
                sky(3, 200, 80).iter().any(|f| f.depth == depth),
                "{depth:?}"
            );
        }
    }

    #[test]
    fn near_butterflies_keep_beside_the_card() {
        let (wide, _) = sprite::cells(Depth::Near);
        let card = (60, 140);
        for tick in 0..300 {
            for f in flyers(5, tick, 200, 40, Some(card)) {
                if f.depth == Depth::Near {
                    let clear = f.x + wide <= 60 || f.x >= 140;
                    assert!(clear, "tick {tick}: {f:?}");
                }
            }
        }
        let squeezed = flyers(5, 100, 80, 24, Some((2, 78)));
        assert!(squeezed.iter().all(|f| f.depth != Depth::Near), "no room");
        assert!(squeezed.len() >= 6, "they fly further off instead");
    }

    #[test]
    fn butterflies_climb_and_beat_their_wings() {
        let start = place(Depth::Near, 40.0, 0.0, 0.0, 24).expect("at the bottom");
        let later = place(Depth::Near, 40.0, 0.0, 10.0, 24).expect("climbing");
        assert!(later.y < start.y);
        let beats: Vec<Wings> = (0..4u8)
            .map(|a| {
                place(Depth::Near, 40.0, 0.0, f32::from(a), 24)
                    .expect("seen")
                    .wings
            })
            .collect();
        assert_eq!(
            beats,
            [Wings::Open, Wings::Half, Wings::Closed, Wings::Half]
        );
        let gone = place(Depth::Far, 40.0, 0.0, 1000.0, 24);
        assert_eq!(gone, None, "gone off the top");
    }
}
