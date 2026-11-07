//! Per-pane facts that arrive after the layout: title, foreground
//! command, place and agent status.

use monarch_proto::ServerMsg;

use super::AttachState;
use crate::Applied;

impl AttachState {
    /// Records a `Title`, `Command`, `Place` or `Agent` message; an
    /// `Agent` also names its pane in `out.agents`.
    pub(super) fn apply_fact(&mut self, msg: ServerMsg, out: &mut Applied) {
        match msg {
            ServerMsg::Title { pane, title } => {
                self.titles.insert(pane, title.unwrap_or_default());
            }
            ServerMsg::Command { pane, command } => {
                self.commands.insert(pane, command.unwrap_or_default());
            }
            ServerMsg::Place { pane, place } => {
                self.places.insert(pane, place);
            }
            ServerMsg::Agent {
                pane,
                status,
                timing,
            } => {
                self.agents.insert(pane, status);
                self.timing.insert(pane, timing);
                out.agents.push(pane);
            }
            _ => {}
        }
    }
}
