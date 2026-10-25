//! A rule's test on its region's text: literal and regex matchers, all of
//! which must hold, combined with nested `all`, `any` and `not` gates.

mod budget;

use regex::Regex;
use serde::Deserialize;

pub(crate) use budget::Budget;

use crate::DetectError;

/// A gate as written in a manifest.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GateSpec {
    #[serde(default)]
    pub all: Vec<GateSpec>,
    #[serde(default)]
    pub any: Vec<GateSpec>,
    #[serde(default, rename = "not")]
    pub not: Vec<GateSpec>,
    #[serde(default)]
    pub contains: Vec<String>,
    #[serde(default)]
    pub regex: Vec<String>,
    #[serde(default)]
    pub line_regex: Vec<String>,
}

impl GateSpec {
    /// Whether it matches something itself (a `not` alone cannot).
    fn positive(&self) -> bool {
        !(self.contains.is_empty()
            && self.regex.is_empty()
            && self.line_regex.is_empty()
            && self.all.is_empty()
            && self.any.is_empty())
    }
}

/// A compiled gate. It holds when every `contains` (case-insensitive)
/// and `regex` matches the text, every `line_regex` matches some line,
/// every `all` gate holds, some `any` gate holds (if there are any), and
/// no `not` gate holds.
#[derive(Debug, Clone)]
pub struct Gate {
    all: Vec<Gate>,
    any: Vec<Gate>,
    not: Vec<Gate>,
    contains: Vec<String>,
    regex: Vec<Regex>,
    line_regex: Vec<Regex>,
}

impl Gate {
    /// Checks `spec` against the format's limits and compiles it; a gate
    /// under a `not` (`negated`) may consist of nested `not`s alone.
    pub(crate) fn compile(
        spec: &GateSpec,
        budget: &mut Budget,
        depth: usize,
        negated: bool,
    ) -> Result<Self, DetectError> {
        budget.enter(spec, depth)?;
        if !spec.positive() && !(negated && !spec.not.is_empty()) {
            return Err(DetectError::Invalid("a gate needs a matcher".into()));
        }
        let nested = |specs: &[GateSpec], negated: bool, budget: &mut Budget| {
            specs
                .iter()
                .map(|s| Self::compile(s, budget, depth + 1, negated))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            all: nested(&spec.all, false, budget)?,
            any: nested(&spec.any, false, budget)?,
            not: nested(&spec.not, true, budget)?,
            contains: spec.contains.iter().map(|s| s.to_lowercase()).collect(),
            regex: compile_all(&spec.regex)?,
            line_regex: compile_all(&spec.line_regex)?,
        })
    }

    /// Whether the gate holds for `text`.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        self.holds(text, &text.to_lowercase())
    }

    fn holds(&self, text: &str, lower: &str) -> bool {
        self.contains
            .iter()
            .all(|needle| lower.contains(needle.as_str()))
            && self.regex.iter().all(|re| re.is_match(text))
            && self
                .line_regex
                .iter()
                .all(|re| text.lines().any(|line| re.is_match(line)))
            && self.all.iter().all(|g| g.holds(text, lower))
            && (self.any.is_empty() || self.any.iter().any(|g| g.holds(text, lower)))
            && !self.not.iter().any(|g| g.holds(text, lower))
    }
}

fn compile_all(patterns: &[String]) -> Result<Vec<Regex>, DetectError> {
    patterns
        .iter()
        .map(|p| Regex::new(p).map_err(|e| DetectError::Invalid(format!("bad regex {p:?}: {e}"))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(toml: &str) -> Result<Gate, DetectError> {
        let spec: GateSpec = toml::from_str(toml).map_err(DetectError::from)?;
        Gate::compile(&spec, &mut Budget::default(), 0, false)
    }

    #[test]
    fn matchers_combine_with_and() {
        let g = gate(
            r#"contains = ["Proceed?"]
line_regex = ['^\s*1\. Yes$']"#,
        )
        .expect("valid");
        assert!(g.matches("do you want to PROCEED?\n  1. Yes\n"));
        assert!(
            !g.matches("proceed?\n  1. Yes, always"),
            "no line is exactly it"
        );
        assert!(!g.matches("  1. Yes"));
    }

    #[test]
    fn nested_gates_any_all_and_not() {
        let g = gate(
            r#"any = [{ contains = ["esc to interrupt"] }, { regex = ['\d+s'] }]
not = [{ contains = ["proceed?"] }]"#,
        )
        .expect("valid");
        assert!(g.matches("thinking (12s)"));
        assert!(g.matches("esc to interrupt"));
        assert!(!g.matches("esc to interrupt\nproceed?"), "not wins");
        assert!(!g.matches("idle"), "no any held");
    }

    #[test]
    fn empty_gates_and_bad_regexes_are_refused() {
        assert!(matches!(gate(""), Err(DetectError::Invalid(_))));
        assert!(
            gate(r#"not = [{ contains = ["x"] }]"#).is_err(),
            "nothing positive"
        );
        assert!(
            gate(
                r#"contains = ["x"]
not = [{ not = [{ contains = ["y"] }] }]"#
            )
            .is_ok()
        );
        let bad = gate("regex = ['(']").expect_err("refused").to_string();
        assert!(bad.starts_with("bad regex \"(\""), "{bad}");
        assert!(matches!(gate("bogus = 1"), Err(DetectError::Parse(_))));
    }
}
