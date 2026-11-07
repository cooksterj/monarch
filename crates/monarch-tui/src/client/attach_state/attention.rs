//! Which agents need the user, most urgent first: what the inbox lists
//! and `next-blocked` cycles through.

use monarch_core::{AgentKind, AgentState, PaneId, UnixMillis};
use monarch_proto::AgentStatus;

use super::AttachState;
use crate::AttentionItem;

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
    /// Every pane whose agent is blocked, failed or done, at `now`:
    /// blocked first, then errors, then done; within each, the one that
    /// has been so longest first (those with no known start last), then
    /// tab order.
    #[must_use]
    pub fn needing_attention(&self, now: UnixMillis) -> Vec<AttentionItem<'_>> {
        let Some(layout) = self.layout.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<(Option<UnixMillis>, AttentionItem<'_>)> = layout
            .tabs
            .iter()
            .enumerate()
            .flat_map(|(tab, summary)| summary.panes.iter().map(move |p| (tab, summary, p)))
            .filter_map(|(tab, summary, pane)| {
                let status = self.agent(pane);
                urgency(status.state)?;
                let item = AttentionItem {
                    tab,
                    pane: pane.pane,
                    name: self.name(summary, pane),
                    status,
                    age: self.age(pane, now),
                };
                Some((self.since(pane), item))
            })
            .collect();
        out.sort_by_key(|(since, item)| {
            let wait = since.map_or(u64::MAX, |s| s.0);
            (urgency(item.status.state), wait)
        });
        out.into_iter().map(|(_, item)| item).collect()
    }

    /// What `pane`'s agent is doing: the latest `Agent` message, else what
    /// the layout said; `None` for a pane not in the layout.
    #[must_use]
    pub fn agent_status(&self, pane: PaneId) -> Option<&AgentStatus> {
        if let Some(status) = self.agents.get(&pane) {
            return Some(status);
        }
        let layout = self.layout.as_ref()?;
        let mut sent = layout.tabs.iter().flat_map(|t| t.panes.iter());
        sent.find(|p| p.pane == pane).map(|p| &p.agent)
    }

    /// Which agent `pane` runs, if one has reported or Monarch launched it.
    #[must_use]
    pub fn agent_kind(&self, pane: PaneId) -> Option<AgentKind> {
        self.agent_status(pane).and_then(|s| s.agent)
    }

    /// Whether an agent runs in `pane`: one Monarch knows by kind, or one
    /// with a state (read from its screen or a notification).
    #[must_use]
    pub fn is_agent(&self, pane: PaneId) -> bool {
        self.agent_status(pane)
            .is_some_and(|s| s.agent.is_some() || s.state != AgentState::Unknown)
    }

    /// The first blocked pane after `from` in tab order, wrapping; `from`
    /// itself only if it is the one blocked pane.
    #[must_use]
    pub fn next_blocked(&self, from: Option<PaneId>) -> Option<PaneId> {
        let order: Vec<PaneId> = self.all_panes().into_iter().map(|(_, p, _)| p).collect();
        let blocked = |p: &PaneId| {
            self.agent_status(*p)
                .is_some_and(|s| s.state == AgentState::Blocked)
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
    use monarch_proto::{AgentTiming, LayoutMsg, PaneSummary, ScreenSize, ServerMsg, TabSummary};

    use super::*;
    use crate::Applied;

    fn pane(id: u32) -> PaneSummary {
        PaneSummary {
            given_name: None,
            pane: PaneId(id),
            title: None,
            command: Some("zsh".into()),
            place: None,
            agent: AgentStatus::default(),
            timing: None,
        }
    }

    fn state(states: &[(u32, AgentState)]) -> AttachState {
        let tab = |ids: &[u32]| TabSummary {
            name: None,
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
                timing: Some(AgentTiming::begun(
                    UnixMillis(1_000 * u64::from(*id)),
                    false,
                )),
            };
            state.apply(msg, &mut Applied::default());
        }
        state
    }

    const NOW: UnixMillis = UnixMillis(10_000);

    #[test]
    fn blocked_first_then_errors_then_done() {
        let state = state(&[
            (1, AgentState::Done),
            (2, AgentState::Working),
            (3, AgentState::Error),
            (4, AgentState::Blocked),
        ]);
        let order: Vec<(usize, u32, &str)> = state
            .needing_attention(NOW)
            .iter()
            .map(|item| (item.tab, item.pane.0, item.name))
            .collect();
        assert_eq!(order, [(1, 4, "codex"), (1, 3, "codex"), (0, 1, "codex")]);
        assert!(AttachState::new().needing_attention(NOW).is_empty());
        assert!(!AttachState::new().any_running());
        assert!(state.any_running());
    }

    #[test]
    fn the_longest_wait_comes_first_and_unknown_ages_last() {
        let mut state = state(&[
            (1, AgentState::Blocked),
            (2, AgentState::Blocked),
            (3, AgentState::Blocked),
            (4, AgentState::Done),
        ]);
        let unknown = ServerMsg::Agent {
            pane: PaneId(1),
            status: state.agent_status(PaneId(1)).cloned().expect("status"),
            timing: None,
        };
        state.apply(unknown, &mut Applied::default());
        let order: Vec<(u32, Option<u64>)> = state
            .needing_attention(NOW)
            .iter()
            .map(|item| (item.pane.0, item.age.map(|a| a.as_secs())))
            .collect();
        assert_eq!(
            order,
            [(2, Some(8)), (3, Some(7)), (1, None), (4, None)],
            "pane 2 has waited longest; pane 1's wait is unknown; done last, \
             with no turn behind it to show"
        );
        assert_eq!(state.next_blocked(Some(PaneId(3))), Some(PaneId(1)));
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
