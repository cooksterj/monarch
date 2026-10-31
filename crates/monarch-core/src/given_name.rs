//! Names the user gives tabs and panes: one tidy line.

/// Characters a given name keeps; the rest is cut off.
const MAX: usize = 40;

/// `text` as a name: control characters become spaces, the ends are
/// trimmed and at most 40 characters are kept. `None` when nothing is
/// left, which means "no name: use the automatic one".
#[must_use]
pub fn given_name(text: &str) -> Option<String> {
    let line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let kept: String = line.trim().chars().take(MAX).collect();
    Some(kept.trim_end().to_owned()).filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(given_name("  api\nwork\x1b ").as_deref(), Some("api work"));
        assert_eq!(given_name(&"x".repeat(99)).map(|n| n.len()), Some(MAX));
        assert_eq!(
            given_name(&format!("{} y", "x".repeat(39))).as_deref(),
            Some("x".repeat(39).as_str())
        );
    }

    #[test]
    fn blank_means_no_name() {
        assert_eq!(given_name("   "), None);
        assert_eq!(given_name(""), None);
        assert_eq!(given_name("\t\r"), None);
    }
}
