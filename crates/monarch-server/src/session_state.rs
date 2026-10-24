//! A session's tabs, panes and clients: plain state behind one mutex.
//!
//! Everything here is synchronous and never awaits, so the server's tasks
//! hold the lock only briefly. Panes are reached through [`PaneIo`], which
//! keeps this logic testable without PTYs. Arranging panes lives in
//! `arrange`, session commands in `actions`, frames in `render`.

mod actions;
mod arrange;
mod commands;
mod render;

use std::collections::BTreeMap;

use monarch_core::{ClientId, PaneId, Rect, Tab};
use monarch_proto::{
    HostInfo, LayoutMsg, PaneLayout, PaneSummary, ScreenSize, ServerMsg, TabSummary,
};
use monarch_term::{HostProfile, ShadowGrid};
use tokio::sync::mpsc::Sender;

pub use actions::ActionEffect;

use crate::{ClientHandle, PaneIo, Placement};

/// Window size assumed until a client reports its own.
pub const DEFAULT_SIZE: ScreenSize = ScreenSize { cols: 80, rows: 24 };

/// One pane plus what has been published for it.
struct PaneEntry {
    io: Box<dyn PaneIo>,
    shadow: ShadowGrid,
    seq: u64,
    title: Option<String>,
    /// The foreground program's name.
    command: Option<String>,
    /// The size the pane was last given.
    size: ScreenSize,
}

/// The session: tabs of panes, window size, host facts and clients.
pub struct SessionState {
    panes: BTreeMap<PaneId, PaneEntry>,
    tabs: Vec<Tab>,
    active: usize,
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
            tabs: Vec::new(),
            active: 0,
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

    /// Adds a spawned pane in a background tab (the first one is shown).
    pub fn insert_pane(&mut self, id: PaneId, io: Box<dyn PaneIo>) {
        self.insert_pane_at(id, io, Placement::BACKGROUND);
    }

    /// Adds a spawned pane where `placement` says.
    pub fn insert_pane_at(&mut self, id: PaneId, io: Box<dyn PaneIo>, placement: Placement) {
        io.shared().lock().set_host_profile(self.host_profile());
        let entry = PaneEntry {
            io,
            shadow: ShadowGrid::new(self.size),
            seq: 0,
            title: None,
            command: None,
            size: ScreenSize::new(0, 0),
        };
        self.panes.insert(id, entry);
        let before = self.visible_panes();
        let first = self.tabs.is_empty();
        match (placement, self.tabs.get_mut(self.active)) {
            (Placement::Split(axis), Some(tab)) => tab.split(id, axis),
            (Placement::Tab { activate }, _) if activate || first => {
                self.tabs.push(Tab::new(id));
                self.active = self.tabs.len() - 1;
            }
            _ => self.tabs.push(Tab::new(id)),
        }
        self.arrange(&before);
    }

    /// Removes a pane (its program exited). Returns `true` if none remain.
    pub fn remove_pane(&mut self, id: PaneId) -> bool {
        let before = self.visible_panes();
        self.panes.remove(&id);
        let area = self.area();
        if let Some(index) = self.tabs.iter().position(|t| t.contains(id))
            && self.tabs[index].remove(area, id)
        {
            self.tabs.remove(index);
            if index < self.active || self.active >= self.tabs.len() {
                self.active = self.active.saturating_sub(1);
            }
        }
        self.arrange(&before);
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

    /// The tabs, in order.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// The index of the shown tab.
    #[must_use]
    pub fn active_tab(&self) -> usize {
        self.active
    }

    /// The focused pane of the shown tab.
    #[must_use]
    pub fn focus(&self) -> Option<PaneId> {
        self.tabs.get(self.active).map(|t| t.focus)
    }

    /// Current window size.
    #[must_use]
    pub fn size(&self) -> ScreenSize {
        self.size
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

    /// Panes clients currently see: the shown tab's (just the focused pane
    /// when zoomed). Every other pane keeps parsing but costs no rendering.
    #[must_use]
    pub fn visible_panes(&self) -> Vec<PaneId> {
        self.visible().into_iter().map(|(id, _)| id).collect()
    }

    /// The layout message for the current state.
    #[must_use]
    pub fn layout(&self) -> LayoutMsg {
        let panes = self
            .visible()
            .into_iter()
            .filter_map(|(id, rect)| {
                let entry = self.panes.get(&id)?;
                Some(PaneLayout {
                    pane: id,
                    x: rect.x,
                    y: rect.y,
                    size: ScreenSize::new(rect.cols, rect.rows),
                    title: entry.title.clone(),
                })
            })
            .collect();
        let tabs = self
            .tabs
            .iter()
            .map(|tab| TabSummary {
                focus: tab.focus,
                zoomed: tab.zoomed,
                panes: tab
                    .tree
                    .panes()
                    .into_iter()
                    .map(|p| self.summary(p))
                    .collect(),
            })
            .collect();
        LayoutMsg {
            generation: self.generation,
            size: self.size,
            focus: self.focus(),
            panes,
            tabs,
            active_tab: self.active,
        }
    }

    /// A pane's names for tab labels and the go-to picker.
    fn summary(&self, pane: PaneId) -> PaneSummary {
        let entry = self.panes.get(&pane);
        PaneSummary {
            pane,
            title: entry.and_then(|e| e.title.clone()),
            command: entry.and_then(|e| e.command.clone()),
        }
    }

    /// The whole window, which the shown tab fills.
    fn area(&self) -> Rect {
        Rect::new(0, 0, self.size.cols, self.size.rows)
    }

    fn visible(&self) -> Vec<(PaneId, Rect)> {
        self.tabs
            .get(self.active)
            .map(|tab| tab.visible(self.area()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
