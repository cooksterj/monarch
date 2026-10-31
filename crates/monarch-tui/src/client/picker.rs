//! A fuzzy-searched list (the command palette and the go-to picker), or a
//! plain list whose letter keys answer the highlighted row (the inbox).

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::{PickerItem, PickerStep};
use crate::OverlayKey;

/// A row that matches the query, with the label characters that matched.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Match {
    item: usize,
    chars: Vec<u32>,
}

/// Items, the query typed so far, the rows that match it (best first, ties
/// in list order) and which of them is highlighted. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    title: String,
    items: Vec<PickerItem>,
    query: String,
    matches: Vec<Match>,
    selected: usize,
    /// No search: letters and digits answer the highlighted row.
    list: bool,
    /// Footer and no-rows text replacing the defaults.
    text: Option<(String, String)>,
}

impl Picker {
    /// A picker over `items` with an empty query (every row shown).
    #[must_use]
    pub fn new(title: impl Into<String>, items: Vec<PickerItem>) -> Self {
        let mut picker = Self {
            title: title.into(),
            items,
            query: String::new(),
            matches: Vec::new(),
            selected: 0,
            list: false,
            text: None,
        };
        picker.refilter();
        picker
    }

    /// A list over `items` without a search: `j`/`k` move as well as the
    /// arrows, and other letters and digits answer the highlighted row.
    #[must_use]
    pub fn list(title: impl Into<String>, items: Vec<PickerItem>) -> Self {
        Self {
            list: true,
            ..Self::new(title, items)
        }
    }

    /// The same picker with its own footer `keys` and `empty` text (shown
    /// when no row matches).
    #[must_use]
    pub fn with_text(self, keys: impl Into<String>, empty: impl Into<String>) -> Self {
        Self {
            text: Some((keys.into(), empty.into())),
            ..self
        }
    }

