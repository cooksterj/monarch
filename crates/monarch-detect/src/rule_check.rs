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
    /// The end of the text it read (agents draw prompts at the bottom),
    /// starting with `…` when cut.
    pub preview: String,
}

impl<'m> RuleCheck<'m> {
    /// The outcome of `rule` on `region` (its text).
    #[must_use]
    pub fn new(rule: &'m Rule, region: &str, matched: bool) -> Self {
        let region = region.trim_end();
        let skip = region.chars().count().saturating_sub(PREVIEW_CHARS);
        let mut preview: String = region.chars().skip(skip).collect();
        if skip > 0 {
            preview.insert(0, '…');
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
        assert_eq!(RuleCheck::new(rule, "short\n\n", false).preview, "short");
        let long = format!("{}{}", "a".repeat(300), "é".repeat(10));
        let cut = RuleCheck::new(rule, &long, true).preview;
        assert_eq!(cut.chars().count(), PREVIEW_CHARS + 1);
        assert!(
            cut.starts_with('…') && cut.ends_with("éé"),
            "the bottom is kept"
        );
    }
}
