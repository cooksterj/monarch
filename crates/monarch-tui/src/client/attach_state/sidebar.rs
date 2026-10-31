//! The sidebar's model: every pane grouped by the git checkout it works
//! in, groups in alphabetical order and panes in tab order, with panes
//! outside a repository last.

use std::path::Path;

use monarch_core::{AgentState, PaneId};
use monarch_proto::{AgentStatus, PanePlace, PaneSummary};

use super::AttachState;
use crate::{SidebarGroup, SidebarRow};

impl AttachState {
    /// Every pane in every tab grouped by checkout, the groups sorted by
    /// project then branch (ignoring case), then the panes outside any
    /// repository under `Scratch`.
    #[must_use]
    pub fn sidebar_groups(&self) -> Vec<SidebarGroup> {
        let Some(layout) = self.layout.as_ref() else {
            return Vec::new();
        };
        let mut groups: Vec<SidebarGroup> = Vec::new();
        let mut scratch = SidebarGroup::scratch();
        for (tab, summary) in layout.tabs.iter().enumerate() {
            for pane in &summary.panes {
                let place = self.place(pane);
                let agent = self.agent(pane);
                let row = SidebarRow {
                    pane: pane.pane,
                    label: format!("{} {}", tab + 1, self.name(summary, pane))
                        .trim_end()
                        .to_owned(),
                    state: agent.state,
                    look: None,
                    detail: activity(agent).or_else(|| place.and_then(where_in)),
                    focused: Some(pane.pane) == layout.focus,
                    shown: tab == layout.active_tab,
                };
                match place.filter(|p| p.worktree.is_some()) {
                    Some(place) => group_for(&mut groups, place).rows.push(row),
                    None => scratch.rows.push(row),
                }
            }
        }
        groups.sort_by_cached_key(|g| (g.title.to_lowercase(), g.detail.clone()));
        if !scratch.rows.is_empty() {
            groups.push(scratch);
        }
        groups
    }

    /// What a pane's agent does, from the latest `Agent` or else the
    /// layout.
    pub(super) fn agent<'a>(&'a self, sent: &'a PaneSummary) -> &'a AgentStatus {
        self.agents.get(&sent.pane).unwrap_or(&sent.agent)
    }

    /// Where `pane` works, if it is in the layout and its place is known.
    #[must_use]
    pub fn place_of(&self, pane: PaneId) -> Option<&PanePlace> {
        let layout = self.layout.as_ref()?;
        let mut panes = layout.tabs.iter().flat_map(|t| &t.panes);
        self.place(panes.find(|p| p.pane == pane)?)
    }

    /// Where a pane works, from the latest `Place` or else the layout.
    fn place<'a>(&'a self, sent: &'a PaneSummary) -> Option<&'a PanePlace> {
        match self.places.get(&sent.pane) {
            Some(latest) => latest.as_ref(),
            None => sent.place.as_ref(),
        }
    }
}

/// The group for `place`'s checkout, added at the end if new.
fn group_for<'a>(groups: &'a mut Vec<SidebarGroup>, place: &PanePlace) -> &'a mut SidebarGroup {
    let index = match groups.iter().position(|g| g.worktree == place.worktree) {
        Some(index) => index,
        None => {
            groups.push(SidebarGroup {
                worktree: place.worktree.clone(),
                title: place.project.clone().unwrap_or_default(),
                detail: place.branch.clone(),
                status: None,
                rows: Vec::new(),
            });
            groups.len() - 1
        }
    };
    let group = &mut groups[index];
    group.status = group.status.or(place.status);
    group
}

/// An agent's activity line, or its state's name when it has none; a
/// plain shell has neither.
fn activity(agent: &AgentStatus) -> Option<String> {
    if agent.state == AgentState::Unknown {
        return None;
    }
    Some(
        agent
            .activity
            .clone()
            .unwrap_or_else(|| agent.state.label().to_owned()),
    )
}

