//! Lays out one attached frame: the sidebar on the left, the panes beside
//! it, the status line below.

use monarch_proto::ScreenSize;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use super::AttachState;
use crate::{
    ContextMenuView, PaneBorders, PaneView, PickerView, SelectionSpan, Sidebar, StatusLine,
    WhichKey,
};

/// Rows the client keeps for itself at the bottom of the host terminal.
const STATUS_ROWS: u16 = 1;

/// A pure view of the session for one draw.
pub struct AttachView<'a> {
    /// The session model built from frames.
    pub state: &'a AttachState,
    /// The bottom bar.
    pub status: StatusLine<'a>,
    /// The sidebar, in the state's `left()` columns, when shown.
    pub sidebar: Option<Sidebar<'a>>,
    /// The focused pane's selection, in its viewport coordinates.
    pub selection: Option<SelectionSpan>,
    /// Search matches in the focused pane (copy mode).
    pub matches: &'a [SelectionSpan],
    /// The copy-mode cursor's cell in the focused pane, replacing the
    /// pane's own cursor.
    pub copy_cursor: Option<(u16, u16)>,
    /// The key popup, drawn over the panes while it is up.
    pub which_key: Option<WhichKey<'a>>,
    /// The palette or go-to picker, drawn over everything; the cursor
    /// sits in its query.
    pub picker: Option<PickerView<'a>>,
    /// The right-click menu, drawn over everything at the pointer.
    pub menu: Option<ContextMenuView<'a>>,
}

