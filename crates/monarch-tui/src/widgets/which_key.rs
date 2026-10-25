//! The popup listing what each key after the prefix does.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::WhichKeyEntry;

/// Space between columns.
const GAP: u16 = 3;

/// Entries in as many columns as fit, filled top to bottom, in a box at the
/// bottom of the pane area.
#[derive(Debug, Clone, Copy)]
pub struct WhichKey<'a> {
    /// What to list.
    pub entries: &'a [WhichKeyEntry],
    /// The box title (the prefix, e.g. `ctrl+b`).
    pub title: &'a str,
}

impl WhichKey<'_> {
    /// Where the popup goes inside `area`: centred, flush with the bottom.
    #[must_use]
    pub fn placement(&self, area: Rect) -> Rect {
        let inner = area.width.saturating_sub(2);
        let (cols, rows) = self.grid(inner);
        let width =
            (cols * self.column_width() + (cols.saturating_sub(1)) * GAP + 4).min(area.width);
        let height = (rows + 2).min(area.height);
        Rect::new(
            area.x + (area.width - width) / 2,
            area.bottom() - height,
            width,
            height,
        )
    }

    fn widths(&self) -> (u16, u16) {
        let width = |s: &str| u16::try_from(s.chars().count()).unwrap_or(u16::MAX);
        let keys = self.entries.iter().map(|e| width(&e.keys)).max();
        let label = self.entries.iter().map(|e| width(&e.label)).max();
        (keys.unwrap_or(0), label.unwrap_or(0))
    }

    fn column_width(&self) -> u16 {
        let (keys, label) = self.widths();
        keys + 2 + label
    }

    /// Columns and rows for `width` cells.
    fn grid(&self, width: u16) -> (u16, u16) {
        let count = u16::try_from(self.entries.len()).unwrap_or(u16::MAX);
        if count == 0 {
            return (1, 0);
        }
        let per = self.column_width() + GAP;
        let cols = ((width.saturating_sub(2) + GAP) / per.max(1)).clamp(1, count);
        (cols, count.div_ceil(cols))
    }
}

impl Widget for WhichKey<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = self.placement(area);
        if popup.width < 4 || popup.height < 3 {
            return;
        }
        Clear.render(popup, buf);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().add_modifier(Modifier::DIM))
            .title(format!(" {} ", self.title));
        let inner = block.inner(popup);
        block.render(popup, buf);
        let (cols, rows) = self.grid(area.width.saturating_sub(2));
        // A short host gets fewer rows; columns still fill top to bottom.
        let rows = rows.min(inner.height).max(1);
        let step = self.column_width() + GAP;
        let shown_cols = (0..cols)
            .take_while(|c| inner.x + 1 + c * step < inner.right())
            .count();
        let room = shown_cols * usize::from(rows);
        let mut cells: Vec<(&str, String)> = self
            .entries
            .iter()
            .map(|e| (e.keys.as_str(), e.label.clone()))
            .collect();
        if cells.len() > room && room > 0 {
            let hidden = cells.len() - (room - 1);
            cells.truncate(room - 1);
            cells.push(("…", format!("{hidden} more")));
        }
        for (i, (keys, label)) in (0u16..).zip(&cells) {
            let x = inner.x + 1 + (i / rows) * step;
            self.cell(buf, inner, (x, inner.y + i % rows), (keys, label));
        }
    }
}

impl WhichKey<'_> {
    /// One `key  label` pair at `at`, cut at the box's edge.
    fn cell(&self, buf: &mut Buffer, inner: Rect, at: (u16, u16), text: (&str, &str)) {
        let ((x, y), (keys, label)) = (at, text);
        if x >= inner.right() || y >= inner.bottom() {
            return;
        }
        let bold = Style::new().add_modifier(Modifier::BOLD);
        buf.set_stringn(x, y, keys, usize::from(inner.right() - x), bold);
        let label_x = x + self.widths().0 + 2;
        if label_x < inner.right() {
            let room = usize::from(inner.right() - label_x);
            buf.set_stringn(label_x, y, label, room, Style::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Keymap;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn draw(width: u16, height: u16) -> String {
        let entries = WhichKeyEntry::from_keymap(&Keymap::default());
        let popup = WhichKey {
            entries: &entries,
            title: "ctrl+b",
        };
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        term.draw(|f| f.render_widget(popup, f.area()))
            .expect("draw");
        let buf = term.backend().buffer();
        (0..height)
            .map(|y| {
                let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                row.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim_start_matches('\n')
            .to_owned()
    }

    #[test]
    fn short_hosts_say_how_many_keys_did_not_fit() {
        let text = draw(80, 6);
        assert!(text.contains("…"), "{text}");
        let more = text
            .lines()
            .find(|l| l.contains("more"))
            .expect("a more row");
        assert!(more.contains("more"), "{text}");
        assert!(!text.contains("detach"), "cut");
    }

    #[test]
    fn wide_hosts_get_columns() {
        insta::assert_snapshot!(draw(80, 11), @r#"
        ┌ ctrl+b ─────────────────────────────────────────────────────────────────────┐
        │ " -        split down     HJKL       resize         [          copy mode    │
        │ % |        split right    1-9        go to tab      a          inbox        │
        │ x          close pane     c          new tab        g w        go to pane   │
        │ z          zoom           n          next tab       m          toggle mouse │
        │ hjkl ←↓↑→  focus          p          prev tab       s          projects     │
        │ o          focus next     :          palette        t          sidebar      │
        │ {          swap prev      N          new worktree   v          review       │
        │ }          swap next      R          reload         tab        next blocked │
        │ =          equalize       S          sessions       d          detach       │
        └─────────────────────────────────────────────────────────────────────────────┘
        "#);
    }

    #[test]
    fn narrow_hosts_get_one_column_and_a_count_of_the_rest() {
        insta::assert_snapshot!(draw(24, 8), @r#"
        ┌ ctrl+b ──────────────┐
        │ " -        split down│
        │ % |        split righ│
        │ x          close pane│
        │ z          zoom      │
        │ hjkl ←↓↑→  focus     │
        │ …          22 more   │
        └──────────────────────┘
        "#);
    }

    #[test]
    fn nothing_bound_draws_nothing_but_the_box() {
        let popup = WhichKey {
            entries: &[],
            title: "ctrl+b",
        };
        let area = Rect::new(0, 0, 40, 10);
        assert_eq!(popup.placement(area).height, 2);
    }
}
