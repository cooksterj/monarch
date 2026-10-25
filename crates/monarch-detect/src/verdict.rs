//! What a manifest concluded about a screen.

use monarch_core::AgentState;

use crate::Rule;

/// The rule that won, or none.
#[derive(Debug, Clone, Copy)]
pub enum Verdict<'m> {
    /// This rule matched with the highest priority.
    Rule(&'m Rule),
    /// No rule matched: an agent showing nothing recognisable is idle.
    NoMatch,
}

impl Verdict<'_> {
    /// The detected state; `None` when the winning rule says to keep the
    /// current one.
    #[must_use]
    pub fn state(&self) -> Option<AgentState> {
        match self {
            Self::Rule(rule) if rule.hold => None,
            Self::Rule(rule) => Some(rule.state),
            Self::NoMatch => Some(AgentState::Idle),
        }
    }

    /// Whether the screen visibly shows the state (strong evidence).
    #[must_use]
    pub fn visible(&self) -> bool {
        matches!(self, Self::Rule(rule) if rule.visible)
    }

    /// The winning rule's id.
    #[must_use]
    pub fn rule_id(&self) -> Option<&str> {
        match self {
            Self::Rule(rule) => Some(&rule.id),
            Self::NoMatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Manifest;

    #[test]
    fn hold_rules_keep_the_state_and_no_match_is_idle() {
        let manifest = Manifest::parse(
            r#"id = "t"
[[rules]]
id = "viewer"
state = "unknown"
skip_state_update = true
contains = ["transcript"]
[[rules]]
id = "ask"
state = "blocked"
visible_blocker = true
contains = ["proceed?"]"#,
        )
        .expect("valid");
        let hold = Verdict::Rule(&manifest.rules[0]);
        assert_eq!((hold.state(), hold.visible()), (None, false));
        let ask = Verdict::Rule(&manifest.rules[1]);
        assert_eq!(
            (ask.state(), ask.visible()),
            (Some(AgentState::Blocked), true)
        );
        assert_eq!(ask.rule_id(), Some("ask"));
        let none = Verdict::NoMatch;
        assert_eq!(
            (none.state(), none.rule_id()),
            (Some(AgentState::Idle), None)
        );
    }
}
