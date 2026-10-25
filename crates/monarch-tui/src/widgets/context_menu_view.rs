//! Draws the right-click menu: a small box beside the pointer, kept on
//! screen, each row with its key on the right.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::ContextMenu;

/// Columns between a label and its key.
const GAP: u16 = 3;

/// A context menu as drawn over the panes.
#[derive(Debug, Clone, Copy)]
pub struct ContextMenuView<'a> {
    /// What to draw.
    pub menu: &'a ContextMenu,
}

impl ContextMenuView<'_> {
    /// The box inside `area`: just right of and below the pointer, moved
    /// left or up when it would leave the area.
    #[must_use]
    pub fn placement(&self, area: Rect) -> Rect {
        let items = self.menu.items();
        let widest = |f: fn(&crate::PickerItem) -> &str| {
            items
                .iter()
                .map(|i| f(i).chars().count())
                .max()
                .unwrap_or(0)
        };
        let inner = widest(|i| &i.label) + usize::from(GAP) + widest(|i| &i.detail);
        let width = (u16::try_from(inner).unwrap_or(u16::MAX) + 4).min(area.width);
        let height = (u16::try_from(items.len()).unwrap_or(u16::MAX) + 2).min(area.height);
        let (col, row) = self.menu.at();
        let x = (col + 1)
            .min(area.right().saturating_sub(width))
            .max(area.x);
        let y = (row + 1)
            .min(area.bottom().saturating_sub(height))
            .max(area.y);
        Rect::new(x, y, width, height)
    }

    /// The row under host cell (`col`, `row`) when the box is placed in
    /// `area`.
    #[must_use]
    pub fn item_at(&self, area: Rect, col: u16, row: u16) -> Option<usize> {
        let inner = self
            .placement(area)
            .inner(ratatui::layout::Margin::new(1, 1));
        let inside =
            (inner.x..inner.right()).contains(&col) && (inner.y..inner.bottom()).contains(&row);
        inside.then(|| usize::from(row - inner.y))
    }
}

impl Widget for ContextMenuView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = self.placement(area);
        if popup.width < 6 || popup.height < 3 {
            return;
        }
        Clear.render(popup, buf);
        let dim = Style::new().add_modifier(Modifier::DIM);
        Block::new()
            .borders(Borders::ALL)
            .border_style(dim)
            .render(popup, buf);
        let inner = popup.inner(ratatui::layout::Margin::new(1, 1));
        let rows = self.menu.items().iter().enumerate();
        for (i, item) in rows.take(usize::from(inner.height)) {
            let y = inner.y + u16::try_from(i).unwrap_or(0);
            let style = if i == self.menu.selected() {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            buf.set_style(Rect::new(inner.x, y, inner.width, 1), style);
            let width = usize::from(inner.width.saturating_sub(1));
            buf.set_stringn(inner.x + 1, y, &item.label, width, style);
            let key_width = u16::try_from(item.detail.chars().count()).unwrap_or(0);
            let key_x = inner.right().saturating_sub(key_width + 1);
            buf.set_stringn(
                key_x,
                y,
                &item.detail,
                width,
                style.add_modifier(Modifier::DIM),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Keymap;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn draw(menu: &ContextMenu, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        term.draw(|f| f.render_widget(ContextMenuView { menu }, f.area()))
            .expect("draw");
        let buf = term.backend().buffer();
        (0..height)
            .map(|y| {
                let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                row.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn opens_beside_the_pointer() {
        let menu = ContextMenu::new(&Keymap::default(), 1, 0);
        insta::assert_snapshot!(draw(&menu, 30, 11), @r#"

          ┌─────────────────────┐
          │ split right       % │
          │ split down        " │
          │ zoom              z │
          │ close pane        x │
          │ new tab           c │
          │ copy mode         [ │
          │ toggle sidebar    t │
          │ command palette   : │
          └─────────────────────┘
        "#);
    }

    #[test]
    fn stays_on_screen_and_maps_clicks_to_rows() {
        let menu = ContextMenu::new(&Keymap::default(), 38, 18);
        let view = ContextMenuView { menu: &menu };
        let area = Rect::new(0, 0, 40, 20);
        let placed = view.placement(area);
        assert_eq!(
            (placed.right(), placed.bottom()),
            (40, 20),
            "moved left and up"
        );
        assert_eq!(view.item_at(area, placed.x + 2, placed.y + 1), Some(0));
        assert_eq!(view.item_at(area, placed.x + 2, placed.y + 3), Some(2));
        assert_eq!(
            view.item_at(area, placed.x, placed.y + 1),
            None,
            "the border"
        );
        assert_eq!(view.item_at(area, 0, 0), None, "outside");
    }
}
