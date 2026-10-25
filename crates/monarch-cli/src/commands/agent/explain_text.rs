//! How `monarch agent explain` prints its answer: one line per pane, or
//! one pane in full with every rule of its agent's manifest.

use std::fmt::Write;

use monarch_core::AgentState;
use monarch_proto::{PaneExplanation, RuleOutcome};

/// Longest rule preview printed, in characters.
const PREVIEW: usize = 100;

/// `p3  tab 2  gemini  ● Working  (its screen: esc_cancel_working)`.
pub(super) fn list(panes: &[PaneExplanation]) -> String {
    if panes.is_empty() {
        return "no panes\n".to_owned();
    }
    let width = panes
        .iter()
        .map(|p| program(p).chars().count())
        .max()
        .unwrap_or(0);
    let mut text = String::new();
    for pane in panes {
        let id = pane.pane.to_string();
        let _ = write!(
            text,
            "{id:<4} tab {}  {:<width$}  ",
            pane.tab,
            program(pane)
        );
        if pane.status.state == AgentState::Unknown && pane.manifest.is_none() {
            text.push_str("$ not an agent\n");
            continue;
        }
        let rule = pane
            .winner
            .as_deref()
            .map(|r| format!(": {r}"))
            .unwrap_or_default();
        let _ = writeln!(
            text,
            "{}  (from {}{rule})",
            state(pane.status.state),
            pane.source.describe()
        );
    }
    text.push_str("\nshow one pane's rules with `monarch agent explain <pane>`\n");
    text
}

/// One pane: its status and source, then its manifest's rules, the
/// winner marked `▶` and other matches `✓`, with what matched rules read.
pub(super) fn detail(pane: &PaneExplanation) -> String {
    let mut text = format!(
        "pane {} (tab {}) · {}\n",
        pane.pane,
        pane.tab,
        program(pane)
    );
    let _ = writeln!(
        text,
        "status    {}, from {}",
        state(pane.status.state),
        pane.source.describe()
    );
    if let Some(activity) = &pane.status.activity {
        let _ = writeln!(text, "activity  {activity}");
    }
    let Some(manifest) = &pane.manifest else {
        let _ = writeln!(
            text,
            "screen    no detection manifest for {}",
            program(pane)
        );
        return text;
    };
    let shows = match (pane.screen_state, pane.winner.as_deref()) {
        (Some(s), Some(rule)) => format!("{} (rule {rule})", state(s)),
        (Some(s), None) => format!("{} (no rule matched)", state(s)),
        (None, rule) => format!("unchanged (rule {} keeps the state)", rule.unwrap_or("?")),
    };
    let _ = writeln!(text, "manifest  {manifest}\nscreen    {shows}\n");
    let id_width = pane
        .rules
        .iter()
        .map(|r| r.id.chars().count())
        .max()
        .unwrap_or(0);
    for rule in &pane.rules {
        let won = pane.winner.as_deref() == Some(rule.id.as_str());
        text.push_str(&rule_line(rule, won, id_width));
    }
    text
}

fn rule_line(rule: &RuleOutcome, won: bool, width: usize) -> String {
    let mark = match (won, rule.matched) {
        (true, _) => '▶',
        (false, true) => '✓',
        (false, false) => '·',
    };
    let does = match rule.state {
        _ if rule.hold => "hold".to_owned(),
        AgentState::Unknown => "unknown".to_owned(),
        state => state.label().to_lowercase(),
    };
    let mut line = format!(
        "{mark} {:<width$}  {does:<8} {:>5}  {}\n",
        rule.id, rule.priority, rule.region
    );
    if rule.matched {
        let _ = writeln!(line, "      read: {}", preview(&rule.preview));
    }
    line
}

/// The end of the region's text on one line, at most [`PREVIEW`]
/// characters.
fn preview(text: &str) -> String {
    let flat: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let flat = flat.join(" ⏎ ");
    let skip = flat.chars().count().saturating_sub(PREVIEW);
    let cut: String = flat.chars().skip(skip).collect();
    if skip > 0 { format!("…{cut}") } else { cut }
}

fn state(state: AgentState) -> String {
    format!("{} {}", state.glyph(), state.label())
}

fn program(pane: &PaneExplanation) -> &str {
    pane.program.as_deref().unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{AgentStatus, StatusSource};

    use super::*;

    fn rule(id: &str, state: AgentState, matched: bool) -> RuleOutcome {
        RuleOutcome {
            id: id.into(),
            state,
            priority: 900,
            region: "whole_recent".into(),
            matched,
            hold: false,
            preview: "line one\n\n  line two\n".into(),
        }
    }

    fn gemini() -> PaneExplanation {
        PaneExplanation {
            pane: PaneId(3),
            tab: 2,
            program: Some("gemini".into()),
            status: AgentStatus {
                state: AgentState::Blocked,
                ..AgentStatus::default()
            },
            source: StatusSource::Screen,
            manifest: Some("gemini 2026.1".into()),
            screen_state: Some(AgentState::Blocked),
            winner: Some("ask".into()),
            rules: vec![
                rule("ask", AgentState::Blocked, true),
                rule("busy", AgentState::Working, true),
                rule("menu", AgentState::Unknown, false),
            ],
        }
    }

    fn shell() -> PaneExplanation {
        PaneExplanation {
            pane: PaneId(1),
            tab: 1,
            program: Some("zsh".into()),
            manifest: None,
            screen_state: None,
            winner: None,
            rules: Vec::new(),
            source: StatusSource::None,
            status: AgentStatus::default(),
        }
    }

    #[test]
    fn the_list_has_a_line_per_pane() {
        let text = list(&[shell(), gemini()]);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "p1   tab 1  zsh     $ not an agent");
        assert_eq!(
            lines[1],
            "p3   tab 2  gemini  ◆ Blocked  (from its screen: ask)"
        );
        assert!(text.ends_with("`monarch agent explain <pane>`\n"));
        assert_eq!(list(&[]), "no panes\n");
    }

    #[test]
    fn the_detail_marks_the_winner_and_shows_what_matched() {
        let text = detail(&gemini());
        assert!(
            text.starts_with("pane p3 (tab 2) · gemini\nstatus    ◆ Blocked, from its screen\n")
        );
        assert!(text.contains("screen    ◆ Blocked (rule ask)\n"));
        assert!(
            text.contains(
                "▶ ask   blocked    900  whole_recent\n      read: line one ⏎ line two\n"
            )
        );
        assert!(text.contains("✓ busy  working    900"));
        assert!(
            text.contains("· menu  unknown    900  whole_recent\n"),
            "{text}"
        );
        let plain = detail(&shell());
        assert!(plain.ends_with("screen    no detection manifest for zsh\n"));
    }

    #[test]
    fn previews_are_one_bounded_line() {
        let long = preview(&format!("{}end", "x".repeat(150)));
        assert_eq!(long.chars().count(), PREVIEW + 1);
        assert!(long.starts_with('…') && long.ends_with("end"));
        assert_eq!(preview("a\n\nb"), "a ⏎ b");
    }
}