impl AttachView<'_> {
    /// The size the server lays panes out in for a host of `host` cells
    /// with `left` columns of sidebar.
    #[must_use]
    pub fn pane_area(host: ScreenSize, left: u16) -> ScreenSize {
        let rows = if host.rows > STATUS_ROWS {
            host.rows - STATUS_ROWS
        } else {
            host.rows
        };
        ScreenSize::new(host.cols.saturating_sub(left).max(1), rows)
    }

    /// Where the sidebar goes on a host of `host` cells.
    #[must_use]
    pub fn sidebar_area(host: Rect, left: u16) -> Rect {
        let panes = Self::pane_area(ScreenSize::new(host.width, host.height), left);
        Rect::new(host.x, host.y, left.min(host.width), panes.rows)
    }

    /// Draws panes, the focused cursor and the status line.
    pub fn draw(self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let left = self.state.left();
        let panes = Self::pane_area(ScreenSize::new(area.width, area.height), left);
        let pane_rect = Rect::new(area.x + left, area.y, panes.cols, panes.rows).intersection(area);
        if panes.rows < area.height {
            let status = Rect::new(area.x, area.y + panes.rows, area.width, STATUS_ROWS);
            frame.render_widget(self.status, status);
        }
        if let Some(sidebar) = self.sidebar.filter(|_| left > 0) {
            frame.render_widget(sidebar, Self::sidebar_area(area, left));
        }
        self.draw_panes(frame, pane_rect);
        if let Some(popup) = self.which_key {
            frame.render_widget(popup, pane_rect);
        }
        if let Some(picker) = self.picker {
            frame.render_widget(picker, pane_rect);
            frame.set_cursor_position(picker.cursor(pane_rect));
        }
        if let Some(menu) = self.menu {
            frame.render_widget(menu, area);
        }
    }

    fn draw_panes(&self, frame: &mut Frame<'_>, pane_rect: Rect) {
        let Some(layout) = self.state.layout() else {
            return;
        };
        let rect_of = |p: &monarch_proto::PaneLayout| {
            Rect::new(
                pane_rect.x + p.x,
                pane_rect.y + p.y,
                p.size.cols,
                p.size.rows,
            )
            .intersection(pane_rect)
        };
        let rects: Vec<Rect> = layout.panes.iter().map(rect_of).collect();
        let window = Rect::new(pane_rect.x, pane_rect.y, layout.size.cols, layout.size.rows);
        let borders = PaneBorders {
            panes: &rects,
            focused: layout.focused().map(rect_of),
        };
        frame.render_widget(borders, window.intersection(pane_rect));
        for pane in &layout.panes {
            let Some(screen) = self.state.screen(pane.pane) else {
                continue;
            };
            let rect = rect_of(pane);
            let focused = Some(pane.pane) == layout.focus;
            let view = PaneView::new(screen)
                .with_selection(self.selection.filter(|_| focused))
                .with_matches(if focused { self.matches } else { &[] });
            let cursor = match self.copy_cursor {
                Some((row, col)) => (col < rect.width && row < rect.height)
                    .then(|| Position::new(rect.x + col, rect.y + row)),
                None => view.cursor_position(rect),
            };
            if focused && let Some(pos) = cursor {
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
            tabs: &[],
            active_tab: 0,
            prefix: "ctrl+b",
            detach: Some(monarch_core::Key::Char('d')),
            pending: false,
            message: None,
        }
    }

    #[test]
    fn panes_get_all_rows_but_the_status_line() {
        assert_eq!(
            AttachView::pane_area(ScreenSize::new(80, 24), 0),
            ScreenSize::new(80, 23)
        );
        assert_eq!(
            AttachView::pane_area(ScreenSize::new(80, 1), 0),
            ScreenSize::new(80, 1)
        );
        assert_eq!(
            AttachView::pane_area(ScreenSize::new(120, 24), 32),
            ScreenSize::new(88, 23),
            "the sidebar takes columns"
        );
        assert_eq!(
            AttachView::sidebar_area(Rect::new(0, 0, 120, 24), 32),
            Rect::new(0, 0, 32, 23)
        );
    }

    #[test]
    fn before_a_layout_only_the_status_line_is_drawn() {
        let mut term = Terminal::new(TestBackend::new(40, 3)).expect("terminal");
        let state = AttachState::new();
        let view = AttachView {
            state: &state,
            status: status(),
            sidebar: None,
            selection: None,
            matches: &[],
            copy_cursor: None,
            which_key: None,
            picker: None,
            menu: None,
        };
        term.draw(|f| view.draw(f)).expect("draw");
        let buf = term.backend().buffer();
        let rows: Vec<String> = (0..3)
            .map(|y| (0..40).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        assert_eq!(rows[0].trim(), "");
        assert_eq!(rows[1].trim(), "");
        assert_eq!(rows[2], " monarch │ s │          ctrl+b d detach ");
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
            tabs: Vec::new(),
            active_tab: 0,
        };
        state.apply(ServerMsg::Layout(layout), &mut crate::Applied::default());
        let mut term = Terminal::new(TestBackend::new(9, 3)).expect("terminal");
        let view = AttachView {
            state: &state,
            status: status(),
            sidebar: None,
            selection: None,
            matches: &[],
            copy_cursor: None,
            which_key: None,
            picker: None,
            menu: None,
        };
        term.draw(|f| view.draw(f)).expect("draw");
        let buf = term.backend().buffer();
        assert_eq!(buf[(4, 0)].symbol(), "│");
        assert_eq!(buf[(4, 1)].symbol(), "│");
        assert_eq!(buf[(3, 0)].symbol(), " ", "pane cells are not borders");
    }

    /// Two 3-column panes side by side in one tab, pane 1 focused.
    fn two_panes() -> AttachState {
        use monarch_core::PaneId;
        use monarch_proto::{LayoutMsg, PaneLayout, PaneSummary, ServerMsg, TabSummary};

        let pane = |id, x| PaneLayout {
            pane: PaneId(id),
            x,
            y: 0,
            size: ScreenSize::new(3, 2),
            title: None,
        };
        let summary = |id| PaneSummary {
            pane: PaneId(id),
            title: Some(format!("p{id}")),
            command: None,
            place: None,
        };
        let layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(7, 2),
            focus: Some(PaneId(1)),
            panes: vec![pane(1, 0), pane(2, 4)],
            tabs: vec![TabSummary {
                focus: PaneId(1),
                zoomed: false,
                panes: vec![summary(1), summary(2)],
            }],
            active_tab: 0,
        };
        let mut state = AttachState::new();
        state.apply(ServerMsg::Layout(layout), &mut crate::Applied::default());
        state
    }

    #[test]
    fn panes_sit_right_of_the_sidebar() {
        let mut state = two_panes();
        state.set_left(8);
        let groups = state.sidebar_groups();
        let mut term = Terminal::new(TestBackend::new(15, 3)).expect("terminal");
        let view = AttachView {
            state: &state,
            status: status(),
            sidebar: Some(Sidebar { groups: &groups }),
            selection: None,
            matches: &[],
            copy_cursor: None,
            which_key: None,
            picker: None,
            menu: None,
        };
        term.draw(|f| view.draw(f)).expect("draw");
        let buf = term.backend().buffer();
        let row = |y| (0..15).map(|x| buf[(x, y)].symbol()).collect::<String>();
        assert_eq!(row(0), "▾ Scrat│   │   ");
        assert_eq!(row(1), "▸ 1 p1 │   │   ");
        assert_eq!(state.pane_rects()[1].1.x, 12, "clicks use host columns");
        assert_eq!(state.focus_view().x, 8);
    }
}
