//! The format's limits on how big a manifest's gates may get, so a
//! downloaded or edited manifest can't make detection slow.

use super::GateSpec;
use crate::DetectError;

const MAX_DEPTH: usize = 8;
const MAX_GATES: usize = 512;
const MAX_MATCHERS_PER_GATE: usize = 32;
const MAX_MATCHERS: usize = 1024;
const MAX_MATCHER_CHARS: usize = 512;

/// What one manifest's gates have used so far.
#[derive(Debug, Default)]
pub(crate) struct Budget {
    gates: usize,
    matchers: usize,
}

impl Budget {
    /// Counts the gate `spec` at `depth`, failing past any limit.
    pub(crate) fn enter(&mut self, spec: &GateSpec, depth: usize) -> Result<(), DetectError> {
        if depth > MAX_DEPTH {
            return Err(over(&format!("gates nest deeper than {MAX_DEPTH}")));
        }
        self.gates += 1;
        if self.gates > MAX_GATES {
            return Err(over(&format!("more than {MAX_GATES} gates")));
        }
        let direct = [&spec.contains, &spec.regex, &spec.line_regex];
        let count: usize = direct.iter().map(|m| m.len()).sum();
        if count > MAX_MATCHERS_PER_GATE {
            return Err(over(&format!(
                "a gate has {count} matchers, max {MAX_MATCHERS_PER_GATE}"
            )));
        }
        self.matchers += count;
        if self.matchers > MAX_MATCHERS {
            return Err(over(&format!("more than {MAX_MATCHERS} matchers")));
        }
        let long = direct.iter().flat_map(|m| m.iter());
        if long
            .into_iter()
            .any(|m| m.chars().count() > MAX_MATCHER_CHARS)
        {
            return Err(over(&format!(
                "a matcher is longer than {MAX_MATCHER_CHARS} characters"
            )));
        }
        Ok(())
    }
}

fn over(what: &str) -> DetectError {
    DetectError::Invalid(format!("manifest too large: {what}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(contains: usize, chars: usize) -> GateSpec {
        GateSpec {
            contains: vec!["x".repeat(chars); contains],
            ..GateSpec::default()
        }
    }

    #[test]
    fn limits_are_enforced() {
        let mut budget = Budget::default();
        assert!(budget.enter(&spec(32, 512), 8).is_ok());
        assert!(budget.enter(&spec(1, 1), 9).is_err(), "too deep");
        assert!(budget.enter(&spec(33, 1), 0).is_err(), "too many in one");
        assert!(budget.enter(&spec(1, 513), 0).is_err(), "too long");
        let mut budget = Budget::default();
        let errs = (0..40).filter(|_| budget.enter(&spec(32, 1), 0).is_err());
        assert_eq!(errs.count(), 8, "1024 matchers in all");
        let mut budget = Budget::default();
        let many = (0..513).filter(|_| budget.enter(&spec(0, 0), 0).is_err());
        assert_eq!(many.count(), 1, "512 gates in all");
    }
}
