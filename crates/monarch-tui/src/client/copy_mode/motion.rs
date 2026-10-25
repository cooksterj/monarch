//! Cursor motions, vi style: `h j k l` and arrows, `w b e` words, `0 ^ $`
//! and Home/End, `H M L` in view, Ctrl-u/d half pages, Ctrl-b/f and
//! PgUp/PgDn pages, `g`/`G` oldest and newest line.

use monarch_proto::{Cell, GridPoint, Screen};

use super::CopyMode;
use crate::OverlayKey;

/// What kind of character a cell holds, for word motions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Blank,
    Word,
    Punct,
}

impl CopyMode {
    /// Moves the cursor for `key`; other keys do nothing.
    pub(super) fn motion(&mut self, key: OverlayKey, screen: &Screen) {
        let rows = i64::from(screen.size.rows.max(1));
        let last_line = i64::from(screen.history) + rows - 1;
        let last_col = screen.size.cols.saturating_sub(1);
        let top = GridPoint::top_line(screen);
        let c = &mut self.cursor;
        match key {
            OverlayKey::Char('h') | OverlayKey::Left => c.col = c.col.saturating_sub(1),
            OverlayKey::Char('l') | OverlayKey::Right => c.col = (c.col + 1).min(last_col),
            OverlayKey::Char('j') | OverlayKey::Down => c.line += 1,
            OverlayKey::Char('k') | OverlayKey::Up => c.line -= 1,
            OverlayKey::Ctrl('d') => c.line += rows / 2,
            OverlayKey::Ctrl('u') => c.line -= rows / 2,
            OverlayKey::Ctrl('f') | OverlayKey::PageDown => c.line += rows,
            OverlayKey::Ctrl('b') | OverlayKey::PageUp => c.line -= rows,
            OverlayKey::Char('g') => *c = GridPoint { line: 0, col: 0 },
            OverlayKey::Char('G') => {
                *c = GridPoint {
                    line: last_line,
                    col: 0,
                }
            }
            OverlayKey::Char('H') => c.line = top,
            OverlayKey::Char('M') => c.line = top + rows / 2,
            OverlayKey::Char('L') => c.line = top + rows - 1,
            OverlayKey::Char('0') | OverlayKey::Home => c.col = 0,
            other => self.in_line(other, screen),
        }
        self.cursor.line = self.cursor.line.clamp(0, last_line);
    }

    /// Motions that read the cursor's line: `^ $ w b e`. The cursor is
    /// always in view, so its line is in the client's copy of the screen.
    fn in_line(&mut self, key: OverlayKey, screen: &Screen) {
        let Some(cells) = row(screen, self.cursor) else {
            return;
        };
        let classes: Vec<Class> = cells
            .iter()
            .scan(Class::Blank, |prev, cell| {
                *prev = if cell.width == 0 { *prev } else { class(cell) };
                Some(*prev)
            })
            .collect();
        let col = usize::from(self.cursor.col).min(classes.len().saturating_sub(1));
        let found = match key {
            OverlayKey::Char('$') | OverlayKey::End => {
                classes.iter().rposition(|c| *c != Class::Blank)
            }
            OverlayKey::Char('^') => classes.iter().position(|c| *c != Class::Blank),
            OverlayKey::Char('w') => next_start(&classes, col),
            OverlayKey::Char('b') => prev_start(&classes, col),
            OverlayKey::Char('e') => next_end(&classes, col),
            _ => return,
        };
        if let Some(col) = found.and_then(|c| u16::try_from(c).ok()) {
            self.cursor.col = col;
        }
    }
}

fn row(screen: &Screen, at: GridPoint) -> Option<&[Cell]> {
    let index = usize::try_from(at.viewport_row(screen)).ok()?;
    screen.rows.get(index).map(Vec::as_slice)
}

fn class(cell: &Cell) -> Class {
    if cell.ch.is_whitespace() || cell.ch == '\0' {
        Class::Blank
    } else if cell.ch.is_alphanumeric() || cell.ch == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// The start of the next word after `col`.
fn next_start(classes: &[Class], col: usize) -> Option<usize> {
    (col + 1..classes.len()).find(|&i| classes[i] != Class::Blank && classes[i] != classes[i - 1])
}

/// The start of the word before `col` (or of the one it is in).
fn prev_start(classes: &[Class], col: usize) -> Option<usize> {
    (0..col)
        .rev()
        .find(|&i| classes[i] != Class::Blank && (i == 0 || classes[i - 1] != classes[i]))
}

/// The end of the word after `col` (or of the one it is in).
fn next_end(classes: &[Class], col: usize) -> Option<usize> {
    (col + 1..classes.len()).find(|&i| {
        classes[i] != Class::Blank && classes.get(i + 1).is_none_or(|n| *n != classes[i])
    })
}
