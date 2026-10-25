//! Mouse selection, copy and the mouse passthrough toggle.

use std::thread;

use monarch_core::{Action, MouseMode};
use monarch_proto::{ClientMsg, SelectShape};
use std::time::{Duration, Instant};

use super::AttachApp;
use crate::client::{DragScroll, SelectEvent, SelectUnit, Selection};
use crate::{HostClipboard, LocalClipboard, SelectionSpan, SystemClipboard};

/// How long a status-line notice stays up.
pub(super) const NOTICE: Duration = Duration::from_secs(2);

impl AttachApp {
    /// Applies a selection gesture to the focused pane.
    pub(super) fn on_select(&mut self, gesture: SelectEvent) {
        let Some(pane) = self.state.focus() else {
            return;
        };
        let Some(screen) = self.state.screen(pane) else {
            return;
        };
        if !matches!(gesture, SelectEvent::Extend { .. }) {
            self.drag = None;
        }
        match gesture {
            SelectEvent::Start { col, row, clicks } => {
                self.selection = Some(Selection::start(
                    screen,
                    row,
                    col,
                    SelectUnit::from_clicks(clicks),
                ));
            }
            SelectEvent::Extend { col, row, beyond } => {
                if let Some(selection) = &mut self.selection {
                    selection.extend(screen, row, col);
                    self.drag = DragScroll::follow(self.drag, beyond, col, Instant::now());
                }
            }
            SelectEvent::End => match self.selection.filter(|s| !s.is_empty()) {
                // The server reads the text: lines scrolled out of view
                // are included and wrapped lines come back whole.
                Some(selection) => {
                    let (start, end) = selection.bounds();
                    let _ = self.link.send(&ClientMsg::Copy {
                        pane,
                        start,
                        end,
                        shape: SelectShape::Chars,
                    });
                }
                None => self.selection = None,
            },
        }
        self.dirty = true;
    }

    /// While a drag holds the pointer past the pane's edge, scrolls one
    /// step when due and keeps the selection's head on the edge row.
    pub(super) fn drag_tick(&mut self) {
        let Some(pane) = self.state.focus() else {
            return;
        };
        let Some(lines) = self.drag.as_mut().and_then(|d| d.step(Instant::now())) else {
            return;
        };
        let _ = self.link.send(&ClientMsg::Scroll { pane, lines });
        self.drag_extend();
    }

    /// Moves the selection's head to the edge the drag is past, in the
    /// pane's latest screen (so text scrolled in is selected).
    pub(super) fn drag_extend(&mut self) {
        let Some(screen) = self.state.focus().and_then(|p| self.state.screen(p)) else {
            return;
        };
        let (Some(drag), Some(selection)) = (self.drag, self.selection.as_mut()) else {
            return;
        };
        let rows = u16::try_from(screen.rows.len()).unwrap_or(u16::MAX);
        let (row, col) = drag.head(rows);
        selection.extend(screen, row, col);
        self.dirty = true;
    }

    /// `prefix m`: hand the mouse to the host terminal (native selection,
    /// its own menus) or take it back.
    pub(super) fn toggle_mouse(&mut self) {
        self.passthrough = !self.passthrough;
        let (policy, notice) = if self.passthrough {
            (
                MouseMode::Off,
                match self.opts.keymap.key_for(Action::ToggleMouse) {
                    Some(key) => format!(
                        "mouse: terminal ({} {key} to restore)",
                        self.opts.prefix_label
                    ),
                    None => "mouse: terminal".to_owned(),
                },
            )
        } else {
            (self.opts.mouse, "mouse: monarch".to_owned())
        };
        self.mirror.set_mouse(policy);
        self.selection = None;
        self.notice = Some((notice, Instant::now() + NOTICE));
        self.dirty = true;
    }

    /// Typing ends a selection, as in a bare terminal.
    pub(super) fn on_typed(&mut self) {
        if self.selection.take().is_some() {
            self.dirty = true;
        }
    }

    /// Sends `text` to the host clipboard (OSC 52) and, when this client is
    /// not running over SSH, to the local clipboard too, since some hosts
    /// ignore OSC 52.
    pub(super) fn copy(&mut self, text: String) {
        self.out.extend(HostClipboard::osc52(&text));
        let count = text.chars().count();
        if is_local() {
            thread::spawn(move || SystemClipboard.copy(&text));
        }
        let unit = if count == 1 {
            "character"
        } else {
            "characters"
        };
        self.notice = Some((format!("copied {count} {unit}"), Instant::now() + NOTICE));
    }

    /// Drops an expired notice; returns whether the screen changed.
    pub(super) fn expire_notice(&mut self) {
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, until)| Instant::now() >= *until)
        {
            self.notice = None;
            self.dirty = true;
        }
    }

    /// The selection in the focused pane's viewport coordinates.
    pub(super) fn selection_span(&self) -> Option<SelectionSpan> {
        let screen = self.state.focus().and_then(|p| self.state.screen(p))?;
        Some(self.selection.filter(|s| !s.is_empty())?.span(screen))
    }
}

/// A client started over SSH is far from the user's clipboard.
fn is_local() -> bool {
    std::env::var_os("SSH_CONNECTION").is_none() && std::env::var_os("SSH_TTY").is_none()
}
