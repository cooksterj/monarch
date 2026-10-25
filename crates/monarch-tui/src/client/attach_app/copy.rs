//! Copy mode in the client: opening and leaving it, its keys, and the
//! scroll, search and copy requests it makes of the server.

use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, GridPoint};

use super::AttachApp;
use super::select::NOTICE;
use crate::client::{CopyMode, CopyStep};
use crate::{OverlayKey, SelectionSpan};

/// What drawing needs from copy mode.
pub(super) struct CopyView {
    pub selection: Option<SelectionSpan>,
    pub matches: Vec<SelectionSpan>,
    pub cursor: Option<(u16, u16)>,
    pub status: String,
}

impl AttachApp {
    /// `prefix [`: starts copy mode on the focused pane.
    pub(super) fn open_copy(&mut self) {
        let Some(pane) = self.state.focus() else {
            return;
        };
        let Some(screen) = self.state.screen(pane) else {
            return;
        };
        self.copy = Some(CopyMode::new(pane, screen));
        self.selection = None;
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// One key in copy mode.
    pub(super) fn copy_key(&mut self, key: OverlayKey) {
        let Some(copy) = self.copy.as_mut() else {
            return;
        };
        let pane = copy.pane();
        let Some(screen) = self.state.screen(pane) else {
            return self.close_copy();
        };
        let step = copy.key(key, screen);
        if let Some(top) = copy.take_scroll() {
            let _ = self.link.send(&ClientMsg::ScrollTo { pane, top });
        }
        self.dirty = true;
        let msg = match step {
            CopyStep::Stay => return,
            CopyStep::Exit => return self.close_copy(),
            CopyStep::Copy { start, end, shape } => {
                self.close_copy();
                ClientMsg::Copy {
                    pane,
                    start,
                    end,
                    shape,
                }
            }
            CopyStep::Search {
                query,
                from,
                backward,
            } => ClientMsg::Search {
                pane,
                query,
                from,
                backward,
            },
        };
        let _ = self.link.send(&msg);
    }

    /// Leaves copy mode and returns the pane to live output.
    pub(super) fn close_copy(&mut self) {
        if let Some(copy) = self.copy.take() {
            let top = i64::MAX;
            let _ = self.link.send(&ClientMsg::ScrollTo {
                pane: copy.pane(),
                top,
            });
        }
        self.focus.set_overlay(self.any_overlay());
        self.dirty = true;
    }

    /// After frames: copy mode ends if its pane lost focus or closed, and
    /// search answers move its cursor.
    pub(super) fn after_copy_frames(
        &mut self,
        found: Vec<(PaneId, Option<(GridPoint, GridPoint)>)>,
    ) {
        let Some(pane) = self.copy.as_ref().map(CopyMode::pane) else {
            return;
        };
        if self.state.focus() != Some(pane) || self.state.screen(pane).is_none() {
            return self.close_copy();
        }
        for (_, hit) in found.into_iter().filter(|(p, _)| *p == pane) {
            let (Some(copy), Some(screen)) = (self.copy.as_mut(), self.state.screen(pane)) else {
                return;
            };
            if !copy.found(hit, screen) {
                let query = copy.query().unwrap_or_default();
                self.notice = Some((format!("not found: {query}"), Instant::now() + NOTICE));
            }
            if let Some(top) = copy.take_scroll() {
                let _ = self.link.send(&ClientMsg::ScrollTo { pane, top });
            }
            self.dirty = true;
        }
    }

    /// Copy mode as drawn, when it is on.
    pub(super) fn copy_view(&self) -> Option<CopyView> {
        let copy = self.copy.as_ref()?;
        let screen = self.state.screen(copy.pane())?;
        Some(CopyView {
            selection: copy.span(screen),
            matches: copy.matches(screen),
            cursor: copy.cursor_cell(screen),
            status: copy.status(screen),
        })
    }
}
