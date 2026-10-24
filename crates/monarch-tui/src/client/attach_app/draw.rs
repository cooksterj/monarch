//! Drawing one frame: host modes and title, then panes, status line and
//! the key popup inside a synchronized update.

use std::io::Write;
use std::time::Instant;

use monarch_core::Action;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

use super::AttachApp;
use crate::client::{AttachView, WindowTitle};
use crate::{StatusLine, TuiError, WhichKey};

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
        let (tabs, active_tab) = self.state.tab_labels();
        let view = AttachView {
            state: &self.state,
            status: StatusLine {
                session: &self.opts.session,
                tabs: &tabs,
                active_tab,
                prefix: &self.opts.prefix_label,
                detach: self.opts.keymap.key_for(Action::Detach),
                pending: self.prefix_pending,
                message: self.notice.as_ref().map(|(text, _)| text.as_str()),
            },
            selection: self.selection_span(),
            which_key: self.popup.then_some(WhichKey {
                entries: &self.which_key,
                title: &self.opts.prefix_label,
            }),
        };
        self.ui
            .draw(|frame| view.draw(frame))
            .map_err(|e| TuiError::io("drawing", e))?;
        Ok(())
    }
}
