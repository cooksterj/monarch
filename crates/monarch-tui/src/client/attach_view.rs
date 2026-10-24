//! Lays out one attached frame: the panes above, the status line below.

use monarch_proto::ScreenSize;
use ratatui::Frame;
use ratatui::layout::Rect;

use super::AttachState;
use crate::{PaneView, StatusLine};

/// Rows the client keeps for itself at the bottom of the host terminal.
const STATUS_ROWS: u16 = 1;

/// A pure view of the session for one draw.
pub struct AttachView<'a> {
    /// The session model built from frames.
    pub state: &'a AttachState,
    /// The bottom bar.
    pub status: StatusLine<'a>,
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
        for pane in &layout.panes {
            let Some(screen) = self.state.screen(pane.pane) else {
                continue;
            };
            let rect =
                Rect::new(pane.x, pane.y, pane.size.cols, pane.size.rows).intersection(pane_rect);
            let view = PaneView::new(screen);
            if Some(pane.pane) == layout.focus
                && let Some(pos) = view.cursor_position(rect)
            {
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
}
