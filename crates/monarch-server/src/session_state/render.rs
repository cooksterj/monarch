//! Producing frames: capture visible dirty panes and encode per client.

use monarch_core::PaneId;
use monarch_proto::ServerMsg;

use super::SessionState;
use crate::FrameHeader;

impl SessionState {
    /// Renders one frame. For each visible pane that has new output or a
    /// client needing a snapshot: capture its damaged rows (pane lock held
    /// only for that), diff against the shadow grid, then queue a snapshot
    /// or delta for every client. Hidden panes are skipped entirely; their
    /// damage accumulates until they are shown.
    pub fn render_frame(&mut self) {
        let layout = self.layout();
        for client in self.clients.values_mut() {
            if !client.sees_layout(layout.generation) {
                client.queue([ServerMsg::Layout(layout.clone())]);
            }
        }
        for pane in self.visible_panes() {
            self.render_pane(pane);
        }
        self.clients
            .values_mut()
            .for_each(crate::ClientHandle::flush);
        self.recover_clients();
    }

    fn render_pane(&mut self, pane: PaneId) {
        let generation = self.generation;
        let Some(entry) = self.panes.get_mut(&pane) else {
            return;
        };
        let wants_snapshot = self.clients.values().any(|c| c.needs_snapshot(pane));
        if !entry.io.shared().take_dirty() && !wants_snapshot {
            return;
        }
        let capture = entry.io.shared().lock().capture(false);
        let change = entry.shadow.apply(capture);
        if change.is_some() {
            entry.seq += 1;
        }
        let header = FrameHeader {
            pane,
            generation,
            seq: entry.seq,
        };
        let resized = change.as_ref().is_some_and(|c| c.resized);
        for client in self.clients.values_mut() {
            let frame = if resized || client.needs_snapshot(pane) {
                client.mark_fresh(pane);
                client.encoder.snapshot(header, &entry.shadow)
            } else if let Some(change) = &change {
                client.encoder.delta(header, &entry.shadow, &change.lines)
            } else {
                continue;
            };
            client.queue(frame);
        }
    }

    /// Drops closed clients and schedules a full resync for any whose lane
    /// overflowed during this frame.
    fn recover_clients(&mut self) {
        self.clients.retain(|_, c| !c.is_closed());
        let visible = self.visible_panes();
        for client in self.clients.values_mut() {
            if client.take_overflow() {
                client.resync(visible.iter().copied());
            }
        }
    }
}
