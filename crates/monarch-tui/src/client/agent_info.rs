//! The agent info (`prefix i`): why the focused pane's agent is in its
//! state, as the server explains it, scrolled with the arrows.

use monarch_core::{AgentState, PaneId, UnixMillis, age_label};
use monarch_proto::{AgentTransition, PaneExplanation, RuleOutcome};

use super::InfoStep;
use crate::{HelpLine, OverlayKey};

/// Lines scrolled by Page Up and Page Down when the page height is not
/// known yet.
const PAGE: usize = 10;
/// Longest rule preview shown, in characters.
const PREVIEW: usize = 60;

/// The pane asked about, the answer once it comes, and the scroll. Pure
/// data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pane: PaneId,
    name: String,
    /// `None` until the server answers; `Some(None)` for a pane that is
    /// gone.
    explained: Option<Option<Box<PaneExplanation>>>,
    scroll: usize,
    rows: usize,
}

impl AgentInfo {
    /// Info on `pane`, called `name`, waiting for the server.
    #[must_use]
    pub fn new(pane: PaneId, name: impl Into<String>) -> Self {
        Self {
            pane,
            name: name.into(),
            explained: None,
            scroll: 0,
            rows: PAGE,
        }
    }

    /// The pane asked about.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// The server's answer; the scroll is kept where it was.
    pub fn load(&mut self, explained: Option<Box<PaneExplanation>>) {
        self.explained = Some(explained);
    }

    /// `agent info · pane 3 claude`.
    #[must_use]
    pub fn title(&self) -> String {
        format!(" agent info · pane {} {} ", self.pane.0, self.name).replace("  ", " ")
    }

    /// The first shown line of [`AgentInfo::lines`].
    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Tells it how many lines a page shows.
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows.max(1);
    }

    /// Applies one key: arrows, `j`/`k` and pages scroll, `r` asks again,
    /// Esc, `q`, `i` or Ctrl-c closes.
    pub fn key(&mut self, key: OverlayKey) -> InfoStep {
        let page = isize::try_from(self.rows).unwrap_or(isize::MAX);
        match key {
            OverlayKey::Esc | OverlayKey::Char('q' | 'i') | OverlayKey::Ctrl('c' | 'g') => {
                return InfoStep::Close;
            }
            OverlayKey::Char('r') => return InfoStep::Refresh,
            OverlayKey::Up | OverlayKey::Char('k') => self.scroll_by(-1),
            OverlayKey::Down | OverlayKey::Char('j') => self.scroll_by(1),
            OverlayKey::PageUp | OverlayKey::Ctrl('u') => self.scroll_by(-page),
            OverlayKey::PageDown | OverlayKey::Ctrl('d') => self.scroll_by(page),
            OverlayKey::Home | OverlayKey::Char('g') => self.scroll = 0,
            OverlayKey::End | OverlayKey::Char('G') => self.scroll = usize::MAX,
            _ => {}
        }
        InfoStep::Open
    }

    /// The page at `now`: the status and where it came from, the states
    /// entered lately, then the manifest's rules.
    #[must_use]
    pub fn lines(&self, now: UnixMillis) -> Vec<HelpLine> {
        let explained = match &self.explained {
            None => return vec![HelpLine::entry("", "asking the server…")],
            Some(None) => return vec![HelpLine::entry("", "the pane is gone")],
            Some(Some(explained)) => explained,
        };
        let mut lines = status_lines(explained, now);
        if !explained.transitions.is_empty() {
            lines.push(HelpLine::Heading("Recent states, oldest first".into()));
            lines.extend(explained.transitions.iter().map(|t| transition(t, now)));
        }
        if explained.manifest.is_some() {
            lines.push(HelpLine::Heading("Rules · ▶ fired · ✓ matched".into()));
            for rule in &explained.rules {
                let won = explained.winner.as_deref() == Some(rule.id.as_str());
                lines.extend(rule_lines(rule, won));
            }
        }
        lines
    }

    /// The scroll, clamped to the lines there are at `now`.
    #[must_use]
    pub fn clamped_scroll(&self, now: UnixMillis) -> usize {
        self.scroll
            .min(self.lines(now).len().saturating_sub(self.rows))
    }

    fn scroll_by(&mut self, by: isize) {
        self.scroll = self.scroll.saturating_add_signed(by);
    }
}

