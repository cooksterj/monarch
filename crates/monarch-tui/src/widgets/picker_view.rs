//! Draws an open picker: a box near the top with the query and the rows
//! that match it.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::{Picker, PickerItem};

/// Widest the box gets.
const WIDTH: u16 = 64;
/// Most rows shown at once; the list scrolls to keep the highlight in view.
const ROWS: u16 = 12;
/// Border, query line and border.
const CHROME: u16 = 3;
const PROMPT: &str = "> ";
const SEARCH_KEYS: &str = " ↑↓ move · enter run · esc close ";
const LIST_KEYS: &str = " ↑↓ move · enter jump · y/n/1-9 answer · esc close ";

/// A picker as drawn over the panes.
#[derive(Debug, Clone, Copy)]
pub struct PickerView<'a> {
    /// What to draw.
    pub picker: &'a Picker,
}

impl PickerView<'_> {
    /// Where the box goes inside `area`: centred, a little below the top.
    #[must_use]
    pub fn placement(&self, area: Rect) -> Rect {
        let rows = u16::try_from(self.picker.rows().len()).unwrap_or(u16::MAX);
        let height = (rows.clamp(1, ROWS) + self.chrome()).min(area.height);
        let width = WIDTH.min(area.width);
        let top = (area.height - height) / 5;
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + top,
            width,
            height,
        )
    }

    /// Where the cursor sits: after the query; a list has none.
    #[must_use]
    pub fn cursor(&self, area: Rect) -> Option<Position> {
        if self.picker.is_list() {
            return None;
        }
        let inner = inner(self.placement(area));
        let typed = Line::from(format!("{PROMPT}{}", self.picker.query())).width();
        let x = inner.x + 1 + u16::try_from(typed).unwrap_or(u16::MAX);
        Some(Position::new(
            x.min(inner.right().saturating_sub(1)),
            inner.y,
        ))
    }

    /// Rows taken by the border and, in a search, the query line.
    fn chrome(&self) -> u16 {
        if self.picker.is_list() {
            CHROME - 1
        } else {
            CHROME
        }
    }

    fn draw_rows(&self, area: Rect, buf: &mut Buffer) {
        if self.picker.rows().len() == 0 {
            let dim = Style::new().add_modifier(Modifier::DIM);
            let empty = if self.picker.is_list() {
                "nothing needs you"
            } else {
                "no matches"
            };
            buf.set_stringn(area.x + 1, area.y, empty, usize::from(area.width), dim);
            return;
        }
        let shown = usize::from(area.height);
        let skip = (self.picker.selected() + 1).saturating_sub(shown);
        for (i, (item, chars)) in self.picker.rows().enumerate().skip(skip).take(shown) {
            let y = area.y + u16::try_from(i - skip).unwrap_or(0);
            let row = Rect::new(area.x, y, area.width, 1);
            draw_row(row, item, chars, i == self.picker.selected(), buf);
        }
    }
}

impl Widget for PickerView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = self.placement(area);
        if popup.width < 8 || popup.height < self.chrome() + 1 {
            return;
        }
        Clear.render(popup, buf);
        let dim = Style::new().add_modifier(Modifier::DIM);
        let keys = if self.picker.is_list() {
            LIST_KEYS
        } else {
            SEARCH_KEYS
        };
        Block::new()
            .borders(Borders::ALL)
            .border_style(dim)
            .title(format!(" {} ", self.picker.title()))
            .title_bottom(Line::styled(keys, dim))
            .render(popup, buf);
        let inner = inner(popup);
        if self.picker.is_list() {
            self.draw_rows(inner, buf);
            return;
        }
        let width = usize::from(inner.width.saturating_sub(1));
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let x = buf.set_stringn(inner.x + 1, inner.y, PROMPT, width, bold).0;
        let room = usize::from(inner.right().saturating_sub(x));
        buf.set_stringn(x, inner.y, self.picker.query(), room, Style::new());
        let list = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        self.draw_rows(list, buf);
    }
}

fn inner(popup: Rect) -> Rect {
    Block::new().borders(Borders::ALL).inner(popup)
}

