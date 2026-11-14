//! How `monarch agent explain` prints its answer: one line per pane, or
//! one pane in full with every rule of its agent's manifest.

use std::fmt::Write;

use monarch_core::{AgentState, UnixMillis, age_label};
use monarch_proto::{AgentTransition, PaneExplanation, RuleOutcome, StatusSource, SubagentStatus};

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
        let rule = match (pane.source, pane.winner.as_deref()) {
            (StatusSource::Screen, Some(rule)) => format!(": {rule}"),
            _ => String::new(),
        };
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

/// One pane: its status and source, the states it entered lately (aged
/// against `now`), then its manifest's rules, the winner marked `▶` and
/// other matches `✓`, with what matched rules read.
pub(super) fn detail(pane: &PaneExplanation, now: UnixMillis) -> String {
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
    text.push_str(&history(&pane.transitions, now));
    text.push_str(&subagents(&pane.status.subagents));
    let Some(manifest) = &pane.manifest else {
        let _ = writeln!(
            text,
            "screen    no detection manifest for {}",
            program(pane)
        );
        return text;
    };
    let shows = shows(pane);
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

/// What the screen shows to the manifest, with the rule that decided.
fn shows(pane: &PaneExplanation) -> String {
    match (pane.screen_state, pane.winner.as_deref()) {
        (Some(s), Some(rule)) => format!("{} (rule {rule})", state(s)),
        (Some(s), None) => format!("{} (no rule matched)", state(s)),
        (None, rule) => format!("unchanged (rule {} keeps the state)", rule.unwrap_or("?")),
    }
}

/// `history   3m ago  ● Working  from its hooks  Edit a.rs`, one line
/// per state entered, oldest first; nothing for a pane with no history.
fn history(transitions: &[AgentTransition], now: UnixMillis) -> String {
    let mut text = String::new();
    for (i, t) in transitions.iter().enumerate() {
        let head = if i == 0 { "history" } else { "" };
        let ago = format!("{} ago", age_label(t.at.age_at(now)));
        let _ = write!(
            text,
            "{head:<9} {ago:>8}  {:<11} from {}",
            state(t.state),
            t.source.describe()
        );
        if let Some(activity) = &t.activity {
            let _ = write!(text, "  {activity}");
        }
        text.push('\n');
    }
    text
}

/// `subagent ● Working   Explore  Grep TODO`, one line per sub-agent.
fn subagents(children: &[SubagentStatus]) -> String {
    let mut text = String::new();
    for (i, child) in children.iter().enumerate() {
        let head = if i == 0 { "subagent" } else { "" };
        let _ = write!(
            text,
            "{head:<9} {:<11} {}",
            state(child.state),
            child.label()
        );
        if let Some(activity) = &child.activity {
            let _ = write!(text, "  {activity}");
        }
        text.push('\n');
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
    use monarch_proto::AgentStatus;

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
            transitions: vec![
                AgentTransition {
                    state: AgentState::Working,
                    source: StatusSource::Screen,
                    activity: None,
                    at: UnixMillis(1_000),
                },
                AgentTransition {
                    state: AgentState::Blocked,
                    source: StatusSource::Screen,
                    activity: Some("Allow?".into()),
                    at: UnixMillis(61_000),
                },
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
            transitions: Vec::new(),
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
        let hooked = PaneExplanation {
            source: StatusSource::Hooks,
            ..gemini()
        };
        let line = list(&[hooked]);
        assert!(
            line.contains("(from its hooks)\n"),
            "the screen's rule didn't decide: {line}"
        );
        assert_eq!(list(&[]), "no panes\n");
    }

    #[test]
    fn the_detail_marks_the_winner_and_shows_what_matched() {
        let text = detail(&gemini(), UnixMillis(121_000));
        assert!(
            text.starts_with("pane p3 (tab 2) · gemini\nstatus    ◆ Blocked, from its screen\n")
        );
        assert!(
            text.contains(
                "history     2m ago  ● Working   from its screen\n            1m ago  ◆ Blocked   from its screen  Allow?\n"
            ),
            "{text}"
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
        let plain = detail(&shell(), UnixMillis(0));
        assert!(plain.ends_with("screen    no detection manifest for zsh\n"));
        assert!(!plain.contains("history"), "{plain}");
    }

    #[test]
    fn the_detail_lists_subagents_under_the_history() {
        let child = |kind: &str, state, activity: Option<&str>| SubagentStatus {
            id: kind.into(),
            kind: kind.into(),
            state,
            activity: activity.map(Into::into),
        };
        let plain = shell();
        let pane = PaneExplanation {
            status: AgentStatus {
                subagents: vec![
                    child("Explore", AgentState::Working, Some("Grep TODO")),
                    child("", AgentState::Blocked, None),
                ],
                ..plain.status.clone()
            },
            ..plain
        };
        let text = detail(&pane, UnixMillis(0));
        assert!(
            text.contains(
                "subagent  ● Working   Explore  Grep TODO\n          ◆ Blocked   subagent\n"
            ),
            "{text}"
        );
        assert!(!detail(&shell(), UnixMillis(0)).contains("subagent"));
    }

    #[test]
    fn previews_are_one_bounded_line() {
        let long = preview(&format!("{}end", "x".repeat(150)));
        assert_eq!(long.chars().count(), PREVIEW + 1);
        assert!(long.starts_with('…') && long.ends_with("end"));
        assert_eq!(preview("a\n\nb"), "a ⏎ b");
    }
}
