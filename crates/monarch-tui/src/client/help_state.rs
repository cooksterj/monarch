//! The help page: every key, gesture and command, filtered by what you
//! type and scrolled with the arrows.

mod lines;

use monarch_core::Keymap;

use crate::{HelpLine, OverlayKey};

/// Lines scrolled by Page Up and Page Down when the page height is not
/// known yet.
const PAGE: usize = 10;

/// The lines, the search and the scroll. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpState {
    lines: Vec<HelpLine>,
    filter: String,
    scroll: usize,
    rows: usize,
}

impl HelpState {
    /// The help for `keymap`.
    #[must_use]
    pub fn new(keymap: &Keymap) -> Self {
        Self {
            lines: lines::help_lines(keymap),
            filter: String::new(),
            scroll: 0,
            rows: PAGE,
        }
    }

    /// What was typed.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The first shown line of [`HelpState::visible`].
    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Tells it how many lines a page shows.
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows.max(1);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// The lines matching the search: an entry that matches, with its
    /// heading, or a whole section whose heading matches.
    #[must_use]
    pub fn visible(&self) -> Vec<&HelpLine> {
        let needle = self.filter.to_lowercase();
        if needle.is_empty() {
            return self.lines.iter().collect();
        }
        let mut out = Vec::new();
        let mut heading: Option<(&HelpLine, bool)> = None;
        for line in &self.lines {
            if let HelpLine::Heading(_) = line {
                heading = Some((line, line.matches(&needle)));
                continue;
            }
            let whole = heading.is_some_and(|(_, m)| m);
            if !(whole || line.matches(&needle)) {
                continue;
            }
            if let Some((h, _)) = heading.filter(|(h, _)| !out.contains(h)) {
                out.push(h);
            }
            out.push(line);
        }
        out
    }

    /// Applies one key; `false` once it closes. Typing searches, arrows
    /// and pages scroll, Esc clears the search and then closes, and `?`
    /// with nothing typed closes as it opened.
    pub fn key(&mut self, key: OverlayKey) -> bool {
        let page = self.rows;
        match key {
            OverlayKey::Esc if self.filter.is_empty() => return false,
            OverlayKey::Char('?') if self.filter.is_empty() => return false,
            OverlayKey::Esc | OverlayKey::Ctrl('u') => self.search(String::new()),
            OverlayKey::Char(c) => self.search(format!("{}{c}", self.filter)),
            OverlayKey::Backspace => {
                let mut filter = self.filter.clone();
                filter.pop();
                self.search(filter);
            }
            OverlayKey::Up => self.scroll_by(-1),
            OverlayKey::Down => self.scroll_by(1),
            OverlayKey::PageUp | OverlayKey::Ctrl('b') => self.scroll_by(-signed(page)),
            OverlayKey::PageDown | OverlayKey::Ctrl('f') => self.scroll_by(signed(page)),
            OverlayKey::Home => self.scroll = 0,
            OverlayKey::End => self.scroll = self.max_scroll(),
            _ => {}
        }
        true
    }

    fn search(&mut self, filter: String) {
        self.filter = filter;
        self.scroll = 0;
    }

    fn scroll_by(&mut self, by: isize) {
        self.scroll = self.scroll.saturating_add_signed(by).min(self.max_scroll());
    }

    fn max_scroll(&self) -> usize {
        self.visible().len().saturating_sub(self.rows)
    }
}

fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> HelpState {
        let mut help = HelpState::new(&Keymap::default());
        for c in text.chars() {
            assert!(help.key(OverlayKey::Char(c)));
        }
        help
    }

    #[test]
    fn typing_keeps_matches_under_their_headings() {
        let help = typed("zoom");
        let shown = help.visible();
        assert_eq!(shown.len(), 2, "{shown:?}");
        assert_eq!(shown[0], &HelpLine::Heading("Panes".into()));
        let whole = typed("mouse");
        assert!(
            whole.visible().len() > 7,
            "a matching heading keeps its section"
        );
        assert!(typed("xyzzy").visible().is_empty());
    }

    #[test]
    fn esc_clears_then_closes_and_question_mark_closes() {
        let mut help = typed("zo");
        help.key(OverlayKey::Backspace);
        assert_eq!(help.filter(), "z");
        assert!(help.key(OverlayKey::Esc), "first esc clears");
        assert_eq!(help.filter(), "");
        assert!(!help.key(OverlayKey::Esc));
        assert!(!HelpState::new(&Keymap::default()).key(OverlayKey::Char('?')));
        assert!(
            typed("a").key(OverlayKey::Char('?')),
            "? searches once typing"
        );
    }

    #[test]
    fn scrolling_stops_at_the_last_page() {
        let mut help = HelpState::new(&Keymap::default());
        help.set_rows(10);
        let total = help.visible().len();
        help.key(OverlayKey::Up);
        assert_eq!(help.scroll(), 0);
        help.key(OverlayKey::PageDown);
        assert_eq!(help.scroll(), 10);
        help.key(OverlayKey::End);
        assert_eq!(help.scroll(), total - 10);
        help.key(OverlayKey::Down);
        assert_eq!(help.scroll(), total - 10);
        help.set_rows(total + 5);
        assert_eq!(help.scroll(), 0, "everything fits");
    }
}
