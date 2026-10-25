//! Drawing one frame: host modes and title, then panes, status line and
//! the pop-ups inside a synchronized update.

use std::io::Write;
use std::time::Instant;

use monarch_core::Action;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

use super::AttachApp;
use crate::client::{AttachOptions, AttachView, WindowTitle};
use crate::{MouseOwner, ReviewView, Sidebar, SidebarGroup, StatusLine, TuiError, WhichKey};

impl AttachApp {
    /// Mirrors the focused pane onto the host, then draws every visible
    /// pane, inside one synchronized update when the host supports it.
    pub(super) fn draw(&mut self) -> Result<(), TuiError> {
        let focus = self.state.focus();
        if let Some(screen) = focus.and_then(|p| self.state.screen(p)) {
            self.mirror
                .sync(&screen.modes, &screen.cursor, &mut self.out);
        }
        let title = WindowTitle::format(
            &self.opts.window_title,
            &self.opts.session,
            focus,
            self.state.focused_title(),
        );
        self.mirror.sync_title(Some(&title), &mut self.out);
        let io = |e| TuiError::io("drawing", e);
        if self.host.sync_output {
            queue!(self.ui.backend_mut(), BeginSynchronizedUpdate).map_err(io)?;
        }
        self.ui.backend_mut().write_all(&self.out).map_err(io)?;
        self.out.clear();
        self.size_review();
        self.paint()?;
        if self.host.sync_output {
            execute!(self.ui.backend_mut(), EndSynchronizedUpdate).map_err(io)?;
        }
        self.dirty = false;
        self.last_draw = Instant::now();
        Ok(())
    }

    /// Draws panes, the status line and the key popup.
    fn paint(&mut self) -> Result<(), TuiError> {
        let groups = self.sidebar_rows();
        let (tabs, active_tab) = self.state.tab_labels();
        let copy = self.copy_view();
        let notice = self.notice.as_ref().map(|(text, _)| text.as_str());
        let (picker, menu, review, form) = self.popups.views(self.glyphs.theme());
        let view = AttachView {
            state: &self.state,
            status: StatusLine {
                tabs: &tabs,
                active_tab,
                pending: self.prefix_pending,
                message: notice.or(copy.as_ref().map(|c| c.status.as_str())),
                mouse: MouseOwner::new(self.mirror.captures_mouse()),
                ..session_status(&self.opts)
            },
            sidebar: Some(Sidebar { groups: &groups }),
            selection: copy
                .as_ref()
                .map_or_else(|| self.selection_span(), |c| c.selection),
            matches: copy.as_ref().map_or(&[][..], |c| &c.matches),
            copy_cursor: copy.as_ref().and_then(|c| c.cursor),
            picker,
            menu,
            review,
            form,
            which_key: (self.popup && !self.any_overlay()).then_some(WhichKey {
                entries: &self.which_key,
                title: &self.opts.prefix_label,
            }),
        };
        self.ui
            .draw(|frame| view.draw(frame))
            .map(drop)
            .map_err(|e| TuiError::io("drawing", e))
    }

    /// Tells the review pane how many diff lines a page holds.
    fn size_review(&mut self) {
        if let Some(review) = self.popups.review.as_mut() {
            let rows = AttachView::pane_area(self.size, self.state.left()).rows;
            review.set_page(ReviewView::diff_rows(rows));
        }
    }

    /// The sidebar's groups with animated glyphs; none while it is hidden.
    fn sidebar_rows(&mut self) -> Vec<SidebarGroup> {
        let mut groups = if self.state.left() > 0 {
            self.state.sidebar_groups()
        } else {
            Vec::new()
        };
        self.dress(&mut groups);
        groups
    }
}

/// The status line's session, prefix and detach key, with nothing else
/// shown yet.
fn session_status(opts: &AttachOptions) -> StatusLine<'_> {
    StatusLine {
        session: &opts.session,
        tabs: &[],
        active_tab: 0,
        prefix: &opts.prefix_label,
        detach: opts.keymap.key_for(Action::Detach),
        pending: false,
        message: None,
        mouse: MouseOwner::new(false),
    }
}
