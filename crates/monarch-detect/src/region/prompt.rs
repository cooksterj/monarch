//! Regions found by the prompt's shape: Codex's `›` prompt and the block
//! markers above it, and the `─`-ruled input box of Claude-style agents.

use super::lines::{from_line, is_rule, line_start};

/// After the last `›` prompt line (all of it if none).
pub(super) fn after_last_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    match lines.iter().rposition(|line| is_prompt(line)) {
        Some(index) => from_line(content, &lines, index + 1),
        None => content,
    }
}

/// Before the current prompt (all of it if none).
pub(super) fn before_current(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    match current_prompt(&lines) {
        Some(index) => &content[..line_start(content, &lines, index)],
        None => content,
    }
}

/// The screen, or nothing while a prompt is current.
pub(super) fn unless_current(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    if current_prompt(&lines).is_some() {
        ""
    } else {
        content
    }
}

/// The last block marker line above the current prompt.
pub(super) fn block_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    current_prompt(&lines)
        .and_then(|prompt| lines[..prompt].iter().rev().find(|l| is_block(l)))
        .copied()
        .unwrap_or("")
}

/// From that block marker line to the end.
pub(super) fn after_block_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let block =
        current_prompt(&lines).and_then(|prompt| lines[..prompt].iter().rposition(|l| is_block(l)));
    block.map_or("", |block| from_line(content, &lines, block))
}

/// Inside the prompt box: after its top rule, up to the next rule.
pub(super) fn box_body(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(top) = box_top(&lines) else {
        return "";
    };
    let end = lines[top + 1..]
        .iter()
        .position(|line| is_rule(line))
        .map_or(lines.len(), |rel| top + 1 + rel);
    let start = line_start(content, &lines, top + 1);
    &content[start..line_start(content, &lines, end).max(start)]
}

/// Everything above the prompt box (all of it if none).
pub(super) fn above_box(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    match box_top(&lines) {
        Some(top) => &content[..line_start(content, &lines, top)],
        None => content,
    }
}

/// The prompt line that is current: the last one, unless a block marker
/// was written after it (the agent has moved on).
fn current_prompt(lines: &[&str]) -> Option<usize> {
    let prompt = lines.iter().rposition(|line| is_prompt(line))?;
    let moved_on = lines[prompt + 1..].iter().any(|line| is_block(line));
    (!moved_on).then_some(prompt)
}

/// The box's top border: the second rule line from the bottom.
fn box_top(lines: &[&str]) -> Option<usize> {
    let mut rules = (0..lines.len()).rev().filter(|&i| is_rule(lines[i]));
    rules.nth(1)
}

fn is_prompt(line: &str) -> bool {
    line == "›" || line.starts_with("› ")
}

fn is_block(line: &str) -> bool {
    line.starts_with(['•', '■', '✗', '✓'])
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX: &str = "• Ran tests\n  ok\n› fix it\nfooter\n";

    #[test]
    fn codex_regions_follow_the_current_prompt() {
        assert_eq!(after_last_marker(CODEX), "footer\n");
        assert_eq!(before_current(CODEX), "• Ran tests\n  ok\n");
        assert_eq!(unless_current(CODEX), "");
        assert_eq!(block_marker(CODEX), "• Ran tests");
        assert_eq!(after_block_marker(CODEX), CODEX);
    }

    #[test]
    fn a_block_after_the_prompt_means_no_current_prompt() {
        let busy = "› fix it\n• Working\n";
        assert_eq!(before_current(busy), busy);
        assert_eq!(unless_current(busy), busy);
        assert_eq!(block_marker(busy), "");
        assert_eq!(after_block_marker(busy), "");
        assert_eq!(after_last_marker("no prompt"), "no prompt");
        assert!(is_prompt("›") && !is_prompt("›x"));
    }

    #[test]
    fn the_box_is_between_the_last_two_rules() {
        let screen = "answer\n───\n> typed\n───\n? for shortcuts\n";
        assert_eq!(box_body(screen), "> typed\n");
        assert_eq!(above_box(screen), "answer\n");
        let open = "answer\n───\n> typed\n";
        assert_eq!(box_body(open), "", "one rule is no box");
        assert_eq!(above_box(open), open);
    }
}
