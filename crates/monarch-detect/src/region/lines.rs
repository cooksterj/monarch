//! Regions measured in lines: the last or first few, the text after a
//! horizontal rule. Every function returns a slice of its input.

/// The last `count` lines.
pub(super) fn bottom(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    from_line(content, &lines, lines.len().saturating_sub(count))
}

/// From the `count`th-last non-blank line to the end.
pub(super) fn bottom_non_empty(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, line)| !line.trim().is_empty())
        .take(count)
        .last()
        .map(|(index, _)| index);
    start.map_or("", |start| from_line(content, &lines, start))
}

/// From the start through the `count`th non-blank line.
pub(super) fn top_non_empty(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let end = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .take(count)
        .last()
        .map(|(index, _)| index);
    end.map_or("", |end| &content[..line_start(content, &lines, end + 1)])
}

/// After the last horizontal rule line (all of it if none).
pub(super) fn after_last_rule(content: &str) -> &str {
    let mut end = 0;
    let mut offset = 0;
    for line in content.lines() {
        let next = offset + line.len() + 1;
        if is_rule(line) {
            end = next.min(content.len());
        }
        offset = next;
    }
    &content[end..]
}

/// The last non-blank line, or nothing.
pub(super) fn last_non_empty(content: &str) -> &str {
    content
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
}

/// A line drawn with `─`: only rule characters, or at least three of
/// them followed by a label.
pub(super) fn is_rule(line: &str) -> bool {
    let trimmed = line.trim();
    let count = trimmed.chars().take_while(|&c| c == '─').count();
    if count == 0 {
        return false;
    }
    let rest = &trimmed[count * '─'.len_utf8()..];
    rest.trim_start().is_empty() || count >= 3
}

/// From line `index` (of `lines`, split from `content`) to the end.
pub(super) fn from_line<'a>(content: &'a str, lines: &[&str], index: usize) -> &'a str {
    &content[line_start(content, lines, index)..]
}

/// The byte offset where line `index` starts (the end, past the last).
pub(super) fn line_start(content: &str, lines: &[&str], index: usize) -> usize {
    lines[..index.min(lines.len())]
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>()
        .min(content.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: &str = "a\n\nb\nc\n\n";

    #[test]
    fn bottom_regions_count_from_the_end() {
        assert_eq!(bottom(SCREEN, 2), "c\n\n");
        assert_eq!(bottom(SCREEN, 99), SCREEN);
        assert_eq!(bottom_non_empty(SCREEN, 2), "b\nc\n\n");
        assert_eq!(bottom_non_empty("\n \n", 2), "", "nothing written");
        let repeated = "same\nx\nsame\n";
        assert_eq!(bottom_non_empty(repeated, 1), "same\n", "the bottom one");
    }

    #[test]
    fn top_regions_count_from_the_start() {
        assert_eq!(top_non_empty(SCREEN, 2), "a\n\nb\n");
        assert_eq!(top_non_empty("\n", 1), "");
        assert_eq!(top_non_empty("only", 1), "only", "no newline at the end");
    }

    #[test]
    fn rules_are_runs_of_box_lines() {
        assert!(is_rule("  ─────  "));
        assert!(is_rule("─"));
        assert!(is_rule("─── Plan ───"));
        assert!(!is_rule("─ x"), "a dash before text is a bullet");
        assert!(!is_rule("text ───"));
        assert!(!is_rule("   "));
        let boxed = "out\n───\n> typed\n";
        assert_eq!(after_last_rule(boxed), "> typed\n");
        assert_eq!(after_last_rule("no rules"), "no rules");
        assert_eq!(last_non_empty("a\nb\n \n"), "b");
    }
}
