//! One manifest rule: when its gate holds on its region, the agent is in
//! its state, at its priority.

use monarch_core::AgentState;
use serde::Deserialize;

use crate::gate::{Budget, GateSpec};
use crate::{DetectError, Gate, Region};

/// A rule as written: its own fields, then its gate's matchers inline.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuleSpec {
    id: String,
    state: Option<StateSpec>,
    #[serde(default)]
    priority: i32,
    region: Option<String>,
    #[serde(default)]
    visible_idle: bool,
    #[serde(default)]
    visible_blocker: bool,
    #[serde(default)]
    visible_working: bool,
    #[serde(default)]
    skip_state_update: bool,
    #[serde(default)]
    all: Vec<GateSpec>,
    #[serde(default)]
    any: Vec<GateSpec>,
    #[serde(default, rename = "not")]
    not: Vec<GateSpec>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}

/// The states a manifest may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StateSpec {
    Idle,
    Working,
    Blocked,
    Unknown,
}

impl From<StateSpec> for AgentState {
    fn from(state: StateSpec) -> Self {
        match state {
            StateSpec::Idle => Self::Idle,
            StateSpec::Working => Self::Working,
            StateSpec::Blocked => Self::Blocked,
            StateSpec::Unknown => Self::Unknown,
        }
    }
}

/// A compiled rule.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Its name, shown by `explain`.
    pub id: String,
    /// The state it detects (`Unknown` for a hold rule).
    pub state: AgentState,
    /// Higher wins; on a tie the earlier rule wins.
    pub priority: i32,
    /// What it reads.
    pub region: Region,
    /// Whether a match is visible proof of its state (an on-screen
    /// prompt), strong enough to override a hook's report.
    pub visible: bool,
    /// Whether a match means "can't tell now, keep the state" (a
    /// transcript viewer covering the screen).
    pub hold: bool,
    /// The test on the region's text.
    pub gate: Gate,
}

impl Rule {
    /// Validates and compiles `spec`.
    pub(crate) fn compile(spec: RuleSpec, budget: &mut Budget) -> Result<Self, DetectError> {
        let fail = |why: String| DetectError::Invalid(format!("rule {}: {why}", spec.id));
        if spec.id.trim().is_empty() {
            return Err(DetectError::Invalid("a rule has no id".into()));
        }
        let state = spec.state.map_or(AgentState::Unknown, AgentState::from);
        let visible = match state {
            AgentState::Idle => spec.visible_idle,
            AgentState::Working => spec.visible_working,
            AgentState::Blocked => spec.visible_blocker,
            _ => false,
        };
        let any_visible = spec.visible_idle || spec.visible_blocker || spec.visible_working;
        if spec.skip_state_update && (spec.state != Some(StateSpec::Unknown) || any_visible) {
            return Err(fail(
                "skip_state_update needs state \"unknown\" and no visible_*".into(),
            ));
        }
        let region = match &spec.region {
            Some(name) => name
                .parse()
                .map_err(|n| fail(format!("unknown region {n:?}")))?,
            None => Region::WholeRecent,
        };
        let gate =
            Gate::compile(&spec.gate(), budget, 0, false).map_err(|e| fail(e.to_string()))?;
        Ok(Self {
            id: spec.id,
            state,
            priority: spec.priority,
            region,
            visible,
            hold: spec.skip_state_update,
            gate,
        })
    }
}

impl RuleSpec {
    /// The matchers written inline, as a gate.
    fn gate(&self) -> GateSpec {
        GateSpec {
            all: self.all.clone(),
            any: self.any.clone(),
            not: self.not.clone(),
            contains: self.contains.clone(),
            regex: self.regex.clone(),
            line_regex: self.line_regex.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(toml: &str) -> Result<Rule, DetectError> {
        let spec: RuleSpec = toml::from_str(toml)?;
        Rule::compile(spec, &mut Budget::default())
    }

    #[test]
    fn fields_default_and_visibility_follows_the_state() {
        let r = rule(
            r#"id = "a"
state = "blocked"
visible_blocker = true
visible_working = true
contains = ["x"]"#,
        )
        .expect("valid");
        assert_eq!(
            (r.state, r.priority, r.region, r.visible, r.hold),
            (AgentState::Blocked, 0, Region::WholeRecent, true, false)
        );
        let none = rule("id = \"b\"\ncontains = [\"x\"]").expect("valid");
        assert_eq!(none.state, AgentState::Unknown, "no state named");
        let idle = rule("id = \"c\"\nstate = \"idle\"\nvisible_working = true\ncontains = [\"x\"]");
        assert!(
            !idle.expect("valid").visible,
            "visible_working on an idle rule"
        );
    }

    #[test]
    fn hold_rules_must_stay_neutral() {
        let hold = "id = \"h\"\nstate = \"unknown\"\nskip_state_update = true\ncontains = [\"x\"]";
        assert!(rule(hold).expect("valid").hold);
        let working = hold.replace("unknown", "working");
        assert!(rule(&working).is_err());
        let shown = format!("{hold}\nvisible_idle = true");
        assert!(rule(&shown).is_err());
    }

    #[test]
    fn bad_rules_say_which_rule() {
        let err = rule("id = \"r\"\nregion = \"top\"\ncontains = [\"x\"]").expect_err("refused");
        assert_eq!(err.to_string(), "rule r: unknown region \"top\"");
        let err = rule("id = \"r\"").expect_err("refused");
        assert_eq!(err.to_string(), "rule r: a gate needs a matcher");
        assert!(rule("id = \" \"\ncontains = [\"x\"]").is_err());
        assert!(rule("id = \"r\"\nstate = \"done\"\ncontains = [\"x\"]").is_err());
        assert!(rule("id = \"r\"\ncolour = 1\ncontains = [\"x\"]").is_err());
    }
}
