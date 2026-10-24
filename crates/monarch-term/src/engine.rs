//! The contract every terminal emulator backend implements.

use std::time::Instant;

use monarch_proto::{Screen, ScreenSize, TerminalModes};

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

    /// Every visible row as plain text; used by detection and `pane.read`.
    /// Engines may override this with something cheaper than `screen()`.
    fn text(&self) -> String {
        self.screen().text()
    }
}
