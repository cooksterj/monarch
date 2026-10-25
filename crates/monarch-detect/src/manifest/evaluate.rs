//! Running a manifest on a screen: every rule reads its region, and the
//! highest-priority match wins (the earliest one on a tie).

use super::Manifest;
use crate::{DetectionInput, RuleCheck, Verdict};

impl Manifest {
    /// What the screen in `input` says the agent is doing.
    #[must_use]
    pub fn evaluate(&self, input: DetectionInput<'_>) -> Verdict<'_> {
        let matched = self
            .rules
            .iter()
            .filter(|rule| rule.gate.matches(rule.region.slice(input)));
        best(matched)
    }

    /// The verdict with every rule's outcome, for `monarch agent explain`.
    #[must_use]
    pub fn explain(&self, input: DetectionInput<'_>) -> (Verdict<'_>, Vec<RuleCheck<'_>>) {
        let checks: Vec<RuleCheck<'_>> = self
            .rules
            .iter()
            .map(|rule| {
                let region = rule.region.slice(input);
                RuleCheck::new(rule, region, rule.gate.matches(region))
            })
            .collect();
        let verdict = best(checks.iter().filter(|c| c.matched).map(|c| c.rule));
        (verdict, checks)
    }
}

/// The first rule with the highest priority.
fn best<'m>(matched: impl Iterator<Item = &'m crate::Rule>) -> Verdict<'m> {
    matched
        .fold(None, |won: Option<&crate::Rule>, rule| match won {
            Some(won) if won.priority >= rule.priority => Some(won),
            _ => Some(rule),
        })
        .map_or(Verdict::NoMatch, Verdict::Rule)
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentState;

    use super::*;

    const MANIFEST: &str = r#"id = "t"
[[rules]]
id = "first_working"
state = "working"
priority = 10
contains = ["esc to interrupt"]
[[rules]]
id = "tied_idle"
state = "idle"
priority = 10
contains = ["esc"]
[[rules]]
id = "blocked"
state = "blocked"
priority = 20
region = "bottom_non_empty_lines(1)"
contains = ["proceed?"]
[[rules]]
id = "title"
state = "working"
priority = 30
region = "osc_title"
regex = ['^⠋']"#;

    fn verdict(input: DetectionInput<'_>) -> Option<String> {
        let manifest = Manifest::parse(MANIFEST).expect("valid");
        manifest.evaluate(input).rule_id().map(Into::into)
    }

    #[test]
    fn the_highest_priority_wins_and_ties_keep_the_first() {
        let screen = |s| DetectionInput::screen(s);
        assert_eq!(
            verdict(screen("esc to interrupt")).as_deref(),
            Some("first_working")
        );
        assert_eq!(
            verdict(screen("esc to interrupt\nproceed?")).as_deref(),
            Some("blocked")
        );
        assert_eq!(
            verdict(screen("proceed?\nlater")),
            None,
            "not in its region"
        );
        let titled = DetectionInput {
            osc_title: "⠋ task",
            ..screen("proceed?")
        };
        assert_eq!(verdict(titled).as_deref(), Some("title"));
    }

    #[test]
    fn explain_shows_every_rule_and_agrees() {
        let manifest = Manifest::parse(MANIFEST).expect("valid");
        let input = DetectionInput::screen("esc to interrupt");
        let (verdict, checks) = manifest.explain(input);
        assert_eq!(verdict.rule_id(), manifest.evaluate(input).rule_id());
        let matched: Vec<_> = checks
            .iter()
            .map(|c| (c.rule.id.as_str(), c.matched))
            .collect();
        assert_eq!(
            matched,
            [
                ("first_working", true),
                ("tied_idle", true),
                ("blocked", false),
                ("title", false)
            ]
        );
        let (none, _) = manifest.explain(DetectionInput::screen("❯"));
        assert_eq!(none.state(), Some(AgentState::Idle));
    }
}
