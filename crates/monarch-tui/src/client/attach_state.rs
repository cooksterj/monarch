//! The client's model of the session, built only from server messages.

use std::collections::{BTreeMap, BTreeSet};

use monarch_core::{PaneId, Rect};
use monarch_proto::{LayoutMsg, ModeFlags, PaneSummary, Screen, ServerMsg, StyleCache};

use super::{Applied, FocusView, PaneReplica};

/// Longest pane title shown in a tab label, in characters.
const TAB_TITLE: usize = 20;

/// Layout, decoded pane screens, titles and the connection's style cache.
/// Pure data: applying a message never does I/O, it reports what to do in
/// an [`Applied`].
#[derive(Debug, Clone, Default)]
pub struct AttachState {
    styles: StyleCache,
    layout: Option<LayoutMsg>,
    panes: BTreeMap<PaneId, PaneReplica>,
    titles: BTreeMap<PaneId, String>,
    commands: BTreeMap<PaneId, String>,
    resyncing: BTreeSet<PaneId>,
}

impl AttachState {
    /// An empty model (nothing received yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one server message.
    pub fn apply(&mut self, msg: ServerMsg, out: &mut Applied) {
        match msg {
            ServerMsg::Layout(layout) => {
                self.panes
                    .retain(|id, _| layout.panes.iter().any(|p| p.pane == *id));
                self.layout = Some(layout);
                out.redraw = true;
            }
            ServerMsg::Snapshot(snapshot) => {
                self.resyncing.remove(&snapshot.pane);
                let replica = PaneReplica::from_snapshot(&snapshot, &self.styles);
                self.panes.insert(snapshot.pane, replica);
                out.redraw = true;
            }
            ServerMsg::Delta(delta) => match self.panes.get_mut(&delta.pane) {
                Some(replica) if replica.accepts(&delta) => {
                    replica.apply(&delta, &self.styles);
                    out.redraw = true;
                }
                _ if self.resyncing.insert(delta.pane) => out.resync.push(delta.pane),
                _ => {}
            },
            ServerMsg::DefineStyle(def) => self.styles.define(def),
            ServerMsg::ResetStyles => self.styles.clear(),
            ServerMsg::Title { pane, title } => {
                self.titles.insert(pane, title.unwrap_or_default());
                out.redraw = true;
            }
            ServerMsg::Command { pane, command } => {
                self.commands.insert(pane, command.unwrap_or_default());
                out.redraw = true;
            }
            ServerMsg::HostWrite(bytes) => out.host_write.extend(bytes),
            ServerMsg::Clipboard(text) => out.clipboard.push(text),
            ServerMsg::PaneExited { pane } => {
                self.panes.remove(&pane);
                out.exited.push(pane);
                out.redraw = true;
            }
            _ => {}
        }
    }

    /// The current layout, once received.
    #[must_use]
    pub fn layout(&self) -> Option<&LayoutMsg> {
        self.layout.as_ref()
    }

    /// The focused pane.
    #[must_use]
    pub fn focus(&self) -> Option<PaneId> {
        self.layout.as_ref()?.focus
    }

    /// A pane's decoded screen.
    #[must_use]
    pub fn screen(&self, pane: PaneId) -> Option<&Screen> {
        self.panes.get(&pane).map(|r| &r.screen)
    }

    /// The focused pane's title, preferring the latest `Title` message.
    #[must_use]
    pub fn focused_title(&self) -> Option<&str> {
        let focus = self.focus()?;
        if let Some(title) = self.titles.get(&focus) {
            return Some(title.as_str());
        }
        self.layout.as_ref()?.focused()?.title.as_deref()
    }

    /// One label per tab (`1 vim`, `2 zsh [Z]`) and the shown tab's index,
    /// named after each tab's focused pane (see [`Self::all_panes`]).
    #[must_use]
    pub fn tab_labels(&self) -> (Vec<String>, usize) {
        let Some(layout) = self.layout.as_ref() else {
            return (Vec::new(), 0);
        };
        let labels = layout
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let name = tab.focused().map_or("", |p| self.name(p));
                let mut label = format!("{} {}", i + 1, shorten(name.trim()));
                if tab.zoomed {
                    label.push_str(" [Z]");
                }
                label.trim_end().to_owned()
            })
            .collect();
        (labels, layout.active_tab)
    }

    /// Every pane in every tab, in tab then reading order, as
    /// `(tab index, pane, name)`. A pane is named by its title, or by its
    /// foreground program when the title is blank, following the latest
    /// `Title` and `Command` messages.
    #[must_use]
    pub fn all_panes(&self) -> Vec<(usize, PaneId, &str)> {
        let Some(layout) = self.layout.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, tab) in layout.tabs.iter().enumerate() {
            for pane in &tab.panes {
                out.push((i, pane.pane, self.name(pane)));
            }
        }
        out
    }

    /// A pane's name from what the layout said and later messages.
    fn name<'a>(&'a self, sent: &'a PaneSummary) -> &'a str {
        let latest = |map: &'a BTreeMap<PaneId, String>| map.get(&sent.pane).map(String::as_str);
        let title = latest(&self.titles).or(sent.title.as_deref());
        let command = latest(&self.commands).or(sent.command.as_deref());
        PaneSummary::pick(title, command).unwrap_or_default()
    }

    /// Every visible pane and its place on the host.
    #[must_use]
    pub fn pane_rects(&self) -> Vec<(PaneId, Rect)> {
        self.layout.as_ref().map_or_else(Vec::new, |layout| {
            layout
                .panes
                .iter()
                .map(|p| (p.pane, Rect::new(p.x, p.y, p.size.cols, p.size.rows)))
                .collect()
        })
    }

    /// The focused pane as the input thread needs it.
    #[must_use]
    pub fn focus_view(&self) -> FocusView {
        let Some(pane) = self.layout.as_ref().and_then(LayoutMsg::focused) else {
            return FocusView::default();
        };
        let screen = self.screen(pane.pane);
        FocusView {
            pane: Some(pane.pane),
            x: pane.x,
            y: pane.y,
            cols: pane.size.cols,
            rows: pane.size.rows,
            modes: screen.map_or(ModeFlags::empty(), |s| s.modes.flags),
            scrolled: screen.is_some_and(|s| s.display_offset > 0),
        }
    }
}

/// `title`, cut to [`TAB_TITLE`] characters with an ellipsis.
fn shorten(title: &str) -> String {
    if title.chars().count() <= TAB_TITLE {
        return title.to_owned();
    }
    let mut short: String = title.chars().take(TAB_TITLE - 1).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests;
