//! Lays out one attached frame: the panes above, the status line below.

use monarch_proto::ScreenSize;
use ratatui::Frame;
use ratatui::layout::Rect;

use super::AttachState;
use crate::{PaneBorders, PaneView, SelectionSpan, StatusLine};

/// Rows the client keeps for itself at the bottom of the host terminal.
const STATUS_ROWS: u16 = 1;

/// A pure view of the session for one draw.
pub struct AttachView<'a> {
    /// The session model built from frames.
    pub state: &'a AttachState,
    /// The bottom bar.
    pub status: StatusLine<'a>,
    /// The focused pane's selection, in its viewport coordinates.
    pub selection: Option<SelectionSpan>,
}

impl AttachView<'_> {
    /// The size the server lays panes out in for a host of `host` cells.
    #[must_use]
    pub fn pane_area(host: ScreenSize) -> ScreenSize {
        let rows = if host.rows > STATUS_ROWS {
            host.rows - STATUS_ROWS
        } else {
            host.rows
        };
        ScreenSize::new(host.cols, rows)
    }

    /// Draws panes, the focused cursor and the status line.
    pub fn draw(self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let panes = Self::pane_area(ScreenSize::new(area.width, area.height));
        let pane_rect = Rect::new(area.x, area.y, panes.cols, panes.rows);
        if panes.rows < area.height {
            let status = Rect::new(area.x, area.y + panes.rows, area.width, STATUS_ROWS);
            frame.render_widget(self.status, status);
        }
        let Some(layout) = self.state.layout() else {
            return;
        };
        let rect_of = |p: &monarch_proto::PaneLayout| {
            Rect::new(p.x, p.y, p.size.cols, p.size.rows).intersection(pane_rect)
        };
        let rects: Vec<Rect> = layout.panes.iter().map(rect_of).collect();
        let window = Rect::new(area.x, area.y, layout.size.cols, layout.size.rows);
        let borders = PaneBorders {
            panes: &rects,
            focused: layout.focused().map(rect_of),
        };
        frame.render_widget(borders, window.intersection(pane_rect));
        for pane in &layout.panes {
            let Some(screen) = self.state.screen(pane.pane) else {
                continue;
            };
            let rect =
                Rect::new(pane.x, pane.y, pane.size.cols, pane.size.rows).intersection(pane_rect);
            let focused = Some(pane.pane) == layout.focus;
            let view = PaneView::new(screen).with_selection(self.selection.filter(|_| focused));
            if focused && let Some(pos) = view.cursor_position(rect) {
                frame.set_cursor_position(pos);
            }
            frame.render_widget(view, rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn status() -> StatusLine<'static> {
        StatusLine {
            session: "s",
            title: None,
            prefix: "ctrl+b",
            pending: false,
            message: None,
        }
    }

    #[test]
    fn panes_get_all_rows_but_the_status_line() {
        assert_eq!(
            AttachView::pane_area(ScreenSize::new(80, 24)),
            ScreenSize::new(80, 23)
        );
        assert_eq!(
            AttachView::pane_area(ScreenSize::new(80, 1)),
            ScreenSize::new(80, 1)
        );
    }

    #[test]
    fn before_a_layout_only_the_status_line_is_drawn() {
        let mut term = Terminal::new(TestBackend::new(40, 3)).expect("terminal");
        let state = AttachState::new();
        let view = AttachView {
            state: &state,
            status: status(),
            selection: None,
        };
        term.draw(|f| view.draw(f)).expect("draw");
        let buf = term.backend().buffer();
        let rows: Vec<String> = (0..3)
            .map(|y| (0..40).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        assert_eq!(rows[0].trim(), "");
        assert_eq!(rows[1].trim(), "");
        assert_eq!(rows[2], " monarch │ s            ctrl+b d detach ");
    }

    #[test]
    fn split_panes_are_divided_by_a_line() {
        use monarch_core::PaneId;
        use monarch_proto::{LayoutMsg, PaneLayout, ServerMsg};

        let pane = |id, x| PaneLayout {
            pane: PaneId(id),
            x,
            y: 0,
            size: ScreenSize::new(4, 2),
            title: None,
        };
        let mut state = AttachState::new();
        let layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(9, 2),
            focus: Some(PaneId(1)),
            panes: vec![pane(1, 0), pane(2, 5)],
        };
        state.apply(ServerMsg::Layout(layout), &mut crate::Applied::default());
        let mut term = Terminal::new(TestBackend::new(9, 3)).expect("terminal");
        let view = AttachView {
            state: &state,
            status: status(),
            selection: None,
        };
        term.draw(|f| view.draw(f)).expect("draw");
        let buf = term.backend().buffer();
        assert_eq!(buf[(4, 0)].symbol(), "│");
        assert_eq!(buf[(4, 1)].symbol(), "│");
        assert_eq!(buf[(3, 0)].symbol(), " ", "pane cells are not borders");
    }
}
