//! The popup listing what each key after the prefix does.

use monarch_core::Action;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::{Chrome, WhichKeyEntry};

/// Space between columns: more than the two cells between a key and its
/// label, so a label reads with its own key and not the next column's.
const GAP: u16 = 3;

/// Entries in as many columns as fit, filled top to bottom, in a box at the
/// bottom of the pane area. Keys are right-aligned against their labels and
/// drawn in the accent, so each pair reads as one. The help key sits in the
/// bottom border, so it shows however short the host is.
#[derive(Debug, Clone, Copy, Default)]
pub struct WhichKey<'a> {
    /// What to list.
    pub entries: &'a [WhichKeyEntry],
    /// The box title (the prefix, e.g. `ctrl+b`).
    pub title: &'a str,
    /// The key colour.
    pub chrome: Chrome<'a>,
}

impl WhichKey<'_> {
    /// The entries listed in rows: all but the help key.
    fn rows(&self) -> impl Iterator<Item = &WhichKeyEntry> {
        self.entries.iter().filter(|e| e.action != Action::Help)
    }

    /// ` ? all keys ` for the bottom border, when help has a key.
    fn help(&self) -> Option<String> {
        let help = self.entries.iter().find(|e| e.action == Action::Help)?;
        Some(format!(" {} all keys and help ", help.keys))
    }

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
        let keys = self.rows().map(|e| width(&e.keys)).max();
        let label = self.rows().map(|e| width(&e.label)).max();
        (keys.unwrap_or(0), label.unwrap_or(0))
    }

    fn column_width(&self) -> u16 {
        let (keys, label) = self.widths();
        keys + 2 + label
    }

    /// Columns and rows for `width` cells.
    fn grid(&self, width: u16) -> (u16, u16) {
        let count = u16::try_from(self.rows().count()).unwrap_or(u16::MAX);
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
        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().add_modifier(Modifier::DIM))
            .title(format!(" {} ", self.title));
        if let Some(help) = self.help() {
            block = block.title_bottom(help);
        }
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
            .rows()
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
    /// One `key  label` pair at `at`, the key right-aligned to the label,
    /// cut at the box's edge.
    fn cell(&self, buf: &mut Buffer, inner: Rect, at: (u16, u16), text: (&str, &str)) {
        let ((x, y), (keys, label)) = (at, text);
        if x >= inner.right() || y >= inner.bottom() {
            return;
        }
        let keys_width = self.widths().0;
        let shown = u16::try_from(keys.chars().count()).unwrap_or(u16::MAX);
        let keys_x = (x + keys_width.saturating_sub(shown)).min(inner.right());
        let room = usize::from(inner.right() - keys_x);
        buf.set_stringn(keys_x, y, keys, room, self.chrome.key());
        let label_x = x + keys_width + 2;
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
            chrome: Chrome::default(),
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
        insta::assert_snapshot!(draw(80, 12), @r#"
        ┌ ctrl+b ─────────────────────────────────────────────────────────────────────┐
        │       " -  split down           1-9  go to tab              S  sessions     │
        │       % |  split right            c  new tab                [  copy mode    │
        │         x  close pane             n  next tab               a  inbox        │
        │         z  zoom                   p  prev tab             g w  go to pane   │
        │ hjkl ←↓↑→  focus                  ,  rename tab             i  agent info   │
        │         o  focus next             .  rename pane            m  toggle mouse │
        │         {  swap prev              :  palette                s  projects     │
        │         }  swap next              A  launch agent           t  sidebar      │
        │         =  equalize               N  new worktree           v  review       │
        │      HJKL  resize                 R  reload                 …  2 more       │
        └ ? all keys and help ────────────────────────────────────────────────────────┘
        "#);
    }

    #[test]
    fn narrow_hosts_get_one_column_and_a_count_of_the_rest() {
        insta::assert_snapshot!(draw(24, 8), @r#"
        ┌ ctrl+b ──────────────┐
        │       " -  split down│
        │       % |  split righ│
        │         x  close pane│
        │         z  zoom      │
        │ hjkl ←↓↑→  focus     │
        │         …  26 more   │
        └ ? all keys and help ─┘
        "#);
    }

    #[test]
    fn keys_take_the_accent_and_labels_do_not() {
        let theme = monarch_core::Theme::dark();
        let entries = WhichKeyEntry::from_keymap(&Keymap::default());
        let popup = WhichKey {
            entries: &entries,
            title: "ctrl+b",
            chrome: Chrome::new(Some(&theme)),
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        popup.render(buf.area, &mut buf);
        // Row 3 is `x  close pane`: the key right-aligned at column 10.
        assert_eq!(buf[(10, 3)].symbol(), "x");
        assert_eq!(buf[(10, 3)].fg, Chrome::color(theme.accent));
        assert_eq!(buf[(13, 3)].symbol(), "c");
        assert_eq!(buf[(13, 3)].fg, ratatui::style::Color::Reset);
    }

    #[test]
    fn nothing_bound_draws_nothing_but_the_box() {
        let popup = WhichKey {
            entries: &[],
            title: "ctrl+b",
            chrome: Chrome::default(),
        };
        let area = Rect::new(0, 0, 40, 10);
        assert_eq!(popup.placement(area).height, 2);
    }
}
