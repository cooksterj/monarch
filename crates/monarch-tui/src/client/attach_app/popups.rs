//! The pop-ups that can be open over the panes, kept together so drawing
//! can borrow them apart from the terminal.

use std::time::Instant;

use crate::{
    Chrome, ContextMenu, ContextMenuView, HelpState, HelpView, Picker, PickerView, PopupViews,
    ProjectPicker, ReviewState, ReviewView, SessionPicker, WelcomeState, WelcomeView, WorktreeForm,
    WorktreeFormView,
};

/// Every pop-up; each takes the keys while open.
#[derive(Debug, Default)]
pub(super) struct Popups {
    /// The palette, go-to picker or inbox.
    pub picker: Option<Picker>,
    /// The project picker.
    pub projects: Option<ProjectPicker>,
    /// The session picker.
    pub sessions: Option<SessionPicker>,
    /// The right-click menu, which also takes clicks.
    pub menu: Option<ContextMenu>,
    /// The review pane.
    pub review: Option<ReviewState>,
    /// The new-worktree form.
    pub form: Option<WorktreeForm>,
    /// The help page.
    pub help: Option<HelpState>,
    /// The welcome screen.
    pub welcome: Option<WelcomeState>,
}

impl Popups {
    /// Whether any is open.
    pub fn any(&self) -> bool {
        self.picker.is_some()
            || self.projects.is_some()
            || self.sessions.is_some()
            || self.menu.is_some()
            || self.review.is_some()
            || self.form.is_some()
            || self.help.is_some()
            || self.welcome.is_some()
    }

    /// The widgets for those open, coloured by `chrome`; the help names
    /// `prefix` and the welcome is drawn at `now`, still unless `animate`.
    pub fn views<'a>(
        &'a self,
        chrome: Chrome<'a>,
        prefix: &'a str,
        animate: bool,
    ) -> PopupViews<'a> {
        let theme = chrome.theme;
        let picker = (self.picker.as_ref())
            .or(self.projects.as_ref().map(ProjectPicker::picker))
            .or(self.sessions.as_ref().map(SessionPicker::picker));
        let now = Instant::now();
        PopupViews {
            picker: picker.map(|picker| PickerView { picker }),
            menu: self.menu.as_ref().map(|menu| ContextMenuView { menu }),
            review: self
                .review
                .as_ref()
                .map(|state| ReviewView { state, theme }),
            form: self
                .form
                .as_ref()
                .map(|form| WorktreeFormView { form, theme }),
            help: self.help.as_ref().map(|state| HelpView {
                state,
                prefix,
                chrome,
            }),
            welcome: self.welcome.as_ref().map(|state| WelcomeView {
                keys: state.keys(),
                tick: state.tick(now),
                animate,
                chrome,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_follow_what_is_open() {
        let mut popups = Popups::default();
        let views = |p: &Popups| p.views(Chrome::default(), "ctrl+b", false).picker.is_some();
        assert!(!popups.any() && !views(&popups));
        popups.projects = Some(ProjectPicker::default());
        assert!(popups.any());
        assert!(views(&popups), "the project picker draws as a picker");
        popups.help = Some(HelpState::new(&monarch_core::Keymap::default()));
        let drawn = popups.views(Chrome::default(), "ctrl+b", false);
        assert!(drawn.help.is_some() && drawn.menu.is_none() && drawn.welcome.is_none());
    }
}
