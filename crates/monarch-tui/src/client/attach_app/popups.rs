//! The pop-ups that can be open over the panes, kept together so drawing
//! can borrow them apart from the terminal.

use monarch_core::Theme;

use crate::{
    Chrome, ContextMenu, ContextMenuView, HelpState, HelpView, Picker, PickerView, ProjectPicker,
    ReviewState, ReviewView, SessionPicker, WorktreeForm, WorktreeFormView,
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
}

/// What [`Popups::views`] draws: a picker, the menu, the review and the
/// form.
type Views<'a> = (
    Option<PickerView<'a>>,
    Option<ContextMenuView<'a>>,
    Option<ReviewView<'a>>,
    Option<WorktreeFormView<'a>>,
);

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
    }

    /// The help as drawn, when open; `prefix` is named in its title.
    pub fn help_view<'a>(&'a self, prefix: &'a str, chrome: Chrome<'a>) -> Option<HelpView<'a>> {
        self.help.as_ref().map(|state| HelpView {
            state,
            prefix,
            chrome,
        })
    }

    /// The widgets for those open, coloured by `theme`.
    pub fn views<'a>(&'a self, theme: Option<&'a Theme>) -> Views<'a> {
        let picker = (self.picker.as_ref())
            .or(self.projects.as_ref().map(ProjectPicker::picker))
            .or(self.sessions.as_ref().map(SessionPicker::picker));
        (
            picker.map(|picker| PickerView { picker }),
            self.menu.as_ref().map(|menu| ContextMenuView { menu }),
            self.review
                .as_ref()
                .map(|state| ReviewView { state, theme }),
            self.form
                .as_ref()
                .map(|form| WorktreeFormView { form, theme }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_follow_what_is_open() {
        let mut popups = Popups::default();
        assert!(!popups.any());
        assert!(popups.views(None).0.is_none());
        popups.projects = Some(ProjectPicker::default());
        assert!(popups.any());
        let (picker, menu, review, form) = popups.views(None);
        assert!(picker.is_some(), "the project picker draws as a picker");
        assert!(menu.is_none() && review.is_none() && form.is_none());
        assert!(popups.help_view("ctrl+b", Chrome::default()).is_none());
        popups.help = Some(HelpState::new(&monarch_core::Keymap::default()));
        assert!(popups.help_view("ctrl+b", Chrome::default()).is_some());
    }
}
