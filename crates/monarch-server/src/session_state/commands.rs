//! Naming panes after their foreground program, for panes whose program
//! sets no title.

use monarch_core::PaneId;
use monarch_proto::ServerMsg;

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
}
