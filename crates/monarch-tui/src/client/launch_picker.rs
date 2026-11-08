//! The launch picker (`prefix A`): which agent to start in a new tab, each
//! with the command `[agents.<id>]` runs for it.

use std::collections::BTreeMap;

use monarch_core::{Action, AgentConfig, AgentKind, SESSION_ID};

use super::{LaunchStep, Picker, PickerItem, PickerStep};
use crate::OverlayKey;

const TITLE: &str = "launch an agent";
const KEYS: &str = " ↑↓ move · enter new tab here · esc close ";
const NO_MATCH: &str = "no such agent";

/// The agents on offer and the search over them. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPicker {
    picker: Picker,
}

impl LaunchPicker {
    /// A picker over every agent kind, showing the command each one runs
    /// (`agents` is the `[agents.<id>]` config; a kind missing from it or
    /// naming no command shows the built-in).
    #[must_use]
    pub fn new(agents: &BTreeMap<String, AgentConfig>) -> Self {
        let items = AgentKind::ALL
            .into_iter()
            .map(|kind| item(kind, &configured(agents, kind)))
            .collect();
        Self {
            picker: Picker::new(TITLE, items).with_text(KEYS, NO_MATCH),
        }
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type to search, ↑↓ move, Enter launches the
    /// highlighted agent, Esc closes.
    pub fn key(&mut self, key: OverlayKey) -> LaunchStep {
        match self.picker.key(key) {
            PickerStep::Close => LaunchStep::Close,
            PickerStep::Run(_) => self
                .picker
                .selected_index()
                .and_then(|i| AgentKind::ALL.get(i).copied())
                .map_or(LaunchStep::Open, LaunchStep::Launch),
            PickerStep::Open | PickerStep::Answer(..) => LaunchStep::Open,
        }
    }
}

/// The config for `kind`, as `Config::agent` resolves it.
fn configured(agents: &BTreeMap<String, AgentConfig>, kind: AgentKind) -> AgentConfig {
    agents
        .get(kind.name())
        .filter(|agent| !agent.command.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| AgentConfig::for_kind(kind))
}

/// A row: the agent's icon and name, with its command line at the right.
fn item(kind: AgentKind, config: &AgentConfig) -> PickerItem {
    PickerItem {
        label: format!("{} {}", kind.icon(), kind.display_name()),
        detail: command_line(config),
        // Never run: Enter is handled by row index.
        action: Action::LaunchAgent,
    }
}

/// The command and its arguments, without the session id Monarch adds
/// (`--session-id {session_id}`): what the user wrote, in effect.
fn command_line(config: &AgentConfig) -> String {
    let mut shown = vec![config.command.clone()];
    let args = &config.args;
    for (i, arg) in args.iter().enumerate() {
        let names_id = arg.contains(SESSION_ID);
        let flag_for_id =
            arg.starts_with('-') && args.get(i + 1).is_some_and(|n| n.contains(SESSION_ID));
        if !names_id && !flag_for_id {
            shown.push(arg.clone());
        }
    }
    shown.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(p: &LaunchPicker) -> Vec<(String, String)> {
        p.picker()
            .rows()
            .map(|(i, _)| (i.label.clone(), i.detail.clone()))
            .collect()
    }

    #[test]
    fn rows_show_each_agent_with_its_command() {
        let p = LaunchPicker::new(&AgentConfig::builtin());
        assert_eq!(
            rows(&p),
            [
                ("✻ Claude Code".to_owned(), "claude".to_owned()),
                ("⌬ Codex".to_owned(), "codex".to_owned()),
            ]
        );
    }

    #[test]
    fn a_configured_wrapper_shows_and_an_empty_command_falls_back() {
        let mut agents = AgentConfig::builtin();
        let claude = agents.get_mut("claude").expect("claude");
        claude.command = "/opt/my-claude".into();
        claude.args = vec!["--model".into(), "opus".into()];
        agents.get_mut("codex").expect("codex").command = "  ".into();
        let p = LaunchPicker::new(&agents);
        let details: Vec<_> = rows(&p).into_iter().map(|(_, d)| d).collect();
        assert_eq!(details, ["/opt/my-claude --model opus", "codex"]);
    }

    #[test]
    fn the_session_id_and_its_flag_are_left_out_of_the_command_line() {
        let mut config = AgentConfig::for_kind(AgentKind::Claude);
        config.args = vec![
            "--model".into(),
            "opus".into(),
            "--session-id".into(),
            SESSION_ID.into(),
        ];
        assert_eq!(command_line(&config), "claude --model opus");
        config.args = vec!["--id={session_id}".into(), "-v".into()];
        assert_eq!(command_line(&config), "claude -v");
    }

    #[test]
    fn enter_launches_the_highlighted_agent_and_esc_closes() {
        let mut p = LaunchPicker::new(&AgentConfig::builtin());
        assert_eq!(p.key(OverlayKey::Down), LaunchStep::Open);
        assert_eq!(
            p.key(OverlayKey::Enter),
            LaunchStep::Launch(AgentKind::Codex)
        );
        let mut p = LaunchPicker::new(&AgentConfig::builtin());
        assert_eq!(
            p.key(OverlayKey::Enter),
            LaunchStep::Launch(AgentKind::Claude)
        );
        assert_eq!(p.key(OverlayKey::Esc), LaunchStep::Close);
    }

    #[test]
    fn typing_narrows_and_enter_on_no_match_stays_open() {
        let mut p = LaunchPicker::new(&AgentConfig::builtin());
        p.key(OverlayKey::Char('x'));
        assert_eq!(rows(&p).len(), 1, "only Codex has an x");
        assert_eq!(
            p.key(OverlayKey::Enter),
            LaunchStep::Launch(AgentKind::Codex)
        );
        let mut p = LaunchPicker::new(&AgentConfig::builtin());
        for c in "zz".chars() {
            p.key(OverlayKey::Char(c));
        }
        assert!(rows(&p).is_empty());
        assert_eq!(p.key(OverlayKey::Enter), LaunchStep::Open);
    }
}
