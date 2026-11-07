//! Draws a page of headings and labelled lines in a box: what the help
//! page and the agent info share.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::{Chrome, HelpLine};

/// Widest the keys column gets; longer keys are cut.
const KEYS: usize = 24;

/// A box with a title, lines from `scroll` on, and a footer.
#[derive(Debug, Clone)]
pub struct PageView<'a> {
    /// The title, in the top border.
    pub title: String,
    /// The footer, in the bottom border.
    pub footer: String,
    /// Every line, before scrolling.
    pub lines: Vec<&'a HelpLine>,
    /// The first line shown.
    pub scroll: usize,
    /// What is said when there are no lines.
    pub empty: &'a str,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl PageView<'_> {
    /// Lines a box `outer` shows at once.
    #[must_use]
    pub fn body_rows(outer: Rect) -> usize {
        usize::from(outer.height.saturating_sub(2))
    }

    /// ` 3–12 of 20 ·` when the lines overflow `rows`, else nothing.
    #[must_use]
    pub fn place(&self, rows: usize) -> String {
        let total = self.lines.len();
        let shown = rows.min(total.saturating_sub(self.scroll));
        if total > shown {
            let first = self.scroll + 1;
            format!(" {first}–{} of {total} ·", first + shown - 1)
        } else {
            String::new()
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

impl Widget for PageView<'_> {
    /// Draws the box over `outer`; nothing when it is too small.
    fn render(self, outer: Rect, buf: &mut Buffer) {
        if outer.height < 3 || outer.width < 20 {
            return;
        }
        let rows = Self::body_rows(outer);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(self.title.clone()).style(self.chrome.highlight()))
            .title_bottom(self.footer.clone());
        let inner = block.inner(outer);
        Clear.render(outer, buf);
        block.render(outer, buf);
        let widths = keys_widths(&self.lines);
        let body = self.lines.iter().zip(widths).skip(self.scroll).take(rows);
        for (y, (line, width)) in (inner.y..inner.bottom()).zip(body) {
            let row = Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1);
            self.draw_line(line, width, row, buf);
        }
        if self.lines.is_empty() {
            let dim = Style::new().add_modifier(Modifier::DIM);
            buf.set_stringn(
                inner.x + 1,
                inner.y,
                self.empty,
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
    use super::*;

    fn page<'a>(lines: &'a [HelpLine], scroll: usize) -> PageView<'a> {
        PageView {
            title: " page ".into(),
            footer: " foot ".into(),
            lines: lines.iter().collect(),
            scroll,
            empty: "nothing",
            chrome: Chrome::default(),
        }
    }

    fn draw(view: PageView<'_>, w: u16, h: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        view.render(buf.area, &mut buf);
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .map(|s| s.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn sections_align_their_own_keys_and_scroll_is_reported() {
        let lines = [
            HelpLine::Heading("One".into()),
            HelpLine::entry("a", "first"),
            HelpLine::entry("longer keys", "second"),
            HelpLine::Heading("Two".into()),
            HelpLine::entry("b", "third"),
        ];
        let drawn = draw(page(&lines, 0), 40, 7);
        assert_eq!(drawn[1], "│ One                                  │");
        assert_eq!(drawn[2], "│   a            first                 │");
        assert_eq!(drawn[3], "│   longer keys  second                │");
        assert_eq!(drawn[5], "│   b  third                           │");
        assert_eq!(page(&lines, 0).place(5), "");
        assert_eq!(page(&lines, 1).place(3), " 2–4 of 5 ·");
        let scrolled = draw(page(&lines, 3), 40, 4);
        assert_eq!(scrolled[1], "│ Two                                  │");
        assert!(draw(page(&[], 0), 40, 4)[1].contains("nothing"));
        assert!(draw(page(&lines, 0), 10, 2).iter().all(String::is_empty));
    }
}
