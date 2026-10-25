//! The sidebar in the client: whether it fits, toggling it, and telling
//! the server and the input thread where panes and sidebar rows are.

use std::time::Instant;

use monarch_core::Rect;
use monarch_proto::{ClientMsg, ScreenSize};

use super::AttachApp;
use super::select::NOTICE;
use crate::client::{AttachOptions, AttachView};
use crate::{Chrome, Sidebar};

impl AttachApp {
    /// `prefix t`: shows or hides the sidebar.
    pub(super) fn toggle_sidebar(&mut self) {
        self.sidebar = !self.sidebar;
        if self.sidebar && sidebar_cols(&self.opts, true, self.size) == 0 {
            let text = format!("sidebar needs {} columns", self.opts.sidebar_min_columns);
            self.notice = Some((text, Instant::now() + NOTICE));
        }
        self.relayout();
    }

    /// Gives the panes what the sidebar leaves of the host and tells the
    /// server (which ignores a size it already has).
    pub(super) fn relayout(&mut self) {
        let left = sidebar_cols(&self.opts, self.sidebar, self.size);
        self.state.set_left(left);
        let _ = self
            .link
            .send(&ClientMsg::Resize(AttachView::pane_area(self.size, left)));
        self.publish_panes();
        self.dirty = true;
    }

    /// Tells the input thread where the focused pane, every pane and every
    /// sidebar row are, so clicks focus the right pane.
    pub(super) fn publish_panes(&self) {
        let mut rects = self.state.pane_rects();
        let left = self.state.left();
        if left > 0 {
            let groups = self.state.sidebar_groups();
            let host = ratatui::layout::Rect::new(0, 0, self.size.cols, self.size.rows);
            let area = AttachView::sidebar_area(host, left);
            let rows = Sidebar {
                groups: &groups,
                chrome: Chrome::default(),
            }
            .targets(area);
            rects.extend(
                rows.into_iter()
                    .map(|(pane, r)| (pane, Rect::new(r.x, r.y, r.width, r.height))),
            );
        }
        self.focus.set(&self.state.focus_view());
        self.focus.set_panes(rects);
    }
}

/// Columns the sidebar takes on a host of `size`: none when it is off or
/// the host is narrower than `[ui].sidebar_min_columns`.
pub(super) fn sidebar_cols(opts: &AttachOptions, on: bool, size: ScreenSize) -> u16 {
    let fits = size.cols >= opts.sidebar_min_columns && size.cols > opts.sidebar_width;
    if on && fits { opts.sidebar_width } else { 0 }
}

#[cfg(test)]
mod tests {
    use monarch_core::{Config, LoadedConfig};

    use super::*;

    fn opts() -> AttachOptions {
        let loaded = LoadedConfig {
            config: Config::default(),
            path: "/c.toml".into(),
            found: false,
            unknown_keys: Vec::new(),
        };
        AttachOptions::new("/tmp/m.sock".into(), "main".into(), &loaded).expect("valid")
    }

    #[test]
    fn the_sidebar_shows_only_when_on_and_wide_enough() {
        let opts = opts();
        assert_eq!(sidebar_cols(&opts, true, ScreenSize::new(120, 30)), 32);
        assert_eq!(sidebar_cols(&opts, true, ScreenSize::new(100, 30)), 32);
        assert_eq!(
            sidebar_cols(&opts, true, ScreenSize::new(99, 30)),
            0,
            "too narrow"
        );
        assert_eq!(
            sidebar_cols(&opts, false, ScreenSize::new(200, 30)),
            0,
            "off"
        );
        let odd = AttachOptions {
            sidebar_width: 300,
            ..opts
        };
        assert_eq!(
            sidebar_cols(&odd, true, ScreenSize::new(200, 30)),
            0,
            "wider than the host"
        );
    }
}
