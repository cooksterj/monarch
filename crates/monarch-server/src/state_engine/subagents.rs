//! The sub-agents inside each pane's agent, and how they shape its state.
//!
//! A sub-agent has no pane: its hooks fire in its parent's, naming it by
//! `agent_id`. While one waits on the user the pane is Blocked and its
//! line says which (`Explore: Allow Bash ls?`), even as its siblings keep
//! working; once nobody waits, the parent's own line comes back. Rows
//! end with their parent's turn: finished ones then, and
//! every one when the agent says none still runs in the background, so a
//! missed `SubagentStop` (an interrupted turn) heals at the next.

use monarch_core::{AgentState, PaneId};
use monarch_proto::{AgentReport, AgentStatus, SubagentReport, SubagentStatus};

use super::StateEngine;

impl StateEngine {
    /// Most sub-agents kept per pane.
    pub const MAX_SUBAGENTS: usize = 16;

    /// The sub-agents of `pane`'s agent, oldest first.
    #[must_use]
    pub fn subagents(&self, pane: PaneId) -> &[SubagentStatus] {
        self.panes
            .get(&pane)
            .map_or(&[], |s| s.subagents.as_slice())
    }

    /// Applies `report` to the sub-agents in `next` (the pane's coming
    /// status): one of them reported, or the parent did for itself. A
    /// pane whose agent is working while a sub-agent waits on the user is
    /// then still Blocked, on that sub-agent's question; when the wait
    /// ends the parent's line from before it is back, unless the report
    /// brings a new one.
    pub(super) fn fuse_subagents(
        &mut self,
        pane: PaneId,
        report: &AgentReport,
        next: &mut AgentStatus,
    ) {
        let asking = match &report.subagent {
            Some(sub) => {
                apply(&mut next.subagents, sub);
                next.subagents
                    .iter()
                    .filter(|c| c.state == AgentState::Blocked)
                    .max_by_key(|c| c.id == sub.id)
                    .filter(|_| matches!(next.state, AgentState::Working | AgentState::Blocked))
                    .map(SubagentStatus::asking)
            }
            None if next.state == AgentState::Blocked => {
                self.line_before_asking.remove(&pane);
                return;
            }
            None => {
                parent_moved(&mut next.subagents, report);
                None
            }
        };
        if let Some(line) = asking {
            self.line_before_asking
                .entry(pane)
                .or_insert_with(|| next.activity.clone());
            next.state = AgentState::Blocked;
            next.activity = line;
        } else if let Some(line) = self.line_before_asking.remove(&pane)
            && report.activity.is_none()
        {
            next.activity = line;
        }
    }
}

/// One sub-agent reported: update its row, or add one.
fn apply(children: &mut Vec<SubagentStatus>, sub: &SubagentReport) {
    if let Some(child) = children.iter_mut().find(|c| c.id == sub.id) {
        update(child, sub);
        return;
    }
    // Claude runs agents of its own (prompt suggestions, `/btw`) that
    // only ever report finishing: they are not the user's.
    if sub
        .state
        .is_some_and(|s| s != AgentState::Working && s != AgentState::Blocked)
    {
        return;
    }
    if children.len() >= StateEngine::MAX_SUBAGENTS {
        match children.iter().position(|c| c.state == AgentState::Done) {
            Some(i) => drop(children.remove(i)),
            None => return,
        }
    }
    let mut child = SubagentStatus {
        id: sub.id.clone(),
        kind: String::new(),
        state: AgentState::Working,
        activity: None,
    };
    update(&mut child, sub);
    children.push(child);
}

/// The parent reported for itself: no sub-agent is still waiting on the
/// user (the parent can't move while one is), and a turn's end or the
/// agent's drop rows.
fn parent_moved(children: &mut Vec<SubagentStatus>, report: &AgentReport) {
    if report.state == Some(AgentState::Blocked) {
        return;
    }
    for child in children
        .iter_mut()
        .filter(|c| c.state == AgentState::Blocked)
    {
        child.state = AgentState::Working;
    }
    match report.state {
        Some(AgentState::Done) if report.subagents_running == Some(0) => children.clear(),
        Some(AgentState::Done) => children.retain(|c| c.state != AgentState::Done),
        Some(AgentState::Idle | AgentState::Error | AgentState::Unknown) => children.clear(),
        _ => {}
    }
}