    /// The same picker with `query` already typed.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.edit(|q| *q = query.into());
        self
    }

    /// Its own footer and no-rows text, if it has them.
    #[must_use]
    pub fn text(&self) -> Option<(&str, &str)> {
        self.text.as_ref().map(|(k, e)| (k.as_str(), e.as_str()))
    }

    /// Changes the box title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Whether this is a [`list`](Self::list).
    #[must_use]
    pub fn is_list(&self) -> bool {
        self.list
    }

    /// Replaces the items, keeping the highlight on the same row if it is
    /// still there.
    pub fn refresh(&mut self, items: Vec<PickerItem>) {
        let kept = self.selected_item().map(|item| item.action);
        self.items = items;
        self.refilter();
        let items = &self.items;
        if let Some(i) = self
            .matches
            .iter()
            .position(|m| Some(items[m.item].action) == kept)
        {
            self.selected = i;
        }
    }

    /// Applies one key: type to search, Backspace, Ctrl-w (word), Ctrl-u
    /// (clear), Up/Down/Tab/Shift-Tab/Ctrl-n/Ctrl-p to move, Enter to run,
    /// Esc/Ctrl-c/Ctrl-g to close.
    pub fn key(&mut self, key: OverlayKey) -> PickerStep {
        if self.list
            && let Some(step) = self.list_key(key)
        {
            return step;
        }
        match key {
            OverlayKey::Char(c) => self.edit(|q| q.push(c)),
            OverlayKey::Backspace => self.edit(|q| {
                q.pop();
            }),
            OverlayKey::Ctrl('w') => self.edit(|q| {
                let kept = q.trim_end().rfind(' ').map_or(0, |i| i + 1);
                q.truncate(kept);
            }),
            OverlayKey::Ctrl('u') => self.edit(String::clear),
            OverlayKey::Up | OverlayKey::BackTab | OverlayKey::Ctrl('p') => {
                self.step(self.matches.len().saturating_sub(1));
            }
            OverlayKey::Down | OverlayKey::Tab | OverlayKey::Ctrl('n') => self.step(1),
            OverlayKey::Enter => {
                return self
                    .selected_item()
                    .map_or(PickerStep::Open, |item| PickerStep::Run(item.action));
            }
            OverlayKey::Esc | OverlayKey::Ctrl('c' | 'g') => return PickerStep::Close,
            _ => {}
        }
        PickerStep::Open
    }

    /// The box title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Matching rows, best first, each with the label characters (by
    /// index) that matched.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = (&PickerItem, &[u32])> {
        self.matches
            .iter()
            .map(|m| (&self.items[m.item], m.chars.as_slice()))
    }

    /// Index of the highlighted row in [`rows`](Self::rows).
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The highlighted row, if anything matches.
    #[must_use]
    pub fn selected_item(&self) -> Option<&PickerItem> {
        self.items.get(self.selected_index()?)
    }

    /// Where the highlighted row is in the items given, if anything
    /// matches.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.matches.get(self.selected).map(|m| m.item)
    }

    /// A list's own keys; `None` for the keys it shares with a search.
    fn list_key(&mut self, key: OverlayKey) -> Option<PickerStep> {
        let answer = |p: &Self, c| {
            p.selected_item()
                .map_or(PickerStep::Open, |item| PickerStep::Answer(item.action, c))
        };
        match key {
            OverlayKey::Char('j') => self.step(1),
            OverlayKey::Char('k') => self.step(self.matches.len().saturating_sub(1)),
            OverlayKey::Char(c) if c.is_ascii_alphanumeric() => return Some(answer(self, c)),
            OverlayKey::Char(_) | OverlayKey::Backspace | OverlayKey::Ctrl('w' | 'u') => {}
            _ => return None,
        }
        Some(PickerStep::Open)
    }

    fn edit(&mut self, change: impl FnOnce(&mut String)) {
        change(&mut self.query);
        self.refilter();
    }

    /// Moves the highlight `by` rows down, wrapping.
    fn step(&mut self, by: usize) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + by) % self.matches.len();
        }
    }

    fn refilter(&mut self) {
        let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut buf = Vec::new();
        let mut scored: Vec<(u32, Match)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(item, it)| {
                let mut chars = Vec::new();
                let haystack = Utf32Str::new(&it.label, &mut buf);
                let score = pattern.indices(haystack, &mut matcher, &mut chars)?;
                chars.sort_unstable();
                chars.dedup();
                Some((score, Match { item, chars }))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        self.matches = scored.into_iter().map(|(_, m)| m).collect();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{Action, SplitAxis};

    use super::*;

    fn palette() -> Picker {
        let items = [
            Action::Split(SplitAxis::Columns),
            Action::Split(SplitAxis::Rows),
            Action::Equalize,
            Action::NewTab,
        ]
        .map(|a| PickerItem::for_action(a, ""));
        Picker::new("command palette", items.to_vec())
    }

    fn labels(p: &Picker) -> Vec<&str> {
        p.rows().map(|(item, _)| item.label.as_str()).collect()
    }

    fn typed(p: &mut Picker, text: &str) {
        for c in text.chars() {
            assert_eq!(p.key(OverlayKey::Char(c)), PickerStep::Open);
        }
    }

    #[test]
    fn own_text_title_and_item_index() {
        let mut p = palette().with_text(" keys ", "nothing");
        assert_eq!(p.text(), Some((" keys ", "nothing")));
        assert_eq!(palette().text(), None);
        p.set_title("other");
        assert_eq!(p.title(), "other");
        typed(&mut p, "tab");
        assert_eq!(p.selected_index(), Some(3), "new tab is item 3");
        typed(&mut p, "zzz");
        assert_eq!(p.selected_index(), None);
    }

    #[test]
    fn an_empty_query_lists_everything_in_order() {
        let p = palette();
        assert_eq!(
            labels(&p),
            ["split right", "split down", "equalize", "new tab"]
        );
        assert_eq!(
            p.selected_item().map(|i| i.action),
            Some(Action::Split(SplitAxis::Columns))
        );
    }

    #[test]
    fn typing_narrows_and_marks_the_matched_characters() {
        let mut p = palette();
        typed(&mut p, "sd");
        assert_eq!(labels(&p), ["split down"]);
        let (_, chars) = p.rows().next().expect("a row");
        assert_eq!(chars, [0, 6]);
        assert_eq!(
            p.key(OverlayKey::Enter),
            PickerStep::Run(Action::Split(SplitAxis::Rows))
        );
    }

    #[test]
    fn words_match_separately_and_editing_widens_again() {
        let mut p = palette();
        typed(&mut p, "tab new");
        assert_eq!(labels(&p), ["new tab"]);
        p.key(OverlayKey::Ctrl('w'));
        assert_eq!(p.query(), "tab ");
        p.key(OverlayKey::Ctrl('u'));
        assert_eq!(labels(&p).len(), 4);
        typed(&mut p, "q");
        p.key(OverlayKey::Backspace);
        assert_eq!(p.query(), "");
    }

    #[test]
    fn the_highlight_wraps_and_nothing_matching_cannot_run() {
        let mut p = palette();
        p.key(OverlayKey::Up);
        assert_eq!(p.selected(), 3, "up from the top wraps");
        p.key(OverlayKey::Down);
        assert_eq!(p.selected(), 0);
        typed(&mut p, "zzz");
        assert_eq!(p.rows().len(), 0);
        p.key(OverlayKey::Down);
        assert_eq!(p.key(OverlayKey::Enter), PickerStep::Open);
        assert_eq!(p.key(OverlayKey::Esc), PickerStep::Close);
    }

    #[test]
    fn lists_answer_instead_of_searching() {
        let rows = [3, 4].map(|n| PickerItem {
            label: format!("pane {n}"),
            detail: String::new(),
            action: Action::FocusPane(monarch_core::PaneId(n)),
        });
        let mut list = Picker::list("inbox", rows.to_vec());
        assert!(list.is_list() && !palette().is_list());
        assert_eq!(list.key(OverlayKey::Char('j')), PickerStep::Open);
        let second = Action::FocusPane(monarch_core::PaneId(4));
        assert_eq!(
            list.key(OverlayKey::Char('y')),
            PickerStep::Answer(second, 'y')
        );
        assert_eq!(
            list.key(OverlayKey::Char('2')),
            PickerStep::Answer(second, '2')
        );
        assert_eq!(list.query(), "", "nothing typed into a search");
        assert_eq!(list.key(OverlayKey::Char('?')), PickerStep::Open, "ignored");
        assert_eq!(list.key(OverlayKey::Enter), PickerStep::Run(second));
        list.refresh(vec![rows[1].clone(), rows[0].clone()]);
        assert_eq!(list.selected(), 0, "highlight follows its row");
        list.refresh(vec![rows[0].clone()]);
        assert_eq!(list.selected_item().map(|i| i.action), Some(rows[0].action));
        list.refresh(Vec::new());
        assert_eq!(
            list.key(OverlayKey::Char('y')),
            PickerStep::Open,
            "nothing to answer"
        );
        assert_eq!(list.key(OverlayKey::Esc), PickerStep::Close);
    }
}
