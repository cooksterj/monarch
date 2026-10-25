//! The new-worktree form: a branch name and what to start in it.

use monarch_core::AgentKind;
use monarch_proto::NewWorktree;

use super::WorktreeStep;
use crate::OverlayKey;

/// What the new tab can run, in the order the form offers it.
pub const WORKTREE_CHOICES: [Option<AgentKind>; 3] =
    [None, Some(AgentKind::Claude), Some(AgentKind::Codex)];

/// The name typed so far, the chosen program, and how the last attempt
/// went. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeForm {
    dir: String,
    name: String,
    choice: usize,
    busy: bool,
    error: Option<String>,
}

impl WorktreeForm {
    /// An empty form for the repository holding `dir`.
    #[must_use]
    pub fn new(dir: impl Into<String>) -> Self {
        Self {
            dir: dir.into(),
            name: String::new(),
            choice: 0,
            busy: false,
            error: None,
        }
    }

    /// Applies one key: type the branch, Backspace/Ctrl-w/Ctrl-u edit,
    /// Tab/Shift-Tab/←/→ pick what to start, Enter makes it, Esc closes.
    /// Only Esc works while the server is busy.
    pub fn key(&mut self, key: OverlayKey) -> WorktreeStep {
        if matches!(key, OverlayKey::Esc | OverlayKey::Ctrl('c' | 'g')) {
            return WorktreeStep::Close;
        }
        if self.busy {
            return WorktreeStep::Open;
        }
        let count = WORKTREE_CHOICES.len();
        match key {
            OverlayKey::Char(c) if !c.is_whitespace() => self.edit(|n| n.push(c)),
            OverlayKey::Backspace => self.edit(|n| {
                n.pop();
            }),
            OverlayKey::Ctrl('w') => self.edit(|n| {
                let kept = n.trim_end_matches(|c| c != '/' && c != '-').len();
                n.truncate(kept.min(n.len().saturating_sub(1)));
            }),
            OverlayKey::Ctrl('u') => self.edit(String::clear),
            OverlayKey::Tab | OverlayKey::Right => self.choice = (self.choice + 1) % count,
            OverlayKey::BackTab | OverlayKey::Left => {
                self.choice = (self.choice + count - 1) % count;
            }
            OverlayKey::Enter if !self.name.is_empty() => {
                self.busy = true;
                self.error = None;
                return WorktreeStep::Submit;
            }
            _ => {}
        }
        WorktreeStep::Open
    }

    /// The server could not make it: say why and let the user edit.
    pub fn failed(&mut self, why: impl Into<String>) {
        self.busy = false;
        self.error = Some(why.into());
    }

    /// The request for the server; `root` is `[projects].worktree_root`.
    #[must_use]
    pub fn request(&self, root: Option<String>) -> NewWorktree {
        NewWorktree {
            dir: self.dir.clone(),
            name: self.name.clone(),
            agent: WORKTREE_CHOICES[self.choice],
            root,
            ..NewWorktree::default()
        }
    }

    /// The branch typed so far.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Index of the chosen program in [`WORKTREE_CHOICES`].
    #[must_use]
    pub fn choice(&self) -> usize {
        self.choice
    }

    /// Whether the server is making it.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Why the last attempt failed.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn edit(&mut self, change: impl FnOnce(&mut String)) {
        change(&mut self.name);
        self.error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(form: &mut WorktreeForm, text: &str) {
        for c in text.chars() {
            form.key(OverlayKey::Char(c));
        }
    }

    #[test]
    fn type_a_name_pick_an_agent_and_submit() {
        let mut form = WorktreeForm::new("/src/app");
        assert_eq!(form.key(OverlayKey::Enter), WorktreeStep::Open, "no name");
        typed(&mut form, "feat/x y");
        assert_eq!(form.name(), "feat/xy", "no spaces in branch names");
        form.key(OverlayKey::Tab);
        assert_eq!(form.choice(), 1);
        form.key(OverlayKey::Left);
        form.key(OverlayKey::Left);
        assert_eq!(form.choice(), 2, "wraps");
        assert_eq!(form.key(OverlayKey::Enter), WorktreeStep::Submit);
        assert!(form.busy());
        let request = form.request(Some("/wt".into()));
        assert_eq!(
            (request.dir.as_str(), request.name.as_str(), request.agent),
            ("/src/app", "feat/xy", Some(AgentKind::Codex))
        );
        assert_eq!(request.root.as_deref(), Some("/wt"));
    }

    #[test]
    fn failures_reopen_editing_and_esc_always_closes() {
        let mut form = WorktreeForm::new("/r");
        typed(&mut form, "feat/long-name");
        form.key(OverlayKey::Ctrl('w'));
        assert_eq!(form.name(), "feat/long-");
        form.key(OverlayKey::Enter);
        typed(&mut form, "zzz");
        assert_eq!(form.name(), "feat/long-", "ignored while busy");
        assert_eq!(form.key(OverlayKey::Esc), WorktreeStep::Close);
        form.failed("branch exists");
        assert_eq!(form.error(), Some("branch exists"));
        form.key(OverlayKey::Backspace);
        assert_eq!((form.name(), form.error()), ("feat/long", None));
        form.key(OverlayKey::Ctrl('u'));
        assert_eq!(form.name(), "");
    }
}