/// The label with matched characters bold, the detail dim at the right;
/// the highlighted row in reverse video.
fn draw_row(row: Rect, item: &PickerItem, chars: &[u32], selected: bool, buf: &mut Buffer) {
    let base = if selected {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new()
    };
    buf.set_style(row, base);
    let detail_width = u16::try_from(Line::from(item.detail.as_str()).width()).unwrap_or(0);
    let detail_x = row.right().saturating_sub(detail_width + 1);
    let mut x = row.x + 1;
    let mut text = [0u8; 4];
    for (i, c) in (0u32..).zip(item.label.chars()) {
        let room = usize::from(detail_x.saturating_sub(x + 1));
        if room == 0 {
            break;
        }
        let style = if chars.binary_search(&i).is_ok() {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        };
        x = buf
            .set_stringn(x, row.y, c.encode_utf8(&mut text), room, style)
            .0;
    }
    if detail_x > row.x + 1 {
        let dim = base.add_modifier(Modifier::DIM);
        buf.set_stringn(
            detail_x,
            row.y,
            &item.detail,
            usize::from(detail_width),
            dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{Action, SplitAxis};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::OverlayKey;

    fn picker(n: usize) -> Picker {
        let mut items = vec![
            PickerItem::for_action(Action::Split(SplitAxis::Columns), "%"),
            PickerItem::for_action(Action::Split(SplitAxis::Rows), "\""),
        ];
        items.extend((1..=n).map(|t| PickerItem::for_action(Action::SelectTab(t as u8), "")));
        Picker::new("command palette", items)
    }

    fn draw(p: &Picker, width: u16, height: u16) -> (String, Buffer) {
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        term.draw(|f| f.render_widget(PickerView { picker: p }, f.area()))
            .expect("draw");
        let buf = term.backend().buffer().clone();
        let text = (0..height)
            .map(|y| {
                let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                row.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, buf)
    }

    #[test]
    fn lists_rows_with_keys_and_a_footer() {
        let mut p = picker(1);
        p.key(OverlayKey::Char('s'));
        insta::assert_snapshot!(draw(&p, 40, 8).0, @r#"
        ┌ command palette ─────────────────────┐
        │ > s                                  │
        │ split right                        % │
        │ split down                         " │
        └ ↑↓ move · enter run · esc close ─────┘
        "#);
    }

    #[test]
    fn the_highlight_is_reversed_and_matches_are_bold() {
        let mut p = picker(0);
        p.key(OverlayKey::Char('d'));
        let (_, buf) = draw(&p, 40, 8);
        let area = Rect::new(0, 0, 40, 8);
        let top = PickerView { picker: &p }.placement(area).y + 2;
        assert!(buf[(5, top)].modifier.contains(Modifier::REVERSED));
        let bold: String = (0..40)
            .filter(|&x| buf[(x, top)].modifier.contains(Modifier::BOLD))
            .map(|x| buf[(x, top)].symbol().to_owned())
            .collect();
        assert_eq!(bold, "d");
    }

    #[test]
    fn long_lists_scroll_to_keep_the_highlight_in_view() {
        let mut p = picker(20);
        for _ in 0..15 {
            p.key(OverlayKey::Down);
        }
        let (text, _) = draw(&p, 40, 24);
        assert!(text.contains("tab 14"), "{text}");
        assert!(!text.contains("split right"), "scrolled past the top");
    }

    #[test]
    fn nothing_matching_says_so_and_the_cursor_follows_the_query() {
        let mut p = picker(0);
        for c in "zzz".chars() {
            p.key(OverlayKey::Char(c));
        }
        let (text, _) = draw(&p, 40, 8);
        assert!(text.contains("no matches"));
        let area = Rect::new(0, 0, 40, 8);
        let view = PickerView { picker: &p };
        let inner = inner(view.placement(area));
        assert_eq!(view.cursor(area), Some(Position::new(inner.x + 6, inner.y)));
    }

    #[test]
    fn tiny_hosts_draw_nothing() {
        let (text, _) = draw(&picker(0), 6, 3);
        assert_eq!(text.trim(), "");
    }

    #[test]
    fn lists_have_no_query_line_or_cursor() {
        let rows = vec![PickerItem {
            label: "◆ 2 codex · Approve cargo test?".into(),
            detail: "blocked".into(),
            action: Action::FocusPane(monarch_core::PaneId(4)),
        }];
        let list = Picker::list("inbox", rows);
        insta::assert_snapshot!(draw(&list, 60, 6).0, @r"
        ┌ inbox ───────────────────────────────────────────────────┐
        │ ◆ 2 codex · Approve cargo test?                  blocked │
        └ ↑↓ move · enter jump · y/n/1-9 answer · esc close ───────┘
        ");
        let area = Rect::new(0, 0, 60, 6);
        assert_eq!(PickerView { picker: &list }.cursor(area), None);
        let empty = Picker::list("inbox", Vec::new());
        assert!(draw(&empty, 60, 6).0.contains("nothing needs you"));
    }
}
