//! The session's record of one attached client.

use std::collections::BTreeSet;
use std::time::Instant;

use monarch_core::{ClientId, PaneId};
use monarch_proto::{ScreenSize, ServerMsg};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;

use crate::FrameEncoder;

/// Frames queued for a client's socket writer before it counts as stalled.
pub const RENDER_LANE: usize = 64;

/// One attached client: its bounded render lane, its encoder (style ids)
/// and what it still needs to be sent. A client that stops reading fills
/// its lane; further frames are dropped and it is resynced with snapshots
/// once it drains, without slowing any pane or any other client.
#[derive(Debug)]
pub struct ClientHandle {
    id: ClientId,
    lane: Sender<Vec<ServerMsg>>,
    /// Encoder holding this connection's style ids.
    pub encoder: FrameEncoder,
    layout_generation: Option<u32>,
    stale: BTreeSet<PaneId>,
    pending: Vec<ServerMsg>,
    overflowed: bool,
    closed: bool,
    /// The window size it last reported, and when.
    window: Option<(Instant, ScreenSize)>,
}

impl ClientHandle {
    /// A client that has been sent nothing yet.
    #[must_use]
    pub fn new(id: ClientId, lane: Sender<Vec<ServerMsg>>) -> Self {
        Self {
            id,
            lane,
            encoder: FrameEncoder::new(),
            layout_generation: None,
            stale: BTreeSet::new(),
            pending: Vec::new(),
            overflowed: false,
            closed: false,
            window: None,
        }
    }

    /// Records the window size it reported.
    pub fn set_window(&mut self, size: ScreenSize) {
        self.window = Some((Instant::now(), size));
    }

    /// The window size it last reported, and when.
    #[must_use]
    pub fn window(&self) -> Option<(Instant, ScreenSize)> {
        self.window
    }

    /// Its id.
    #[must_use]
    pub fn id(&self) -> ClientId {
        self.id
    }

    /// Whether `pane` must be sent as a snapshot rather than a delta.
    #[must_use]
    pub fn needs_snapshot(&self, pane: PaneId) -> bool {
        self.stale.contains(&pane)
    }

    /// Requests a snapshot of `pane` on the next frame.
    pub fn mark_stale(&mut self, pane: PaneId) {
        self.stale.insert(pane);
    }

    /// Records that `pane` was just snapshotted.
    pub fn mark_fresh(&mut self, pane: PaneId) {
        self.stale.remove(&pane);
    }

    /// Starts the client over: new layout, fresh styles, snapshots of `panes`.
    pub fn resync(&mut self, panes: impl IntoIterator<Item = PaneId>) {
        self.stale.extend(panes);
        self.encoder.reset();
        self.layout_generation = None;
    }

    /// Whether the client has seen layout `generation`; records it if not.
    pub fn sees_layout(&mut self, generation: u32) -> bool {
        let seen = self.layout_generation == Some(generation);
        self.layout_generation = Some(generation);
        seen
    }

    /// Queues a frame without waiting. A full lane marks the client as
    /// overflowed and drops the frame.
    pub fn push(&mut self, frame: Vec<ServerMsg>) {
        if self.closed || frame.is_empty() {
            return;
        }
        match self.lane.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.overflowed = true,
            Err(TrySendError::Closed(_)) => self.closed = true,
        }
    }

    /// Adds messages to the frame being built (see [`Self::flush`]).
    pub fn queue(&mut self, msgs: impl IntoIterator<Item = ServerMsg>) {
        self.pending.extend(msgs);
    }

    /// Sends the frame being built as one lane entry, so a frame is
    /// delivered or dropped whole.
    pub fn flush(&mut self) {
        let frame = std::mem::take(&mut self.pending);
        self.push(frame);
    }

    /// Whether a frame was dropped since the last call.
    pub fn take_overflow(&mut self) -> bool {
        std::mem::take(&mut self.overflowed)
    }

    /// Whether the client's writer has gone away.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn a_full_lane_drops_frames_and_flags_overflow() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut client = ClientHandle::new(ClientId(1), tx);
        client.push(vec![ServerMsg::Pong(1)]);
        client.push(vec![ServerMsg::Pong(2)]);
        assert!(client.take_overflow());
        assert!(!client.take_overflow());
        assert_eq!(rx.try_recv().expect("first frame"), [ServerMsg::Pong(1)]);
        assert!(rx.try_recv().is_err(), "second frame was dropped");
    }

    #[test]
    fn queued_messages_travel_as_one_frame() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut client = ClientHandle::new(ClientId(1), tx);
        client.queue([ServerMsg::Pong(1)]);
        client.queue([ServerMsg::Pong(2)]);
        client.flush();
        client.flush();
        assert!(!client.take_overflow(), "empty flush sends nothing");
        assert_eq!(
            rx.try_recv().expect("frame"),
            [ServerMsg::Pong(1), ServerMsg::Pong(2)]
        );
    }

    #[test]
    fn a_dropped_receiver_closes_the_client() {
        let (tx, rx) = mpsc::channel(1);
        let mut client = ClientHandle::new(ClientId(1), tx);
        drop(rx);
        client.push(vec![ServerMsg::Pong(1)]);
        assert!(client.is_closed());
    }

    #[test]
    fn resync_requests_layout_and_snapshots() {
        let (tx, _rx) = mpsc::channel(1);
        let mut client = ClientHandle::new(ClientId(1), tx);
        assert!(!client.sees_layout(1));
        assert!(client.sees_layout(1));
        client.resync([PaneId(4)]);
        assert!(client.needs_snapshot(PaneId(4)));
        assert!(!client.sees_layout(1), "layout resent after resync");
        client.mark_fresh(PaneId(4));
        assert!(!client.needs_snapshot(PaneId(4)));
    }
}
