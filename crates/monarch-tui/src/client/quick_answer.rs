//! What a key typed in the inbox sends to an agent's pane.
//!
//! Agents want different keys for the same answer: Claude Code's
//! permission prompt takes `1` for yes and Esc for no, Codex takes `y`
//! and `n`. `[agents.<id>].quick_answers` maps the key the user types to
//! what the agent expects; a key the table doesn't name, or a pane whose
//! agent is unknown, gets the key as typed.

use std::collections::BTreeMap;

use monarch_core::{AgentConfig, AgentKind, PaneId};

/// The bytes an inbox key sends, and how to say what was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickAnswer {
    /// The key the user typed.
    pub key: char,
    /// What goes to the pane.
    pub bytes: Vec<u8>,
    /// Whether the table mapped the key (else it is typed as it is).
    pub mapped: bool,
}

impl QuickAnswer {
    /// Resolves `key` for a pane running `kind`, through `agents`
    /// (`[agents.<id>]` by the agent's name).
    #[must_use]
    pub fn resolve(
        agents: &BTreeMap<String, AgentConfig>,
        kind: Option<AgentKind>,
        key: char,
    ) -> Self {
        let mapped = kind
            .and_then(|k| agents.get(k.name()))
            .and_then(|a| a.quick_answer(key));
        match mapped {
            Some(text) => Self {
                key,
                bytes: text.as_bytes().to_vec(),
                mapped: true,
            },
            None => Self {
                key,
                bytes: key.to_string().into_bytes(),
                mapped: false,
            },
        }
    }

    /// `sent y to pane 3`, or `answered y to pane 3 (sent 1)` when the
    /// table changed it.
    #[must_use]
    pub fn notice(&self, pane: PaneId) -> String {
        if self.mapped {
            format!(
                "answered {} to pane {} (sent {})",
                self.key,
                pane.0,
                describe(&self.bytes)
            )
        } else {
            format!("sent {} to pane {}", self.key, pane.0)
        }
    }
}

/// `bytes` as a person reads them: control keys by name, the rest as text.
fn describe(bytes: &[u8]) -> String {
    match bytes {
        b"\x1b" => "Esc".to_owned(),
        b"\r" | b"\n" => "Enter".to_owned(),
        b"\t" => "Tab".to_owned(),
        _ => String::from_utf8_lossy(bytes)
            .chars()
            .map(|c| if c.is_control() { '?' } else { c })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agents() -> BTreeMap<String, AgentConfig> {
        AgentConfig::builtin()
    }

    #[test]
    fn keys_map_per_agent_and_pass_through_otherwise() {
        let yes = QuickAnswer::resolve(&agents(), Some(AgentKind::Claude), 'y');
        assert_eq!((yes.bytes.as_slice(), yes.mapped), (b"1".as_slice(), true));
        assert_eq!(yes.notice(PaneId(3)), "answered y to pane 3 (sent 1)");
        let no = QuickAnswer::resolve(&agents(), Some(AgentKind::Claude), 'n');
        assert_eq!(no.bytes, b"\x1b");
        assert_eq!(no.notice(PaneId(3)), "answered n to pane 3 (sent Esc)");
        let codex = QuickAnswer::resolve(&agents(), Some(AgentKind::Codex), 'y');
        assert_eq!(
            (codex.bytes.as_slice(), codex.mapped),
            (b"y".as_slice(), true)
        );
        let two = QuickAnswer::resolve(&agents(), Some(AgentKind::Claude), '2');
        assert_eq!((two.bytes.as_slice(), two.mapped), (b"2".as_slice(), false));
        assert_eq!(two.notice(PaneId(5)), "sent 2 to pane 5");
        let unknown = QuickAnswer::resolve(&agents(), None, 'y');
        assert_eq!(
            (unknown.bytes.as_slice(), unknown.mapped),
            (b"y".as_slice(), false)
        );
        let none = QuickAnswer::resolve(&BTreeMap::new(), Some(AgentKind::Claude), 'y');
        assert!(!none.mapped, "no table for the agent: typed as it is");
    }

    #[test]
    fn a_users_mapping_wins_and_odd_bytes_are_named_or_hidden() {
        let mut agents = agents();
        let claude = agents.get_mut("claude").expect("claude");
        claude.quick_answers.insert("a".into(), "2\r".into());
        claude.quick_answers.insert("e".into(), "\r".into());
        let always = QuickAnswer::resolve(&agents, Some(AgentKind::Claude), 'a');
        assert_eq!(always.bytes, b"2\r");
        assert_eq!(always.notice(PaneId(1)), "answered a to pane 1 (sent 2?)");
        let enter = QuickAnswer::resolve(&agents, Some(AgentKind::Claude), 'e');
        assert_eq!(enter.notice(PaneId(1)), "answered e to pane 1 (sent Enter)");
    }
}
