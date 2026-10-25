//! The pop-ups open in one frame, as widgets, and the order they stack in.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::{ContextMenuView, HelpView, PickerView, ReviewView, WelcomeView, WorktreeFormView};

/// Each pop-up's widget, when it is open.
#[derive(Debug, Clone, Copy, Default)]
pub struct PopupViews<'a> {
    /// The palette, a picker or the inbox; the cursor sits in its query.
    pub picker: Option<PickerView<'a>>,
    /// The right-click menu, at the pointer.
    pub menu: Option<ContextMenuView<'a>>,
    /// The review pane.
    pub review: Option<ReviewView<'a>>,
    /// The new-worktree form, with the cursor in it.
    pub form: Option<WorktreeFormView<'a>>,
    /// The help page.
    pub help: Option<HelpView<'a>>,
    /// The welcome screen, over everything else.
    pub welcome: Option<WelcomeView<'a>>,
}

impl PopupViews<'_> {
    /// Draws those open over `panes` (the menu anywhere on the host),
    /// bottom to top: review, help, form, picker, menu, welcome.
    pub fn draw(self, frame: &mut Frame<'_>, panes: Rect) {
        if let Some(review) = self.review {
            frame.render_widget(review, panes);
        }
        if let Some(help) = self.help {
            frame.render_widget(help, panes);
        }
        if let Some(form) = self.form {
            frame.render_widget(form, panes);
            frame.set_cursor_position(form.cursor(panes));
        }
        if let Some(picker) = self.picker {
            frame.render_widget(picker, panes);
            if let Some(cursor) = picker.cursor(panes) {
                frame.set_cursor_position(cursor);
            }
        }
        if let Some(menu) = self.menu {
            frame.render_widget(menu, frame.area());
        }
        if let Some(welcome) = self.welcome {
            frame.render_widget(welcome, panes);
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Keymap;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::{Chrome, HelpState};

    #[test]
    fn nothing_open_draws_nothing_and_help_draws_over_the_panes() {
        let mut term = Terminal::new(TestBackend::new(40, 10)).expect("terminal");
        term.draw(|f| PopupViews::default().draw(f, Rect::new(0, 0, 40, 10)))
            .expect("draw");
        assert!(
            term.backend()
                .buffer()
                .content
                .iter()
                .all(|c| c.symbol() == " ")
        );
        let state = HelpState::new(&Keymap::default());
        let help = HelpView {
            state: &state,
            prefix: "ctrl+b",
            chrome: Chrome::default(),
        };
        let views = PopupViews {
            help: Some(help),
            ..PopupViews::default()
        };
        term.draw(|f| views.draw(f, Rect::new(10, 0, 30, 10)))
            .expect("draw");
        assert_eq!(
            term.backend().buffer()[(10, 0)].symbol(),
            "┌",
            "inside the pane area"
        );
    }
}
