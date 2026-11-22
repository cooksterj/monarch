//! Starting agents for the ask composer: new panes beside the focused
//! one, each given the ask as its first message.

use monarch_core::{AgentKind, PaneId, SplitAxis};

use super::ServerContext;
use crate::{Placement, ServerError};

impl ServerContext {
    /// Starts each of `kinds` beside the focused pane, each with `prompt`
    /// as its first message: the first to its right, the rest stacked
    /// below it. Stops at the first that fails to start.
    ///
    /// # Errors
    /// A PTY or program could not be started.
    pub fn spawn_agents_with_prompt(
        &self,
        kinds: &[AgentKind],
        prompt: &str,
    ) -> Result<Vec<PaneId>, ServerError> {
        let mut started = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            let axis = if i == 0 {
                SplitAxis::Columns
            } else {
                SplitAxis::Rows
            };
            let spec = self.agent_spec(*kind).with_prompt(prompt);
            started.push(self.spawn(&spec, None, Placement::Split(axis))?);
        }
        Ok(started)
    }
}
