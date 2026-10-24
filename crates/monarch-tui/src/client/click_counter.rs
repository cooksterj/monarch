//! Counts quick repeated clicks on the same cell (double, triple click).

use std::time::{Duration, Instant};

/// Clicks closer together than this count as one gesture.
pub const MULTI_CLICK: Duration = Duration::from_millis(400);

/// The last click and how many came in a row.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickCounter {
    last: Option<(Instant, u16, u16)>,
    count: u8,
}

impl ClickCounter {
    /// Registers a press at (`col`, `row`) and returns the click count.
    pub fn press(&mut self, col: u16, row: u16, now: Instant) -> u8 {
        let repeat = self
            .last
            .is_some_and(|(at, c, r)| (c, r) == (col, row) && now.duration_since(at) < MULTI_CLICK);
        self.count = if repeat { self.count % 3 + 1 } else { 1 };
        self.last = Some((now, col, row));
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_presses_on_one_cell_count_up_and_wrap() {
        let mut c = ClickCounter::default();
        let t = Instant::now();
        assert_eq!(c.press(1, 1, t), 1);
        assert_eq!(c.press(1, 1, t + Duration::from_millis(100)), 2);
        assert_eq!(c.press(1, 1, t + Duration::from_millis(200)), 3);
        assert_eq!(c.press(1, 1, t + Duration::from_millis(300)), 1);
    }

    #[test]
    fn slow_or_moved_presses_start_over() {
        let mut c = ClickCounter::default();
        let t = Instant::now();
        c.press(1, 1, t);
        assert_eq!(c.press(1, 1, t + MULTI_CLICK), 1, "too slow");
        assert_eq!(c.press(2, 1, t + MULTI_CLICK), 1, "moved");
    }
}
