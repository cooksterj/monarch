//! The client's model of the session, built only from server messages.

use std::collections::{BTreeMap, BTreeSet};

use monarch_core::PaneId;
use monarch_proto::{LayoutMsg, Screen, ServerMsg, StyleCache};

use super::{Applied, PaneReplica};

/// Layout, decoded pane screens, titles and the connection's style cache.
/// Pure data: applying a message never does I/O, it reports what to do in
/// an [`Applied`].
#[derive(Debug, Clone, Default)]
pub struct AttachState {
    styles: StyleCache,
    layout: Option<LayoutMsg>,
    panes: BTreeMap<PaneId, PaneReplica>,
    titles: BTreeMap<PaneId, String>,
    resyncing: BTreeSet<PaneId>,
}

impl AttachState {
    /// An empty model (nothing received yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one server message.
    pub fn apply(&mut self, msg: ServerMsg, out: &mut Applied) {
        match msg {
            ServerMsg::Layout(layout) => {
                self.panes
                    .retain(|id, _| layout.panes.iter().any(|p| p.pane == *id));
                self.layout = Some(layout);
                out.redraw = true;
            }
            ServerMsg::Snapshot(snapshot) => {
                self.resyncing.remove(&snapshot.pane);
                let replica = PaneReplica::from_snapshot(&snapshot, &self.styles);
                self.panes.insert(snapshot.pane, replica);
                out.redraw = true;
            }
            ServerMsg::Delta(delta) => match self.panes.get_mut(&delta.pane) {
                Some(replica) if replica.accepts(&delta) => {
                    replica.apply(&delta, &self.styles);
                    out.redraw = true;
                }
                _ if self.resyncing.insert(delta.pane) => out.resync.push(delta.pane),
                _ => {}
            },
            ServerMsg::DefineStyle(def) => self.styles.define(def),
            ServerMsg::ResetStyles => self.styles.clear(),
            ServerMsg::Title { pane, title } => {
                self.titles.insert(pane, title.unwrap_or_default());
                out.redraw = true;
            }
            ServerMsg::HostWrite(bytes) => out.host_write.extend(bytes),
            ServerMsg::PaneExited { pane } => {
                self.panes.remove(&pane);
                out.exited.push(pane);
                out.redraw = true;
            }
            _ => {}
        }
    }

    /// The current layout, once received.
    #[must_use]
    pub fn layout(&self) -> Option<&LayoutMsg> {
        self.layout.as_ref()
    }

    /// The focused pane.
    #[must_use]
    pub fn focus(&self) -> Option<PaneId> {
        self.layout.as_ref()?.focus
    }

    /// A pane's decoded screen.
    #[must_use]
    pub fn screen(&self, pane: PaneId) -> Option<&Screen> {
        self.panes.get(&pane).map(|r| &r.screen)
    }

    /// The focused pane's title, preferring the latest `Title` message.
    #[must_use]
    pub fn focused_title(&self) -> Option<&str> {
        let focus = self.focus()?;
        if let Some(title) = self.titles.get(&focus) {
            return Some(title.as_str());
        }
        self.layout.as_ref()?.focused()?.title.as_deref()
    }
}

#[cfg(test)]
mod tests;
