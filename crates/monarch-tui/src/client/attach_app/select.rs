//! Mouse selection, copy and the mouse passthrough toggle.

use std::thread;

use monarch_core::MouseMode;
use std::time::{Duration, Instant};

use super::AttachApp;
use crate::client::{SelectEvent, SelectUnit, Selection};
use crate::{HostClipboard, LocalClipboard, SelectionSpan, SystemClipboard};

/// How long a status-line notice stays up.
const NOTICE: Duration = Duration::from_secs(2);

impl AttachApp {
    /// Applies a selection gesture to the focused pane.
    pub(super) fn on_select(&mut self, gesture: SelectEvent) {
        let Some(screen) = self.state.focus().and_then(|p| self.state.screen(p)) else {
            return;
        };
        match gesture {
            SelectEvent::Start { col, row, clicks } => {
                self.selection = Some(Selection::start(
                    screen,
                    row,
                    col,
                    SelectUnit::from_clicks(clicks),
                ));
            }
            SelectEvent::Extend { col, row } => {
                if let Some(selection) = &mut self.selection {
                    selection.extend(screen, row, col);
                }
            }
            SelectEvent::End => {
                let text = self
                    .selection
                    .filter(|s| !s.is_empty())
                    .map(|s| s.text(screen));
                match text.filter(|t| !t.is_empty()) {
                    Some(text) => self.copy(text),
                    None => self.selection = None,
                }
            }
        }
        self.dirty = true;
    }

    /// `prefix m`: hand the mouse to the host terminal (native selection,
    /// its own menus) or take it back.
    pub(super) fn toggle_mouse(&mut self) {
        self.passthrough = !self.passthrough;
        let (policy, notice) = if self.passthrough {
            (
                MouseMode::Off,
                format!("mouse: terminal ({} m to restore)", self.opts.prefix_label),
            )
        } else {
            (self.opts.mouse, "mouse: monarch".to_owned())
        };
        self.mirror.set_mouse(policy);
        self.selection = None;
        self.notice = Some((notice, Instant::now()));
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
        self.notice = Some((format!("copied {count} {unit}"), Instant::now()));
    }

    /// Drops an expired notice; returns whether the screen changed.
    pub(super) fn expire_notice(&mut self) {
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= NOTICE)
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
