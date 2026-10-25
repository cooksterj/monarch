//! Draws the help page over the panes: a box of headings and key lines,
//! with the search and position in its bottom border.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::{Chrome, HelpLine};
use crate::HelpState;

/// Widest the box gets.
const WIDTH: u16 = 84;
/// Widest the keys column gets; longer keys are cut.
const KEYS: usize = 24;

/// The help page as drawn.
#[derive(Debug, Clone, Copy)]
pub struct HelpView<'a> {
    /// What to draw.
    pub state: &'a HelpState,
    /// The prefix as configured, named in the title.
    pub prefix: &'a str,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl HelpView<'_> {
    /// Where the box goes inside `area`: full height, centred.
    #[must_use]
    pub fn placement(area: Rect) -> Rect {
        let width = WIDTH.min(area.width);
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y,
            width,
            area.height,
        )
    }

    /// Lines of help a box in `area` shows at once.
    #[must_use]
    pub fn body_rows(area: Rect) -> usize {
        usize::from(Self::placement(area).height.saturating_sub(2))
    }

    fn footer(&self, shown: usize, total: usize) -> String {
        let filter = self.state.filter();
        let first = self.state.scroll() + 1;
        let place = if total > shown {
            format!(" {first}–{} of {total} ·", first + shown - 1)
        } else {
            String::new()
        };
        if filter.is_empty() {
            format!("{place} type to search · ↑↓ PgUp PgDn scroll · esc close ")
        } else {
            format!(" search: {filter} ·{place} esc clears ")
        }
    }

    fn draw_line(&self, line: &HelpLine, keys_width: usize, row: Rect, buf: &mut Buffer) {
        let room = usize::from(row.width);
        match line {
            HelpLine::Heading(title) => {
                buf.set_stringn(row.x, row.y, title, room, self.chrome.highlight());
            }
            HelpLine::Entry { keys, text } => {
                let bold = Style::new().add_modifier(Modifier::BOLD);
                buf.set_stringn(row.x + 2, row.y, keys, keys_width.min(room), bold);
                let x = row.x + 4 + u16::try_from(keys_width).unwrap_or(u16::MAX);
                let style = if text.contains("(unbound") {
                    Style::new().add_modifier(Modifier::DIM)
                } else {
                    Style::new()
                };
                let left = usize::from(row.right().saturating_sub(x));
                buf.set_stringn(x, row.y, text, left, style);
            }
        }
    }
}

impl Widget for HelpView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let outer = Self::placement(area);
        if outer.height < 3 || outer.width < 20 {
            return;
        }
        let lines = self.state.visible();
        let rows = usize::from(outer.height - 2);
        let shown = rows.min(lines.len().saturating_sub(self.state.scroll()));
        let title = format!(" help · keys follow {} ", self.prefix);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(title).style(self.chrome.highlight()))
            .title_bottom(self.footer(shown, lines.len()));
        let inner = block.inner(outer);
        Clear.render(outer, buf);
        block.render(outer, buf);
        let widths = keys_widths(&lines);
        let body = lines
            .iter()
            .zip(widths)
            .skip(self.state.scroll())
            .take(rows);
        for (y, (line, width)) in (inner.y..inner.bottom()).zip(body) {
            let row = Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1);
            self.draw_line(line, width, row, buf);
        }
        if lines.is_empty() {
            let dim = Style::new().add_modifier(Modifier::DIM);
            buf.set_stringn(
                inner.x + 1,
                inner.y,
                "no matches",
                usize::from(inner.width),
                dim,
            );
        }
    }
}

/// Each line's keys column width: the widest keys in its section, so a
/// long command doesn't push every section's text to the right.
fn keys_widths(lines: &[&HelpLine]) -> Vec<usize> {
    let mut widths = vec![0; lines.len()];
    let mut start = 0;
    for end in 1..=lines.len() {
        if end < lines.len() && !matches!(lines[end], HelpLine::Heading(_)) {
            continue;
        }
        let widest = lines[start..end]
            .iter()
            .filter_map(|l| match l {
                HelpLine::Entry { keys, .. } => Some(Line::from(keys.as_str()).width()),
                HelpLine::Heading(_) => None,
            })
            .max()
            .unwrap_or(0)
            .min(KEYS);
        widths[start..end].fill(widest);
        start = end;
    }
    widths
}

#[cfg(test)]
mod tests {
    use monarch_core::Keymap;

    use super::*;
    use crate::OverlayKey;

    fn draw(state: &HelpState, w: u16, h: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        HelpView {
            state,
            prefix: "ctrl+b",
            chrome: Chrome::default(),
        }
        .render(buf.area, &mut buf);
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .map(|s| s.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn draws_headings_and_aligned_keys() {
        let mut state = HelpState::new(&Keymap::default());
        state.set_rows(8);
        insta::assert_snapshot!(draw(&state, 70, 10).join("\n"));
    }

    #[test]
    fn a_search_shows_in_the_border() {
        let mut state = HelpState::new(&Keymap::default());
        for c in "detach".chars() {
            state.key(OverlayKey::Char(c));
        }
        insta::assert_snapshot!(draw(&state, 70, 5).join("\n"));
        for c in "zzz".chars() {
            state.key(OverlayKey::Char(c));
        }
        assert!(draw(&state, 70, 5)[1].contains("no matches"));
        assert!(
            draw(&state, 10, 2).iter().all(|l| l.is_empty()),
            "too small"
        );
    }

    #[test]
    fn rows_leave_room_for_the_border() {
        assert_eq!(HelpView::body_rows(Rect::new(0, 0, 100, 30)), 28);
        assert_eq!(HelpView::placement(Rect::new(0, 0, 100, 30)).width, WIDTH);
    }
}
