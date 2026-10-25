//! The contract every terminal emulator backend implements.

use std::time::Instant;

use monarch_proto::{Cell, GridPoint, PaneState, Screen, ScreenSize, SelectShape, TerminalModes};

use crate::{Damage, EngineEvent, HostProfile, ScrollRequest};

/// A headless VT emulator for one pane.
///
/// Implementations must be cheap to `feed` (this is the per-byte hot path)
/// and must never block: replies to terminal queries are returned as
/// [`EngineEvent::Reply`] for the caller to write to the PTY in order.
pub trait Engine: Send {
    /// Parses program output, appending any resulting events to `events`.
    fn feed(&mut self, bytes: &[u8], events: &mut Vec<EngineEvent>);

    /// Resizes the grid, reflowing wrapped lines. Call before resizing the PTY.
    fn resize(&mut self, size: ScreenSize);

    /// Current grid size.
    fn size(&self) -> ScreenSize;

    /// The visible viewport.
    fn screen(&self) -> Screen;

    /// Copies the given viewport rows (each `size().cols` cells wide) into
    /// `out`, in order. Rows past the bottom come back blank. Engines should
    /// override this: the coalescer calls it under the pane lock with only
    /// the damaged rows, so it must not build the whole screen.
    fn rows(&self, lines: &[u16], out: &mut Vec<Vec<Cell>>) {
        let screen = self.screen();
        let blank = vec![Cell::default(); usize::from(screen.size.cols)];
        out.extend(lines.iter().map(|&l| {
            screen
                .rows
                .get(usize::from(l))
                .cloned()
                .unwrap_or_else(|| blank.clone())
        }));
    }

    /// Cursor, modes and scroll position, without the cells.
    fn pane_state(&self) -> PaneState {
        let screen = self.screen();
        PaneState {
            cursor: screen.cursor,
            modes: screen.modes,
            display_offset: screen.display_offset,
            history: screen.history,
        }
    }

    /// Viewport rows changed since the last call, then clears the record.
    fn take_damage(&mut self) -> Damage;

    /// Modes the program has enabled, for mirroring onto the host terminal.
    fn modes(&self) -> TerminalModes;

    /// Scrolls the viewport through history.
    fn scroll(&mut self, request: ScrollRequest);

    /// When a synchronized update (DEC 2026) begun by the program must be
    /// force-ended if it has not finished; `None` when none is pending.
    fn sync_deadline(&self) -> Option<Instant>;

    /// Force-ends a pending synchronized update (the program died mid-frame
    /// or is too slow), flushing its buffered output.
    fn end_sync(&mut self, events: &mut Vec<EngineEvent>);

    /// Updates the host colours and cell size used to answer queries.
    fn set_host_profile(&mut self, profile: HostProfile);

    /// The live screen (the bottom of the buffer, wherever the viewport
    /// is scrolled) as plain text, one line per row; used by detection.
    /// Engines with scrollback must override this.
    fn text(&self) -> String {
        self.screen().text()
    }

    /// The text between two grid points, scrollback included, in `shape`;
    /// lines the terminal wrapped come back joined. Engines without
    /// scrollback access return nothing.
    fn text_range(&mut self, _start: GridPoint, _end: GridPoint, _shape: SelectShape) -> String {
        String::new()
    }

    /// The next match for plain-text `query` after `from` (before it, if
    /// `backward`), wrapping around; its first and last cell.
    fn search(
        &self,
        _query: &str,
        _from: GridPoint,
        _backward: bool,
    ) -> Option<(GridPoint, GridPoint)> {
        None
    }
}
