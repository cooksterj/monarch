//! One line of `monarch agent explain`: a rule, whether it matched, and
//! the text it read.

use crate::Rule;

/// How much of a region `explain` shows.
const PREVIEW_CHARS: usize = 240;

/// A rule's outcome on one screen.
#[derive(Debug, Clone)]
pub struct RuleCheck<'m> {
    /// The rule.
    pub rule: &'m Rule,
    /// Whether its gate held.
    pub matched: bool,
    /// The start of the text it read, `…` when cut.
    pub preview: String,
}

impl<'m> RuleCheck<'m> {
    /// The outcome of `rule` on `region` (its text).
    #[must_use]
    pub fn new(rule: &'m Rule, region: &str, matched: bool) -> Self {
        let mut preview: String = region.chars().take(PREVIEW_CHARS).collect();
        if preview.len() < region.len() {
            preview.push('…');
        }
        Self {
            rule,
            matched,
            preview,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Manifest;

    #[test]
    fn previews_are_bounded() {
        let manifest = Manifest::parse("id = \"t\"\n[[rules]]\nid = \"r\"\ncontains = [\"x\"]")
            .expect("valid");
        let rule = &manifest.rules[0];
        assert_eq!(RuleCheck::new(rule, "short", false).preview, "short");
        let long = "é".repeat(300);
        let cut = RuleCheck::new(rule, &long, true).preview;
        assert_eq!(cut.chars().count(), PREVIEW_CHARS + 1);
        assert!(cut.ends_with('…'));
    }
}
