//! The sub-agents inside each pane's agent, and how they shape its state.
//!
//! A sub-agent has no pane: its hooks fire in its parent's, naming it by
//! `agent_id`. While one waits on the user the pane is Blocked and its
//! line says which (`Explore: Allow Bash ls?`), even as its siblings keep
//! working; once nobody waits, the parent's own line comes back.
//!
//! Rows stay after the parent's turn ends, so the user can see what was
//! done, and go when the user sends the next prompt (not when the agent
//! wakes itself with a background sub-agent's result). At a turn's end a
//! row still shown as running whose id is not among the agent's
//! background tasks has finished, its `SubagentStop` missed or never
//! matched: it is marked done, never dropped. That holds only while every
//! background agent is accounted for: a teammate's task id is not its
//! `agent_id`, so an agent task matching no row means rows can't be told
//! apart and stay as they are, to finish by their own `SubagentStop`.

use monarch_core::{AgentState, PaneId};
use monarch_proto::{AgentReport, AgentStatus, BackgroundTask, SubagentReport, SubagentStatus};

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
/// user (the parent can't move while one is). The user's next prompt
/// drops the last turn's finished rows; a turn's end marks done the rows
/// no longer in the background; the agent's end, or a fresh session,
/// drops all.
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
    let finished = |c: &SubagentStatus| c.state == AgentState::Done;
    if report.user_prompt {
        children.retain(|c| !finished(c));
    }
    match (report.state, &report.background) {
        (Some(AgentState::Done), Some(running)) => finish_unlisted(children, running),
        (Some(AgentState::Idle | AgentState::Unknown), _) => children.clear(),
        _ => {}
    }
}

/// At a turn's end, marks done the rows still running that `running`
/// doesn't list, unless an agent there matches no row (a teammate, whose
/// task id differs from its `agent_id`): then which rows still run can't
/// be told, and none is touched.
fn finish_unlisted(children: &mut [SubagentStatus], running: &[BackgroundTask]) {
    let listed = |id: &str| running.iter().any(|t| t.id == id);
    let unmatched = running
        .iter()
        .any(|t| t.is_agent() && !children.iter().any(|c| c.id == t.id));
    if unmatched {
        return;
    }
    for child in children
        .iter_mut()
        .filter(|c| c.state != AgentState::Done && !listed(&c.id))
    {
        child.state = AgentState::Done;
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

    fn task(id: &str, kind: &str) -> BackgroundTask {
        BackgroundTask {
            id: id.into(),
            kind: kind.into(),
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
    fn finished_rows_stay_until_the_next_prompt() {
        let mut engine = StateEngine::default();
        engine.report(P, &parent(AgentState::Working, Some("audit")));
        engine.report(P, &child("a", AgentState::Working, None));
        engine.report(P, &child("b", AgentState::Working, None));
        engine.report(P, &child("a", AgentState::Done, Some("done")));
        engine.report(P, &parent(AgentState::Working, Some("Read a.rs")));
        assert_eq!(rows(&engine).len(), 2, "mid-turn, nothing goes");
        engine.report(P, &parent(AgentState::Done, Some("Summary")));
        let kept = [
            ("a", AgentState::Done, Some("done")),
            ("b", AgentState::Working, None),
        ];
        assert_eq!(rows(&engine), kept, "a stays to be seen, b runs on");
        engine.seen(P);
        assert_eq!(rows(&engine), kept, "looking at it keeps them");
        let prompt = AgentReport {
            user_prompt: true,
            ..parent(AgentState::Working, Some("next prompt"))
        };
        engine.report(P, &prompt);
        assert_eq!(
            rows(&engine),
            [("b", AgentState::Working, None)],
            "the next prompt starts clean"
        );
    }

    #[test]
    fn the_agent_waking_itself_with_a_result_keeps_the_rows() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("a", AgentState::Working, None));
        engine.report(P, &child("b", AgentState::Working, None));
        let started = AgentReport {
            background: Some(vec![task("a", "subagent"), task("b", "subagent")]),
            ..parent(AgentState::Done, Some("Started two agents"))
        };
        engine.report(P, &started);
        engine.report(P, &child("a", AgentState::Done, Some("Found 3")));
        let woken = parent(AgentState::Working, Some("Reading a sub-agent's report"));
        engine.report(P, &woken);
        engine.report(P, &child("b", AgentState::Done, Some("Found 0")));
        engine.report(P, &woken);
        let end = AgentReport {
            background: Some(Vec::new()),
            ..parent(AgentState::Done, Some("Clean on both counts"))
        };
        engine.report(P, &end);
        assert_eq!(
            rows(&engine),
            [
                ("a", AgentState::Done, Some("Found 3")),
                ("b", AgentState::Done, Some("Found 0"))
            ],
            "both stay until the user writes again"
        );
    }

    #[test]
    fn a_turns_end_marks_done_what_no_longer_runs() {
        let mut engine = StateEngine::default();
        engine.report(P, &child("a", AgentState::Working, Some("Grep TODO")));
        engine.report(P, &child("b", AgentState::Working, None));
        engine.report(P, &child("c", AgentState::Working, None));
        engine.report(P, &child("c", AgentState::Done, Some("ok")));
        let stop = AgentReport {
            background: Some(vec![task("b", "subagent"), task("shell-1", "shell")]),
            ..parent(AgentState::Done, Some("Summary"))
        };
        engine.report(P, &stop);
        assert_eq!(
            rows(&engine),
            [
                ("a", AgentState::Done, Some("Grep TODO")),
                ("b", AgentState::Working, None),
                ("c", AgentState::Done, Some("ok"))
            ],
            "a's stop went missing: done, kept; b still runs"
        );
        let unlisted = parent(AgentState::Done, Some("Again"));
        engine.report(P, &unlisted);
        assert_eq!(rows(&engine)[1].1, AgentState::Working, "no list, no guess");
        engine.report(P, &parent(AgentState::Error, Some("rate limited")));
        assert_eq!(rows(&engine).len(), 3, "a failed turn keeps them too");
        engine.report(P, &parent(AgentState::Unknown, None));
        assert!(engine.subagents(P).is_empty(), "the agent ended");
    }

    #[test]
    fn teammates_listed_under_other_ids_are_not_done_early() {
        let mut engine = StateEngine::default();
        for id in ["atimer-1", "atimer-2", "atimer-3"] {
            engine.report(P, &child(id, AgentState::Working, Some("Bash sleep 10")));
        }
        let teammates = vec![
            task("tj1z2y49j", "teammate"),
            task("tkmaw2byu", "teammate"),
            task("t2vc88d6r", "teammate"),
        ];
        let stop = AgentReport {
            background: Some(teammates),
            ..parent(AgentState::Done, Some("All three are running"))
        };
        engine.report(P, &stop);
        assert!(
            rows(&engine).iter().all(|r| r.1 == AgentState::Working),
            "still running: {:?}",
            rows(&engine)
        );
        engine.report(
            P,
            &child("atimer-1", AgentState::Done, Some("timer-1 done")),
        );
        engine.report(P, &stop);
        let states: Vec<AgentState> = rows(&engine).iter().map(|r| r.1).collect();
        assert_eq!(
            states,
            [AgentState::Done, AgentState::Working, AgentState::Working],
            "each finishes by its own stop"
        );
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
