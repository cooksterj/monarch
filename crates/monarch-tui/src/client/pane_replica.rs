//! The client's copy of one pane's screen, kept current from frames.

use monarch_proto::{Delta, PaneState, RowCodec, Screen, Snapshot, StyleCache};

/// A pane's screen as the client last decoded it, plus where it is in the
/// frame sequence so gaps can be detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneReplica {
    /// The decoded screen.
    pub screen: Screen,
    generation: u32,
    seq: u64,
}

impl PaneReplica {
    /// A replica rebuilt from a snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &Snapshot, styles: &StyleCache) -> Self {
        let mut screen = Screen::blank(snapshot.size);
        for (row, runs) in screen.rows.iter_mut().zip(&snapshot.rows) {
            *row = RowCodec::decode(runs, styles, snapshot.size.cols);
        }
        apply_state(&mut screen, snapshot.state);
        Self {
            screen,
            generation: snapshot.generation,
            seq: snapshot.seq,
        }
    }

    /// Whether `delta` continues exactly from this replica.
    #[must_use]
    pub fn accepts(&self, delta: &Delta) -> bool {
        delta.follows(self.generation, self.seq)
    }

    /// Applies a delta that [`Self::accepts`] approved.
    pub fn apply(&mut self, delta: &Delta, styles: &StyleCache) {
        let cols = self.screen.size.cols;
        for row in &delta.rows {
            if let Some(slot) = self.screen.rows.get_mut(usize::from(row.line)) {
                *slot = RowCodec::decode(&row.runs, styles, cols);
            }
        }
        apply_state(&mut self.screen, delta.state);
        self.seq = delta.seq;
    }
}

fn apply_state(screen: &mut Screen, state: PaneState) {
    screen.cursor = state.cursor;
    screen.modes = state.modes;
    screen.display_offset = state.display_offset;
    screen.history = state.history;
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{RowDelta, Run, ScreenSize, StyleId};

    use super::*;

    fn run(text: &str) -> Run {
        let cells = u16::try_from(text.chars().count()).expect("short");
        Run {
            style: StyleId::DEFAULT,
            width: 1,
            cells,
            text: text.into(),
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            pane: PaneId(1),
            generation: 2,
            seq: 5,
            size: ScreenSize::new(8, 2),
            rows: vec![vec![run("top")], vec![]],
            state: PaneState::default(),
        }
    }

    fn delta(seq: u64, text: &str) -> Delta {
        Delta {
            pane: PaneId(1),
            generation: 2,
            seq,
            rows: vec![RowDelta {
                line: 1,
                runs: vec![run(text)],
            }],
            state: PaneState::default(),
        }
    }

    #[test]
    fn snapshot_then_delta_builds_the_screen() {
        let styles = StyleCache::new();
        let mut replica = PaneReplica::from_snapshot(&snapshot(), &styles);
        assert_eq!(replica.screen.text(), "top\n");
        let next = delta(6, "bottom");
        assert!(replica.accepts(&next));
        replica.apply(&next, &styles);
        assert_eq!(replica.screen.text(), "top\nbottom");
    }

    #[test]
    fn gaps_and_stale_generations_are_refused() {
        let replica = PaneReplica::from_snapshot(&snapshot(), &StyleCache::new());
        assert!(!replica.accepts(&delta(7, "x")), "gap");
        let stale = Delta {
            generation: 1,
            ..delta(6, "x")
        };
        assert!(!replica.accepts(&stale));
    }
}
