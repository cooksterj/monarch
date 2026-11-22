//! Moving the file viewer's cursor and keeping it in view.

use super::FileViewer;
use crate::OverlayKey;

/// Columns `h` and `l` scroll.
const SIDEWAYS: usize = 8;

impl FileViewer {
    /// Moves the cursor for `key`, `count` times where that means
    /// something (`5j`), or to line `count` for `G`.
    pub(super) fn motion(&mut self, key: OverlayKey, count: Option<usize>) {
        let times = count.unwrap_or(1);
        let half = (self.page / 2).max(1);
        match key {
            OverlayKey::Char('j') | OverlayKey::Down | OverlayKey::Enter => self.go_by(times, true),
            OverlayKey::Char('k') | OverlayKey::Up => self.go_by(times, false),
            OverlayKey::Ctrl('d') => self.go_by(half, true),
            OverlayKey::Ctrl('u') => self.go_by(half, false),
            OverlayKey::Char(' ') | OverlayKey::PageDown | OverlayKey::Ctrl('f') => {
                self.go_by(self.page, true);
            }
            OverlayKey::Char('b') | OverlayKey::PageUp | OverlayKey::Ctrl('b') => {
                self.go_by(self.page, false);
            }
            OverlayKey::Char('l') | OverlayKey::Right => self.left += SIDEWAYS,
            OverlayKey::Char('h') | OverlayKey::Left => {
                self.left = self.left.saturating_sub(SIDEWAYS);
            }
            OverlayKey::Char('0') => self.left = 0,
            OverlayKey::Char('g') | OverlayKey::Home => self.go_to(0),
            OverlayKey::Char('G') | OverlayKey::End => {
                let line = count.map_or(usize::MAX, |n| n.saturating_sub(1));
                self.go_to(line);
            }
            _ => {}
        }
    }

    fn go_by(&mut self, lines: usize, down: bool) {
        let to = if down {
            self.cursor.saturating_add(lines)
        } else {
            self.cursor.saturating_sub(lines)
        };
        self.go_to(to);
    }

    /// Opens on line `line` (from 1, clamped), shown a third of the way
    /// down the page so the lines before it give context.
    pub fn jump(&mut self, line: usize) {
        self.go_to(line.saturating_sub(1));
        self.top = self.cursor.saturating_sub(self.page / 3);
    }

    /// Puts the cursor on `line` (from 0), for search: a line already in
    /// view stays put, else it is shown as a jump shows it.
    pub(super) fn go_to_line(&mut self, line: usize) {
        if (self.top..self.top + self.page).contains(&line) {
            self.go_to(line);
        } else {
            self.jump(line + 1);
        }
    }

    /// Puts the cursor on `line`, clamped to the file, and scrolls to it.
    fn go_to(&mut self, line: usize) {
        self.cursor = line.min(self.len().saturating_sub(1));
        self.follow();
    }

    /// Scrolls just enough to show the cursor.
    pub(super) fn follow(&mut self) {
        if self.cursor < self.top {
            self.top = self.cursor;
        } else if self.cursor >= self.top + self.page {
            self.top = self.cursor + 1 - self.page;
        }
    }
}
