//! What each pane runs and where: its foreground program, which names
//! panes whose program sets no title, and its directory and git checkout,
//! which group panes in the sidebar.

use monarch_core::PaneId;
use monarch_proto::{PanePlace, PaneSummary, ServerMsg};

use super::SessionState;

impl SessionState {
    /// Every pane's foreground process id, where it has one.
    #[must_use]
    pub fn foreground_processes(&self) -> Vec<(PaneId, u32)> {
        self.panes
            .iter()
            .filter_map(|(id, e)| Some((*id, e.io.foreground_process()?)))
            .collect()
    }

    /// Records a pane's foreground program and tells every client if it
    /// changed. Returns whether it did.
    pub fn set_command(&mut self, id: PaneId, command: Option<String>) -> bool {
        let Some(entry) = self.panes.get_mut(&id) else {
            return false;
        };
        if entry.command == command {
            return false;
        }
        entry.command.clone_from(&command);
        self.broadcast(&ServerMsg::Command { pane: id, command });
        true
    }

    /// Records where a pane works and tells every client if it changed.
    /// Returns whether it did.
    pub fn set_place(&mut self, id: PaneId, place: Option<PanePlace>) -> bool {
        let Some(entry) = self.panes.get_mut(&id) else {
            return false;
        };
        if entry.place == place {
            return false;
        }
        entry.place.clone_from(&place);
        self.broadcast(&ServerMsg::Place { pane: id, place });
        true
    }

    /// A pane's names and place for tab labels, the go-to picker and the
    /// sidebar.
    pub(super) fn summary(&self, pane: PaneId) -> PaneSummary {
        let entry = self.panes.get(&pane);
        PaneSummary {
            pane,
            title: entry.and_then(|e| e.title.clone()),
            command: entry.and_then(|e| e.command.clone()),
            place: entry.and_then(|e| e.place.clone()),
        }
    }
}
