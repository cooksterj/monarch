//! Which agents need the user, most urgent first: what the inbox lists
//! and `next-blocked` cycles through.

use monarch_core::{AgentState, PaneId};
use monarch_proto::AgentStatus;

use super::AttachState;

/// Urgency of a state that needs the user; `None` for the rest.
fn urgency(state: AgentState) -> Option<u8> {
    match state {
        AgentState::Blocked => Some(0),
        AgentState::Error => Some(1),
        AgentState::Done => Some(2),
        _ => None,
    }
}

impl AttachState {
    /// Every pane whose agent is blocked, failed or done, as (tab, pane,
    /// name, status): blocked first, then errors, then done, each in tab
    /// order.
    #[must_use]
    pub fn needing_attention(&self) -> Vec<(usize, PaneId, &str, &AgentStatus)> {
        let Some(layout) = self.layout.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<_> = layout
            .tabs
            .iter()
            .enumerate()
            .flat_map(|(tab, summary)| summary.panes.iter().map(move |p| (tab, p)))
            .filter_map(|(tab, pane)| {
                let status = self.agent(pane);
                urgency(status.state)?;
                Some((tab, pane.pane, self.name(pane), status))
            })
            .collect();
        out.sort_by_key(|(_, _, _, status)| urgency(status.state));
        out
    }

    /// The first blocked pane after `from` in tab order, wrapping; `from`
    /// itself only if it is the one blocked pane.
    #[must_use]
    pub fn next_blocked(&self, from: Option<PaneId>) -> Option<PaneId> {
        let order: Vec<PaneId> = self.all_panes().into_iter().map(|(_, p, _)| p).collect();
        let blocked = |p: &PaneId| {
            self.needing_attention()
                .iter()
                .any(|(_, q, _, s)| q == p && s.state == AgentState::Blocked)
        };
        let start = from
            .and_then(|f| order.iter().position(|p| *p == f))
            .map_or(0, |i| i + 1);
        let len = order.len();
        (0..len).map(|i| order[(start + i) % len]).find(blocked)
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;
    use monarch_proto::{LayoutMsg, PaneSummary, ScreenSize, ServerMsg, TabSummary};

    use super::*;
    use crate::Applied;

    fn state(states: &[(u32, AgentState)]) -> AttachState {
        let pane = |id| PaneSummary {
            pane: PaneId(id),
            title: None,
            command: Some("zsh".into()),
            place: None,
            agent: AgentStatus::default(),
        };
        let tab = |ids: &[u32]| TabSummary {
            focus: PaneId(ids[0]),
            zoomed: false,
            panes: ids.iter().map(|i| pane(*i)).collect(),
        };
        let layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(80, 24),
            focus: Some(PaneId(1)),
            panes: Vec::new(),
            tabs: vec![tab(&[1, 2]), tab(&[3, 4])],
            active_tab: 0,
        };
        let mut state = AttachState::new();
        state.apply(ServerMsg::Layout(layout), &mut Applied::default());
        for (id, s) in states {
            let status = AgentStatus {
                state: *s,
                activity: None,
                agent: Some(AgentKind::Codex),
            };
            let msg = ServerMsg::Agent {
                pane: PaneId(*id),
                status,
            };
            state.apply(msg, &mut Applied::default());
        }
        state
    }

    #[test]
    fn blocked_first_then_errors_then_done() {
        let state = state(&[
            (1, AgentState::Done),
            (2, AgentState::Working),
            (3, AgentState::Error),
            (4, AgentState::Blocked),
        ]);
        let order: Vec<(usize, u32, &str)> = state
            .needing_attention()
            .iter()
            .map(|(tab, pane, name, _)| (*tab, pane.0, *name))
            .collect();
        assert_eq!(order, [(1, 4, "codex"), (1, 3, "codex"), (0, 1, "codex")]);
        assert!(AttachState::new().needing_attention().is_empty());
    }

    #[test]
    fn next_blocked_cycles_in_tab_order() {
        let state = state(&[(2, AgentState::Blocked), (4, AgentState::Blocked)]);
        assert_eq!(state.next_blocked(Some(PaneId(1))), Some(PaneId(2)));
        assert_eq!(state.next_blocked(Some(PaneId(2))), Some(PaneId(4)));
        assert_eq!(
            state.next_blocked(Some(PaneId(4))),
            Some(PaneId(2)),
            "wraps"
        );
        assert_eq!(state.next_blocked(None), Some(PaneId(2)));
        let one = self::state(&[(3, AgentState::Blocked)]);
        assert_eq!(one.next_blocked(Some(PaneId(3))), Some(PaneId(3)));
        assert_eq!(self::state(&[]).next_blocked(Some(PaneId(1))), None);
    }
}