/// `state`, `activity`, `program` and `screen`.
fn status_lines(explained: &PaneExplanation, now: UnixMillis) -> Vec<HelpLine> {
    let status = &explained.status;
    let since = explained.transitions.last().map(|t| t.at);
    let mut state = glyphed(status.state);
    if let Some(since) = since {
        state.push_str(&format!(" for {}", age_label(since.age_at(now))));
    }
    state.push_str(&format!(" · from {}", explained.source.describe()));
    let mut lines = vec![HelpLine::entry("state", state)];
    if let Some(activity) = &status.activity {
        lines.push(HelpLine::entry("activity", activity.clone()));
    }
    let program = explained.program.as_deref().unwrap_or("?");
    lines.push(HelpLine::entry("program", program));
    let screen = match (
        &explained.manifest,
        explained.screen_state,
        &explained.winner,
    ) {
        (None, _, _) => format!("no detection manifest for {program}"),
        (Some(m), Some(s), Some(rule)) => format!("{} by rule {rule} · {m}", glyphed(s)),
        (Some(m), Some(s), None) => format!("{} (no rule matched) · {m}", glyphed(s)),
        (Some(m), None, rule) => {
            format!(
                "unchanged: rule {} holds · {m}",
                rule.as_deref().unwrap_or("?")
            )
        }
    };
    lines.push(HelpLine::entry("screen", screen));
    lines
}

/// `3m ago   ● Working · from its hooks · Edit a.rs`.
fn transition(t: &AgentTransition, now: UnixMillis) -> HelpLine {
    let mut text = format!("{} · from {}", glyphed(t.state), t.source.describe());
    if let Some(activity) = &t.activity {
        text.push_str(" · ");
        text.push_str(activity);
    }
    HelpLine::entry(format!("{} ago", age_label(t.at.age_at(now))), text)
}