fn update(child: &mut SubagentStatus, sub: &SubagentReport) {
    if let Some(kind) = sub.kind.as_deref().filter(|k| !k.is_empty()) {
        kind.clone_into(&mut child.kind);
    }
    if let Some(state) = sub.state {
        child.state = state;
    }
    if let Some(text) = &sub.activity {
        child.activity = AgentStatus::activity_line(text);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;

    use super::*;

    const P: PaneId = PaneId(1);

    fn parent(state: AgentState, activity: Option<&str>) -> AgentReport {
        AgentReport {
            state: Some(state),
            activity: activity.map(Into::into),
            ..AgentReport::new(P, AgentKind::Claude)
        }
    }

    /// What the hook sends for an event inside sub-agent `id`.
    fn child(id: &str, state: AgentState, activity: Option<&str>) -> AgentReport {
        let parent_state = match state {
            AgentState::Done => None,
            AgentState::Blocked => Some(AgentState::Blocked),
            _ => Some(AgentState::Working),
        };
        AgentReport {
            state: parent_state,
            subagent: Some(SubagentReport {
                id: id.into(),
                kind: Some(format!("kind-{id}")),
                state: Some(state),
                activity: activity.map(Into::into),
            }),
            ..AgentReport::new(P, AgentKind::Claude)
        }
    }

    fn rows(engine: &StateEngine) -> Vec<(&str, AgentState, Option<&str>)> {
        engine
            .subagents(P)
            .iter()
            .map(|c| (c.id.as_str(), c.state, c.activity.as_deref()))
            .collect()
    }

    #[test]
    fn children_report_without_taking_the_parents_line() {
        let mut engine = StateEngine::default();
        engine.report(P, &parent(AgentState::Working, Some("Agent find the bug")));
        engine.report(P, &child("a", AgentState::Working, Some("")));
        engine.report(P, &child("a", AgentState::Working, Some("Grep TODO")));
        assert_eq!(
            rows(&engine),
            [("a", AgentState::Working, Some("Grep TODO"))]
        );
        assert_eq!(engine.subagents(P)[0].kind, "kind-a");
        let status = engine.status(P);
        assert_eq!(
            (status.state, status.activity.as_deref()),
            (AgentState::Working, Some("Agent find the bug"))
        );
        engine.report(P, &child("a", AgentState::Done, Some("Found it\nin a.rs")));
        assert_eq!(rows(&engine), [("a", AgentState::Done, Some("Found it"))]);
        assert_eq!(engine.status(P).state, AgentState::Working);
    }

    #[test]
    fn a_waiting_child_blocks_the_pane_while_its_siblings_work() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("a", AgentState::Working, None));
        engine.report(P, &child("b", AgentState::Blocked, Some("Allow Bash ls?")));
        let blocked = engine.status(P);
        assert_eq!(
            (blocked.state, blocked.activity.as_deref()),
            (AgentState::Blocked, Some("kind-b: Allow Bash ls?"))
        );
        let sibling = engine.report(P, &child("a", AgentState::Working, Some("Read a.rs")));
        assert_eq!(
            sibling.map(|s| (s.state, s.subagents[0].activity.clone())),
            Some((AgentState::Blocked, Some("Read a.rs".into()))),
            "the sibling's line is published, still blocked on b"
        );
        engine.report(P, &child("b", AgentState::Working, None));
        assert_eq!(engine.status(P).state, AgentState::Working, "answered");
    }

    #[test]
    fn the_parents_line_comes_back_when_nobody_waits() {
        let mut engine = StateEngine::default();
        engine.report(P, &parent(AgentState::Working, Some("Agent find the bug")));
        engine.report(P, &child("a", AgentState::Blocked, Some("Allow Bash ls?")));
        engine.report(P, &child("b", AgentState::Blocked, Some("Allow Edit?")));
        engine.report(P, &child("b", AgentState::Working, Some("Edit a.rs")));
        let still = engine.status(P);
        assert_eq!(still.activity.as_deref(), Some("kind-a: Allow Bash ls?"));
        engine.report(P, &child("a", AgentState::Working, Some("Bash ls")));
        let back = engine.status(P);
        assert_eq!(
            (back.state, back.activity.as_deref()),
            (AgentState::Working, Some("Agent find the bug"))
        );
        engine.report(P, &child("a", AgentState::Blocked, None));
        engine.report(P, &parent(AgentState::Done, Some("Fixed")));
        assert_eq!(
            engine.status(P).activity.as_deref(),
            Some("Fixed"),
            "its own"
        );
    }

    #[test]
    fn the_parent_moving_ends_a_childs_wait() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("a", AgentState::Blocked, Some("Allow Edit?")));
        engine.report(P, &parent(AgentState::Working, Some("next prompt")));
        assert_eq!(rows(&engine)[0].1, AgentState::Working, "interrupted wait");
        engine.report(P, &child("b", AgentState::Working, None));
        assert_eq!(
            engine.status(P).state,
            AgentState::Working,
            "no stale block"
        );
    }

    #[test]
    fn a_turns_end_drops_finished_rows_and_all_once_none_run() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("a", AgentState::Working, None));
        engine.report(P, &child("b", AgentState::Working, None));
        engine.report(P, &child("a", AgentState::Done, Some("done")));
        engine.report(P, &parent(AgentState::Done, Some("Summary")));
        assert_eq!(
            rows(&engine),
            [("b", AgentState::Working, None)],
            "b runs on"
        );
        let stop = AgentReport {
            subagents_running: Some(0),
            ..parent(AgentState::Done, Some("Summary"))
        };
        engine.report(P, &stop);
        assert!(engine.subagents(P).is_empty(), "a missed stop heals");
        engine.report(P, &child("c", AgentState::Working, None));
        engine.report(P, &parent(AgentState::Unknown, None));
        assert!(engine.subagents(P).is_empty(), "the agent ended");
    }

    #[test]
    fn unknown_finishers_are_ignored_and_rows_are_capped() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("suggest", AgentState::Done, Some("try tests")));
        assert!(engine.subagents(P).is_empty(), "Claude's own agent");
        for i in 0..StateEngine::MAX_SUBAGENTS {
            engine.report(P, &child(&i.to_string(), AgentState::Working, None));
        }
        engine.report(P, &child("late", AgentState::Working, None));
        assert_eq!(engine.subagents(P).len(), StateEngine::MAX_SUBAGENTS);
        assert!(rows(&engine).iter().all(|r| r.0 != "late"), "none finished");
        engine.report(P, &child("3", AgentState::Done, None));
        engine.report(P, &child("late", AgentState::Working, None));
        assert_eq!(rows(&engine).last().map(|r| r.0), Some("late"));
        assert!(
            rows(&engine).iter().all(|r| r.0 != "3"),
            "a finished one went"
        );
        engine.forget(P);
        assert!(engine.subagents(P).is_empty());
    }
}
