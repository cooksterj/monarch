//! A session's panes, clients and layout: plain state behind one mutex.
//!
//! Everything here is synchronous and never awaits, so the server's tasks
//! hold the lock only briefly. Panes are reached through [`PaneIo`], which
//! keeps this logic testable without PTYs.

mod render;

use std::collections::BTreeMap;

use monarch_core::{ClientId, PaneId};
use monarch_proto::{HostInfo, LayoutMsg, PaneLayout, ScreenSize, ServerMsg};
use monarch_term::{HostProfile, ShadowGrid};
use tokio::sync::mpsc::Sender;

use crate::{ClientHandle, PaneIo};

/// Window size assumed until a client reports its own.
pub const DEFAULT_SIZE: ScreenSize = ScreenSize { cols: 80, rows: 24 };

/// One pane plus what has been published for it.
struct PaneEntry {
    io: Box<dyn PaneIo>,
    shadow: ShadowGrid,
    seq: u64,
    title: Option<String>,
}

/// The session: panes, focus, window size, host facts and clients.
pub struct SessionState {
    panes: BTreeMap<PaneId, PaneEntry>,
    focus: Option<PaneId>,
    size: ScreenSize,
    generation: u32,
    host: HostInfo,
    clients: BTreeMap<ClientId, ClientHandle>,
    next_client: ClientId,
    next_pane: PaneId,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new(DEFAULT_SIZE)
    }
}

impl SessionState {
    /// An empty session for a window of `size`.
    #[must_use]
    pub fn new(size: ScreenSize) -> Self {
        Self {
            panes: BTreeMap::new(),
            focus: None,
            size,
            generation: 0,
            host: HostInfo::default(),
            clients: BTreeMap::new(),
            next_client: ClientId(1),
            next_pane: PaneId(1),
        }
    }

    /// Reserves the id for a pane about to be spawned.
    pub fn allocate_pane_id(&mut self) -> PaneId {
        let id = self.next_pane;
        self.next_pane = id.next();
        id
    }

    /// Adds a spawned pane; the first pane takes focus.
    pub fn insert_pane(&mut self, id: PaneId, io: Box<dyn PaneIo>) {
        io.shared().lock().set_host_profile(self.host_profile());
        io.resize(self.size);
        let shadow = ShadowGrid::new(self.size);
        self.panes.insert(
            id,
            PaneEntry {
                io,
                shadow,
                seq: 0,
                title: None,
            },
        );
        if self.focus.is_none() {
            self.set_focus(id);
        }
    }

    /// Removes a pane (its program exited). Returns `true` if none remain.
    pub fn remove_pane(&mut self, id: PaneId) -> bool {
        self.panes.remove(&id);
        if self.focus == Some(id) {
            self.focus = None;
            if let Some(&next) = self.panes.keys().next() {
                self.set_focus(next);
            } else {
                self.generation += 1;
            }
        }
        self.panes.is_empty()
    }

    /// A pane's I/O, if it exists.
    #[must_use]
    pub fn pane(&self, id: PaneId) -> Option<&dyn PaneIo> {
        self.panes.get(&id).map(|e| e.io.as_ref())
    }

    /// Ids of every pane, ascending.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.panes.keys().copied().collect()
    }

    /// The focused pane.
    #[must_use]
    pub fn focus(&self) -> Option<PaneId> {
        self.focus
    }

    /// Focuses `id`; clients get it as a snapshot. Unknown panes are ignored.
    pub fn set_focus(&mut self, id: PaneId) -> bool {
        if !self.panes.contains_key(&id) {
            return false;
        }
        if self.focus != Some(id) {
            self.focus = Some(id);
            self.generation += 1;
            self.clients.values_mut().for_each(|c| c.mark_stale(id));
        }
        true
    }

    /// Current window size.
    #[must_use]
    pub fn size(&self) -> ScreenSize {
        self.size
    }

    /// Resizes every pane to the window (latest client wins).
    pub fn resize(&mut self, size: ScreenSize) {
        if size == self.size {
            return;
        }
        self.size = size;
        self.generation += 1;
        for entry in self.panes.values() {
            entry.io.resize(size);
            entry.io.shared().mark_dirty();
        }
    }

    /// Records the host facts and applies them to every pane.
    pub fn set_host(&mut self, host: HostInfo) {
        self.host = host;
        let profile = self.host_profile();
        for entry in self.panes.values() {
            entry.io.shared().lock().set_host_profile(profile.clone());
        }
    }

    /// The host profile panes answer queries with.
    #[must_use]
    pub fn host_profile(&self) -> HostProfile {
        HostProfile::from(&self.host)
    }

    /// Records a pane's title and tells every client.
    pub fn set_title(&mut self, id: PaneId, title: Option<String>) {
        if let Some(entry) = self.panes.get_mut(&id) {
            entry.title.clone_from(&title);
            self.broadcast(&ServerMsg::Title { pane: id, title });
        }
    }

    /// Registers a client; it gets the layout and snapshots on the next frame.
    pub fn add_client(&mut self, lane: Sender<Vec<ServerMsg>>) -> ClientId {
        let id = self.next_client;
        self.next_client = id.next();
        let mut client = ClientHandle::new(id, lane);
        client.resync(self.visible_panes());
        self.clients.insert(id, client);
        id
    }

    /// Forgets a client.
    pub fn remove_client(&mut self, id: ClientId) {
        self.clients.remove(&id);
    }

    /// A client, if still attached.
    pub fn client_mut(&mut self, id: ClientId) -> Option<&mut ClientHandle> {
        self.clients.get_mut(&id)
    }

    /// Number of attached clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Sends one message to every client.
    pub fn broadcast(&mut self, msg: &ServerMsg) {
        self.clients
            .values_mut()
            .for_each(|c| c.push(vec![msg.clone()]));
    }

    /// Panes clients currently see (the focused pane fills the window until
    /// splits arrive in M2; every other pane keeps parsing but costs no
    /// rendering).
    #[must_use]
    pub fn visible_panes(&self) -> Vec<PaneId> {
        self.focus.into_iter().collect()
    }

    /// The layout message for the current state.
    #[must_use]
    pub fn layout(&self) -> LayoutMsg {
        let panes = self
            .visible_panes()
            .into_iter()
            .filter_map(|id| {
                let entry = self.panes.get(&id)?;
                Some(PaneLayout {
                    pane: id,
                    x: 0,
                    y: 0,
                    size: self.size,
                    title: entry.title.clone(),
                })
            })
            .collect();
        LayoutMsg {
            generation: self.generation,
            size: self.size,
            focus: self.focus,
            panes,
        }
    }
}

#[cfg(test)]
mod tests;
