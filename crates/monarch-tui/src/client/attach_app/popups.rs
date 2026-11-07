//! The pop-ups that can be open over the panes, kept together so drawing
//! can borrow them apart from the terminal.

use std::time::Instant;

use monarch_core::UnixMillis;

use crate::{
    AgentInfo, AgentInfoView, Chrome, ContextMenu, ContextMenuView, GoToPicker, HelpState,
    HelpView, NamePrompt, NotePrompt, Picker, PickerView, PopupViews, ProjectPicker, ReviewState,
    ReviewView, SessionPicker, WelcomeState, WelcomeView, WorktreeForm, WorktreeFormView,
};

/// Every pop-up; each takes the keys while open.
#[derive(Debug, Default)]
pub(super) struct Popups {
    /// The palette or the inbox.
    pub picker: Option<Picker>,
    /// The go-to picker.
    pub goto: Option<GoToPicker>,
    /// The agent info.
    pub info: Option<AgentInfo>,
    /// The project picker.
    pub projects: Option<ProjectPicker>,
    /// The session picker.
    pub sessions: Option<SessionPicker>,
    /// The rename-tab or rename-pane prompt.
    pub namer: Option<NamePrompt>,
    /// The right-click menu, which also takes clicks.
    pub menu: Option<ContextMenu>,
    /// The review pane.
    pub review: Option<ReviewState>,
    /// A note being written in the review, for its pane's agent.
    pub note: Option<NotePrompt>,
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
            || self.goto.is_some()
            || self.info.is_some()
            || self.projects.is_some()
            || self.sessions.is_some()
            || self.namer.is_some()
            || self.menu.is_some()
            || self.review.is_some()
            || self.note.is_some()
            || self.form.is_some()
            || self.help.is_some()
            || self.welcome.is_some()
    }

    /// The widgets for those open, coloured by `chrome`; the help names
    /// `prefix`, the agent info ages states against `now`, and the
    /// welcome is drawn still unless `animate`.
    pub fn views<'a>(
        &'a self,
        chrome: Chrome<'a>,
        prefix: &'a str,
        animate: bool,
        now: UnixMillis,
    ) -> PopupViews<'a> {
        let theme = chrome.theme;
        let picker = (self.picker.as_ref())
            .or(self.goto.as_ref().map(GoToPicker::picker))
            .or(self.projects.as_ref().map(ProjectPicker::picker))
            .or(self.sessions.as_ref().map(SessionPicker::picker))
            .or(self.namer.as_ref().map(NamePrompt::picker))
            .or(self.note.as_ref().map(NotePrompt::picker));
        let instant = Instant::now();
        PopupViews {
            picker: picker.map(|picker| PickerView { picker }),
            info: self
                .info
                .as_ref()
                .map(|info| AgentInfoView { info, now, chrome }),
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
                tick: state.tick(instant),
                seed: state.seed(),
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
        let now = UnixMillis(0);
        let views = |p: &Popups| {
            p.views(Chrome::default(), "ctrl+b", false, now)
                .picker
                .is_some()
        };
        assert!(!popups.any() && !views(&popups));
        popups.projects = Some(ProjectPicker::default());
        assert!(popups.any());
        assert!(views(&popups), "the project picker draws as a picker");
        popups.help = Some(HelpState::new(&monarch_core::Keymap::default()));
        let drawn = popups.views(Chrome::default(), "ctrl+b", false, now);
        assert!(drawn.help.is_some() && drawn.menu.is_none() && drawn.welcome.is_none());
        popups.projects = None;
        popups.goto = Some(GoToPicker::new(Vec::new()));
        popups.info = Some(AgentInfo::new(monarch_core::PaneId(1), "zsh"));
        let drawn = popups.views(Chrome::default(), "ctrl+b", false, now);
        assert!(drawn.picker.is_some(), "the go-to picker draws as a picker");
        assert!(drawn.info.is_some());
    }
}
