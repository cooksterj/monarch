//! Searching from copy mode: the `/` (towards older output) and `?`
//! prompts, `n`/`N`, results, and the matches in view.

use monarch_proto::{GridPoint, Screen};

use super::{CopyMode, Prompt};
use crate::{CopyStep, OverlayKey, SelectionSpan};

impl CopyMode {
    /// A key while a search is being typed: text, Backspace, Ctrl-u, Enter
    /// to search, Esc or Ctrl-c to give up.
    pub(super) fn prompt_key(&mut self, key: OverlayKey) -> CopyStep {
        let Some(prompt) = self.prompt.as_mut() else {
            return CopyStep::Stay;
        };
        match key {
            OverlayKey::Char(c) => prompt.text.push(c),
            OverlayKey::Backspace if prompt.text.pop().is_none() => self.prompt = None,
            OverlayKey::Ctrl('u') => prompt.text.clear(),
            OverlayKey::Esc | OverlayKey::Ctrl('c') => self.prompt = None,
            OverlayKey::Enter => {
                let Prompt { text, backward } = self.prompt.take().unwrap_or_else(|| Prompt {
                    text: String::new(),
                    backward: true,
                });
                if !text.is_empty() {
                    self.search = Some((text, backward));
                }
                return self.repeat(false);
            }
            _ => {}
        }
        CopyStep::Stay
    }

    /// Searches again for the last text, the same way or (`reverse`) the
    /// other way.
    pub(super) fn repeat(&self, reverse: bool) -> CopyStep {
        match &self.search {
            Some((query, backward)) => CopyStep::Search {
                query: query.clone(),
                from: self.cursor,
                backward: *backward != reverse,
            },
            None => CopyStep::Stay,
        }
    }

    /// The server's answer: moves the cursor to the match. Returns whether
    /// there was one.
    pub fn found(&mut self, hit: Option<(GridPoint, GridPoint)>, screen: &Screen) -> bool {
        let Some((start, _)) = hit else {
            return false;
        };
        self.cursor = start;
        self.follow(screen);
        true
    }

    /// The last searched text, if any.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.search.as_ref().map(|(q, _)| q.as_str())
    }

    /// Every place the last searched text appears in view (case-insensitive
    /// unless it has capitals, like the server's search).
    #[must_use]
    pub fn matches(&self, screen: &Screen) -> Vec<SelectionSpan> {
        let Some(query) = self.query().filter(|q| !q.is_empty()) else {
            return Vec::new();
        };
        let exact = query.chars().any(char::is_uppercase);
        let fold = |c: char| {
            if exact {
                c
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        };
        let needle: Vec<char> = query.chars().map(fold).collect();
        let mut out = Vec::new();
        for (row, cells) in (0i64..).zip(&screen.rows) {
            let text: Vec<(char, u16)> = (0u16..)
                .zip(cells)
                .filter(|(_, c)| c.width > 0)
                .map(|(col, c)| (fold(c.ch), col))
                .collect();
            for start in 0..text.len().saturating_sub(needle.len() - 1) {
                if text[start..start + needle.len()]
                    .iter()
                    .map(|(c, _)| *c)
                    .eq(needle.iter().copied())
                {
                    out.push(SelectionSpan {
                        start_row: row,
                        start_col: text[start].1,
                        end_row: row,
                        end_col: text[start + needle.len() - 1].1,
                        block: false,
                    });
                }
            }
        }
        out
    }
}
