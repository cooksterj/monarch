//! Scrolling a pane while a selection drag holds the pointer past its
//! edge, so one drag can select more than a screen.

use std::time::{Duration, Instant};

/// Time between scroll steps.
pub const DRAG_STEP: Duration = Duration::from_millis(50);

/// Most lines one step scrolls (the pointer far past the edge).
const MAX_LINES: i32 = 8;

/// Which way and how fast to scroll, and where the selection's head goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragScroll {
    beyond: i16,
    col: u16,
    next: Instant,
}

impl DragScroll {
    /// Scrolling for a drag `beyond` rows past the pane's top (negative)
    /// or bottom (positive) at pane column `col`; `None` inside the pane.
    /// A drag still past the same edge keeps `previous`'s timing, so moving
    /// the pointer never delays the next step.
    #[must_use]
    pub fn follow(previous: Option<Self>, beyond: i16, col: u16, now: Instant) -> Option<Self> {
        if beyond == 0 {
            return None;
        }
        let next = previous
            .filter(|p| p.beyond.signum() == beyond.signum())
            .map_or(now, |p| p.next);
        Some(Self { beyond, col, next })
    }

    /// Lines to scroll if a step is due (positive goes back into history),
    /// further past the edge scrolling faster.
    pub fn step(&mut self, now: Instant) -> Option<i32> {
        if now < self.next {
            return None;
        }
        self.next = now + DRAG_STEP;
        let lines = i32::from(self.beyond.unsigned_abs()).min(MAX_LINES);
        Some(if self.beyond < 0 { lines } else { -lines })
    }

    /// The pane cell the selection's head follows: the edge row it is
    /// past, in a pane of `rows` rows.
    #[must_use]
    pub fn head(&self, rows: u16) -> (u16, u16) {
        let row = if self.beyond < 0 {
            0
        } else {
            rows.saturating_sub(1)
        };
        (row, self.col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_on_a_timer_faster_further_out() {
        let t = Instant::now();
        let mut up = DragScroll::follow(None, -2, 4, t).expect("past the top");
        assert_eq!(up.step(t), Some(2), "the first step is immediate");
        assert_eq!(up.step(t + DRAG_STEP / 2), None, "not yet");
        assert_eq!(up.step(t + DRAG_STEP), Some(2));
        let mut far = DragScroll::follow(None, 40, 0, t).expect("past the bottom");
        assert_eq!(far.step(t), Some(-MAX_LINES), "capped, towards live output");
        assert_eq!((up.head(10), far.head(10)), ((0, 4), (9, 0)));
    }

    #[test]
    fn moving_keeps_the_timer_and_inside_stops() {
        let t = Instant::now();
        let mut first = DragScroll::follow(None, -1, 0, t).expect("past the top");
        first.step(t);
        let moved = DragScroll::follow(Some(first), -3, 5, t + DRAG_STEP / 2).expect("still");
        assert_eq!(moved.next, first.next, "same edge: same timing");
        let flipped = DragScroll::follow(Some(first), 1, 5, t).expect("other edge");
        assert_eq!(flipped.next, t, "other edge: steps at once");
        assert_eq!(
            DragScroll::follow(Some(first), 0, 5, t),
            None,
            "back inside"
        );
    }
}
