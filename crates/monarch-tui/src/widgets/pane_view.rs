//! Draws a pane's [`Screen`] into a ratatui buffer, cell for cell.

use monarch_proto::Screen;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

use super::{SelectionSpan, StyleConverter};

/// Renders a screen at the top-left of the area, clipped to it, with an
/// optional selection drawn in reverse video and search matches bold and
/// underlined.
pub struct PaneView<'a> {
    screen: &'a Screen,
    selection: Option<SelectionSpan>,
    matches: &'a [SelectionSpan],
}

impl<'a> PaneView<'a> {
    /// A view of `screen`.
    #[must_use]
    pub fn new(screen: &'a Screen) -> Self {
        Self {
            screen,
            selection: None,
            matches: &[],
        }
    }

    /// Marks search matches (viewport coordinates).
    #[must_use]
    pub fn with_matches(mut self, matches: &'a [SelectionSpan]) -> Self {
        self.matches = matches;
        self
    }

    /// Highlights `selection` (viewport coordinates).
    #[must_use]
    pub fn with_selection(mut self, selection: Option<SelectionSpan>) -> Self {
        self.selection = selection;
        self
    }

    /// Where the host cursor should be placed for this pane, if visible.
    #[must_use]
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let c = self.screen.cursor;
        let inside = c.visible && c.col < area.width && c.row < area.height;
        inside.then(|| Position::new(area.x + c.col, area.y + c.row))
    }
}

impl Widget for PaneView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (y_index, row) in self
            .screen
            .rows
            .iter()
            .take(usize::from(area.height))
            .enumerate()
        {
            let y = area.y + y_index as u16;
            for (x, cell) in row.iter().take(usize::from(area.width)).enumerate() {
                let Some(target) = buf.cell_mut((area.x + x as u16, y)) else {
                    continue;
                };
                // Wide-char spacers keep a blank symbol; ratatui skips the
                // cell after a double-width symbol when diffing.
                if cell.width == 0 {
                    target.set_symbol(" ");
                } else if cell.combining.is_empty() {
                    target.set_char(cell.ch);
                } else {
                    target.set_symbol(&cell.text());
                }
                target.set_style(StyleConverter::style(&cell.style));
                if self
                    .selection
                    .is_some_and(|s| s.contains(y_index as i64, x as u16))
                {
                    target.modifier.toggle(Modifier::REVERSED);
                }
                let (row, col) = (y_index as i64, x as u16);
                if self.matches.iter().any(|m| m.contains(row, col)) {
                    target
                        .modifier
                        .insert(Modifier::BOLD | Modifier::UNDERLINED);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monarch_proto::{Cell, CellStyle, ScreenSize, TermColor};
    use ratatui::style::Color;

    fn screen() -> Screen {
        let mut s = Screen::blank(ScreenSize::new(4, 2));
        s.rows[0][0] = Cell::new('h', 1);
        s.rows[0][1] = Cell {
            style: CellStyle {
                fg: TermColor::Indexed(1),
                ..CellStyle::default()
            },
            ..Cell::new('i', 1)
        };
        s.rows[1][0] = Cell::new('界', 2);
        s.rows[1][1] = Cell::spacer();
        s.cursor.row = 1;
        s.cursor.col = 2;
        s
    }

    #[test]
    fn draws_cells_with_styles() {
        let s = screen();
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        PaneView::new(&s).render(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "h");
        assert_eq!(buf[(1, 0)].fg, Color::Indexed(1));
        assert_eq!(buf[(0, 1)].symbol(), "界");
    }

    #[test]
    fn clips_to_a_smaller_area_with_offset() {
        let s = screen();
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        PaneView::new(&s).render(Rect::new(2, 1, 1, 1), &mut buf);
        assert_eq!(buf[(2, 1)].symbol(), "h");
        assert_eq!(buf[(3, 1)].symbol(), " ");
    }

    #[test]
    fn cursor_position_is_offset_and_hidden_when_outside() {
        let s = screen();
        let view = PaneView::new(&s);
        assert_eq!(
            view.cursor_position(Rect::new(5, 5, 4, 2)),
            Some(Position::new(7, 6))
        );
        assert_eq!(view.cursor_position(Rect::new(0, 0, 2, 2)), None);
    }

    #[test]
    fn selection_is_drawn_in_reverse_video() {
        let screen = screen();
        let span = SelectionSpan {
            start_row: 0,
            start_col: 1,
            end_row: 1,
            end_col: 0,
            block: false,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        PaneView::new(&screen)
            .with_selection(Some(span))
            .render(buf.area, &mut buf);
        let reversed = |x, y| buf[(x, y)].modifier.contains(Modifier::REVERSED);
        assert!(!reversed(0, 0) && reversed(1, 0) && reversed(3, 0));
        assert!(reversed(0, 1) && !reversed(1, 1));
    }

    #[test]
    fn matches_are_bold_and_underlined() {
        let s = screen();
        let m = [SelectionSpan {
            start_row: 0,
            start_col: 1,
            end_row: 0,
            end_col: 1,
            block: false,
        }];
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        PaneView::new(&s)
            .with_matches(&m)
            .render(buf.area, &mut buf);
        let marked = Modifier::BOLD | Modifier::UNDERLINED;
        assert!(buf[(1, 0)].modifier.contains(marked));
        assert!(!buf[(0, 0)].modifier.intersects(marked));
    }
}
