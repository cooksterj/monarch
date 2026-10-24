//! A pane's terminal: an [`Engine`] composed with an [`OscTap`].

use std::time::Instant;

use monarch_proto::{Screen, ScreenSize, TerminalModes};

use crate::{
    AlacrittyEngine, Damage, Engine, EngineEvent, FrameCapture, HostProfile, OscTap, ScrollRequest,
};

/// What the server holds for each pane. Owns the emulator and the OSC tap
/// and feeds both from the same bytes, so captured OSCs and engine events
/// come out in one ordered stream.
pub struct Terminal {
    engine: Box<dyn Engine>,
    tap: OscTap,
    osc_scratch: Vec<crate::Osc>,
}

impl Terminal {
    /// A terminal backed by any engine.
    #[must_use]
    pub fn with_engine(engine: Box<dyn Engine>) -> Self {
        Self {
            engine,
            tap: OscTap::new(),
            osc_scratch: Vec::new(),
        }
    }

    /// A terminal backed by the default engine.
    #[must_use]
    pub fn new(size: ScreenSize, scrollback: usize) -> Self {
        Self::with_engine(Box::new(AlacrittyEngine::new(size, scrollback)))
    }

    /// Feeds program output. Captured OSCs are reported before the engine's
    /// own events for the same chunk.
    pub fn feed(&mut self, bytes: &[u8], events: &mut Vec<EngineEvent>) {
        self.tap.scan(bytes, &mut self.osc_scratch);
        events.extend(self.osc_scratch.drain(..).map(EngineEvent::Osc));
        self.engine.feed(bytes, events);
    }

    /// See [`Engine::resize`].
    pub fn resize(&mut self, size: ScreenSize) {
        self.engine.resize(size);
    }

    /// See [`Engine::size`].
    #[must_use]
    pub fn size(&self) -> ScreenSize {
        self.engine.size()
    }

    /// See [`Engine::screen`].
    #[must_use]
    pub fn screen(&self) -> Screen {
        self.engine.screen()
    }

    /// See [`Engine::take_damage`].
    pub fn take_damage(&mut self) -> Damage {
        self.engine.take_damage()
    }

    /// Copies the damaged rows (all rows if `full`) and the pane state,
    /// clearing the damage. Call with the pane lock held; diff the result
    /// with [`crate::ShadowGrid::apply`] after releasing it.
    pub fn capture(&mut self, full: bool) -> FrameCapture {
        let size = self.engine.size();
        let damage = self.engine.take_damage();
        let (full, lines): (bool, Vec<u16>) = match (full, damage) {
            (false, Damage::Rows(rows)) => {
                (false, rows.into_iter().filter(|&l| l < size.rows).collect())
            }
            _ => (true, (0..size.rows).collect()),
        };
        let mut rows = Vec::with_capacity(lines.len());
        self.engine.rows(&lines, &mut rows);
        FrameCapture {
            size,
            full,
            lines,
            rows,
            state: self.engine.pane_state(),
        }
    }

    /// See [`Engine::modes`].
    #[must_use]
    pub fn modes(&self) -> TerminalModes {
        self.engine.modes()
    }

    /// See [`Engine::scroll`].
    pub fn scroll(&mut self, request: ScrollRequest) {
        self.engine.scroll(request);
    }

    /// See [`Engine::sync_deadline`].
    #[must_use]
    pub fn sync_deadline(&self) -> Option<Instant> {
        self.engine.sync_deadline()
    }

    /// See [`Engine::end_sync`].
    pub fn end_sync(&mut self, events: &mut Vec<EngineEvent>) {
        self.engine.end_sync(events);
    }

    /// See [`Engine::set_host_profile`].
    pub fn set_host_profile(&mut self, profile: HostProfile) {
        self.engine.set_host_profile(profile);
    }

    /// See [`Engine::text`].
    #[must_use]
    pub fn text(&self) -> String {
        self.engine.text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Osc;

    #[test]
    fn osc_and_engine_events_share_one_stream() {
        let mut t = Terminal::new(ScreenSize::new(20, 2), 10);
        let mut events = Vec::new();
        t.feed(b"\x1b]9;done\x07\x1b]2;x\x07hi", &mut events);
        assert_eq!(
            events,
            [
                EngineEvent::Osc(Osc {
                    number: 9,
                    payload: b"done".to_vec()
                }),
                EngineEvent::Title(Some("x".into())),
            ]
        );
        assert_eq!(t.text(), "hi\n");
    }

    #[test]
    fn delegates_resize() {
        let mut t = Terminal::new(ScreenSize::new(20, 2), 10);
        t.resize(ScreenSize::new(30, 5));
        assert_eq!(t.size(), ScreenSize::new(30, 5));
    }
}