/// The directory below the checkout's top (`src/x`), or the directory's
/// name outside one; nothing at the top itself.
fn where_in(place: &PanePlace) -> Option<String> {
    let cwd = Path::new(&place.cwd);
    let shown = match &place.worktree {
        Some(top) => cwd.strip_prefix(top).ok()?.to_string_lossy().into_owned(),
        None => cwd.file_name()?.to_string_lossy().into_owned(),
    };
    (!shown.is_empty()).then_some(shown)
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{CheckoutStatus, LayoutMsg, ScreenSize, ServerMsg, TabSummary};

    use super::*;
    use crate::Applied;

    fn place(cwd: &str, top: Option<&str>, branch: &str) -> PanePlace {
        PanePlace {
            cwd: cwd.into(),
            project: top.map(|t| t.rsplit('/').next().unwrap_or(t).into()),
            worktree: top.map(Into::into),
            branch: top.map(|_| branch.into()),
            status: None,
        }
    }

    fn summary(id: u32, command: &str, at: Option<PanePlace>) -> PaneSummary {
        PaneSummary {
            given_name: None,
            pane: PaneId(id),
            title: None,
            command: Some(command.into()),
            place: at,
            agent: Default::default(),
        }
    }

    fn state() -> AttachState {
        let tab = |focus, panes| TabSummary {
            name: None,
            focus: PaneId(focus),
            zoomed: false,
            panes,
        };
        let layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(80, 24),
            focus: Some(PaneId(2)),
            panes: Vec::new(),
            tabs: vec![
                tab(
                    2,
                    vec![
                        summary(1, "claude", Some(place("/w/api", Some("/w/api"), "main"))),
                        summary(2, "zsh", Some(place("/tmp/x", None, ""))),
                    ],
                ),
                tab(
                    3,
                    vec![
                        summary(3, "vim", Some(place("/w/api/src", Some("/w/api"), "main"))),
                        summary(4, "sh", None),
                    ],
                ),
            ],
            active_tab: 0,
        };
        let mut state = AttachState::new();
        state.apply(ServerMsg::Layout(layout), &mut Applied::default());
        state
    }

    #[test]
    fn a_named_tab_names_its_rows_and_its_label() {
        let mut state = state();
        let Some(layout) = state.layout.as_mut() else {
            unreachable!("state() has a layout");
        };
        layout.tabs[1].name = Some("api docs".into());
        let labels: Vec<String> = state
            .sidebar_groups()
            .into_iter()
            .flat_map(|g| g.rows)
            .map(|r| r.label)
            .collect();
        assert!(labels.contains(&"2 api docs".to_owned()), "{labels:?}");
        assert!(labels.contains(&"1 claude".to_owned()), "others unchanged");
        assert_eq!(state.tab_labels().0, ["1 zsh", "2 api docs"]);
        let named = state.layout.as_ref().map(|l| l.tabs[1].clone());
        assert_eq!(named.as_ref().map(|t| state.tab_name(t)), Some("api docs"));
        assert_eq!(state.shown_tab().map(|t| t.focus), Some(PaneId(2)));
    }

    #[test]
    fn a_named_pane_wins_over_its_tab_name() {
        let mut state = state();
        let Some(layout) = state.layout.as_mut() else {
            unreachable!("state() has a layout");
        };
        layout.tabs[1].name = Some("api".into());
        layout.tabs[1].panes[0].given_name = Some("editor".into());
        layout.tabs[0].panes[1].given_name = Some("scratch".into());
        let labels: Vec<String> = state
            .sidebar_groups()
            .into_iter()
            .flat_map(|g| g.rows)
            .map(|r| r.label)
            .collect();
        for want in ["2 editor", "2 api", "1 scratch", "1 claude"] {
            assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
        }
        assert_eq!(
            state.tab_labels().0,
            ["1 scratch", "2 api"],
            "a tab is named by its name, else its focused pane's"
        );
    }

    #[test]
    fn places_are_found_by_pane() {
        let state = state();
        let vim = state.place_of(PaneId(3)).map(|p| p.cwd.as_str());
        assert_eq!(vim, Some("/w/api/src"));
        assert_eq!(state.place_of(PaneId(4)), None, "not known yet");
        assert_eq!(state.place_of(PaneId(9)), None, "no such pane");
    }

    #[test]
    fn panes_group_by_checkout_with_scratch_last() {
        let groups = state().sidebar_groups();
        let shape: Vec<(&str, Option<&str>, Vec<&str>)> = groups
            .iter()
            .map(|g| {
                let rows = g.rows.iter().map(|r| r.label.as_str()).collect();
                (g.title.as_str(), g.detail.as_deref(), rows)
            })
            .collect();
        assert_eq!(
            shape,
            [
                ("api", Some("main"), vec!["1 claude", "2 vim"]),
                ("Scratch", None, vec!["1 zsh", "2 sh"]),
            ]
        );
        let api = &groups[0].rows;
        assert_eq!(
            (api[0].detail.as_deref(), api[1].detail.as_deref()),
            (None, Some("src"))
        );
        assert!(groups[1].rows[0].focused && groups[1].rows[0].shown);
        assert!(!api[1].shown, "tab 2 is hidden");
        assert_eq!(groups[1].rows[0].detail.as_deref(), Some("x"));
    }

    #[test]
    fn groups_are_alphabetical_with_scratch_last() {
        let mut state = state();
        let moves = [
            (1, place("/w/zeta", Some("/w/zeta"), "main")),
            (3, place("/w/Alpha-x", Some("/w/Alpha-x"), "x")),
            (4, place("/w/alpha", Some("/w/alpha"), "main")),
        ];
        for (pane, at) in moves {
            let msg = ServerMsg::Place {
                pane: PaneId(pane),
                place: Some(at),
            };
            state.apply(msg, &mut Applied::default());
        }
        let order: Vec<_> = state
            .sidebar_groups()
            .into_iter()
            .map(|g| g.title)
            .collect();
        assert_eq!(order, ["alpha", "Alpha-x", "zeta", "Scratch"]);
    }

    #[test]
    fn later_places_move_panes_between_groups() {
        let mut state = state();
        let feat = place("/w/api-feat", Some("/w/api-feat"), "feat");
        let msgs = [
            ServerMsg::Place {
                pane: PaneId(1),
                place: Some(feat),
            },
            ServerMsg::Place {
                pane: PaneId(3),
                place: None,
            },
        ];
        msgs.into_iter()
            .for_each(|m| state.apply(m, &mut Applied::default()));
        let titles: Vec<_> = state
            .sidebar_groups()
            .into_iter()
            .map(|g| (g.detail, g.rows.len()))
            .collect();
        assert_eq!(titles, [(Some("feat".into()), 1), (None, 3)]);
        assert!(
            AttachState::new().sidebar_groups().is_empty(),
            "no layout yet"
        );
    }

    #[test]
    fn a_group_shows_its_checkout_status() {
        let mut state = state();
        assert_eq!(state.sidebar_groups()[0].status, None, "not read yet");
        let dirty = CheckoutStatus {
            added: 2,
            removed: 0,
            untracked: 0,
            ahead: 0,
            behind: 1,
        };
        let place = PanePlace {
            status: Some(dirty),
            ..place("/w/api/src", Some("/w/api"), "main")
        };
        let msg = ServerMsg::Place {
            pane: PaneId(3),
            place: Some(place),
        };
        state.apply(msg, &mut Applied::default());
        assert_eq!(state.sidebar_groups()[0].status, Some(dirty), "from pane 3");
    }

    /// The test state with pane 1 blocked and pane 3 working, both Claude.
    fn with_agents() -> Vec<SidebarRow> {
        let mut state = state();
        let status = |state, activity: Option<&str>| AgentStatus {
            state,
            activity: activity.map(Into::into),
            agent: Some(monarch_core::AgentKind::Claude),
        };
        let blocked = status(AgentState::Blocked, Some("Allow Bash: rm -rf target?"));
        let msgs = [
            ServerMsg::Agent {
                pane: PaneId(1),
                status: blocked,
            },
            ServerMsg::Agent {
                pane: PaneId(3),
                status: status(AgentState::Working, None),
            },
        ];
        msgs.into_iter()
            .for_each(|m| state.apply(m, &mut Applied::default()));
        state
            .sidebar_groups()
            .into_iter()
            .flat_map(|g| g.rows)
            .collect()
    }

    #[test]
    fn agent_rows_show_their_state_and_activity() {
        let rows: Vec<_> = with_agents()
            .into_iter()
            .map(|r| (r.state, r.detail))
            .collect();
        let blocked = Some("Allow Bash: rm -rf target?".into());
        assert_eq!(
            rows[..2],
            [
                (AgentState::Blocked, blocked),
                (AgentState::Working, Some("Working".into())),
            ]
        );
        let shell = (AgentState::Unknown, Some("x".into()));
        assert_eq!(rows[2], shell, "a shell keeps its place");
    }

    #[test]
    fn agent_panes_are_named_after_their_agent() {
        let labels: Vec<String> = with_agents().into_iter().map(|r| r.label).collect();
        assert_eq!(labels, ["1 claude", "2 claude", "1 zsh", "2 sh"]);
    }
}