/// `▶ permission_prompt   blocked · 900 · whole_recent`, then what a
/// matched rule read.
fn rule_lines(rule: &RuleOutcome, won: bool) -> Vec<HelpLine> {
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
    let mut lines = vec![HelpLine::entry(
        format!("{mark} {}", rule.id),
        format!("{does} · {} · {}", rule.priority, rule.region),
    )];
    if rule.matched {
        lines.push(HelpLine::entry(
            "",
            format!("read: {}", preview(&rule.preview)),
        ));
    }
    lines
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

fn glyphed(state: AgentState) -> String {
    format!("{} {}", state.glyph(), state.label())
}

#[cfg(test)]
mod tests {
    use monarch_proto::{AgentStatus, StatusSource};

    use super::*;

    const NOW: UnixMillis = UnixMillis(200_000);

    fn rule(id: &str, state: AgentState, priority: i32, matched: bool) -> RuleOutcome {
        RuleOutcome {
            id: id.into(),
            state,
            priority,
            region: if matched { "whole_recent" } else { "bottom" }.into(),
            matched,
            hold: !matched,
            preview: if matched { "Allow?\n[y/n]" } else { "" }.into(),
        }
    }

    fn entered(state: AgentState, activity: Option<&str>, at: u64) -> AgentTransition {
        AgentTransition {
            state,
            source: StatusSource::Hooks,
            activity: activity.map(Into::into),
            at: UnixMillis(at),
        }
    }

    fn claude() -> PaneExplanation {
        PaneExplanation {
            pane: PaneId(3),
            tab: 2,
            program: Some("claude".into()),
            status: AgentStatus {
                state: AgentState::Blocked,
                activity: Some("Allow Bash?".into()),
                agent: Some(monarch_core::AgentKind::Claude),
            },
            source: StatusSource::Hooks,
            manifest: Some("claude 2026.1".into()),
            screen_state: Some(AgentState::Blocked),
            winner: Some("permission_prompt".into()),
            rules: vec![
                rule("permission_prompt", AgentState::Blocked, 900, true),
                rule("busy", AgentState::Working, 500, false),
            ],
            transitions: vec![
                entered(AgentState::Working, Some("Bash ls"), 20_000),
                entered(AgentState::Blocked, None, 140_000),
            ],
        }
    }

    fn texts(lines: &[HelpLine]) -> Vec<String> {
        lines
            .iter()
            .map(|l| match l {
                HelpLine::Heading(h) => format!("# {h}"),
                HelpLine::Entry { keys, text } => format!("{keys}|{text}"),
            })
            .collect()
    }

    #[test]
    fn the_page_waits_then_shows_status_history_and_rules() {
        let mut info = AgentInfo::new(PaneId(3), "claude");
        assert_eq!(info.title(), " agent info · pane 3 claude ");
        assert_eq!(texts(&info.lines(NOW)), ["|asking the server…"]);
        info.load(Some(Box::new(claude())));
        let lines = texts(&info.lines(NOW));
        assert_eq!(
            lines[..4],
            [
                "state|◆ Blocked for 1m · from its hooks",
                "activity|Allow Bash?",
                "program|claude",
                "screen|◆ Blocked by rule permission_prompt · claude 2026.1",
            ]
        );
        info.load(None);
        assert_eq!(texts(&info.lines(NOW)), ["|the pane is gone"]);
    }

    #[test]
    fn the_page_lists_recent_states_then_every_rule() {
        let mut info = AgentInfo::new(PaneId(3), "claude");
        info.load(Some(Box::new(claude())));
        let lines = texts(&info.lines(NOW));
        assert_eq!(lines[4], "# Recent states, oldest first");
        assert_eq!(lines[5], "3m ago|● Working · from its hooks · Bash ls");
        assert_eq!(lines[6], "1m ago|◆ Blocked · from its hooks");
        assert_eq!(lines[7], "# Rules · ▶ fired · ✓ matched");
        assert_eq!(lines[8], "▶ permission_prompt|blocked · 900 · whole_recent");
        assert_eq!(lines[9], "|read: Allow? ⏎ [y/n]");
        assert_eq!(lines[10], "· busy|hold · 500 · bottom");
        assert_eq!(lines.len(), 11);
    }

    #[test]
    fn a_shell_has_no_manifest_and_no_history() {
        let mut info = AgentInfo::new(PaneId(1), "zsh");
        info.load(Some(Box::new(PaneExplanation {
            pane: PaneId(1),
            tab: 1,
            program: Some("zsh".into()),
            status: AgentStatus::default(),
            source: StatusSource::None,
            manifest: None,
            screen_state: None,
            winner: None,
            rules: Vec::new(),
            transitions: Vec::new(),
        })));
        assert_eq!(
            texts(&info.lines(NOW)),
            [
                "state|$ Shell · from nothing (not an agent)",
                "program|zsh",
                "screen|no detection manifest for zsh",
            ]
        );
    }

    #[test]
    fn keys_scroll_refresh_and_close() {
        let mut info = AgentInfo::new(PaneId(3), "claude");
        info.load(Some(Box::new(claude())));
        info.set_rows(4);
        assert_eq!(info.key(OverlayKey::Down), InfoStep::Open);
        assert_eq!(info.key(OverlayKey::Char('j')), InfoStep::Open);
        assert_eq!(info.scroll(), 2);
        assert_eq!(info.key(OverlayKey::End), InfoStep::Open);
        assert_eq!(info.clamped_scroll(NOW), 11 - 4, "stops at the last page");
        assert_eq!(info.key(OverlayKey::Home), InfoStep::Open);
        assert_eq!(info.key(OverlayKey::Up), InfoStep::Open);
        assert_eq!(info.scroll(), 0);
        assert_eq!(info.key(OverlayKey::Char('r')), InfoStep::Refresh);
        assert_eq!(info.key(OverlayKey::Char('x')), InfoStep::Open, "ignored");
        assert_eq!(info.key(OverlayKey::Char('q')), InfoStep::Close);
        assert_eq!(info.key(OverlayKey::Esc), InfoStep::Close);
        assert_eq!(info.key(OverlayKey::Char('i')), InfoStep::Close);
        assert_eq!(info.pane(), PaneId(3));
    }

    #[test]
    fn previews_are_one_bounded_line() {
        let long = preview(&format!("{}end", "x".repeat(100)));
        assert_eq!(long.chars().count(), PREVIEW + 1);
        assert!(long.starts_with('…') && long.ends_with("end"));
    }
}
