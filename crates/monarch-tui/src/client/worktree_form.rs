//! The new-worktree form: a branch name and what to start in it, or (from
//! the ask composer) the agents to start, each asked the prompt.

use monarch_core::AgentKind;
use monarch_proto::NewWorktree;

use super::WorktreeStep;
use crate::OverlayKey;

/// What the new tab can run, in the order the form offers it.
pub const WORKTREE_CHOICES: [Option<AgentKind>; 3] =
    [None, Some(AgentKind::Claude), Some(AgentKind::Codex)];

/// Longest branch name made from an ask, before any agent suffix.
const SUGGESTED: usize = 40;

/// The name typed so far, the chosen program, and how the last attempt
/// went. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeForm {
    dir: String,
    name: String,
    choice: usize,
    busy: bool,
    error: Option<String>,
    asked: Option<Asked>,
}

/// From the ask composer: the agents still to start, each in its own
/// worktree, and what each is asked first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    kinds: Vec<AgentKind>,
    prompt: String,
    /// More than one agent: each branch ends in its agent's name.
    several: bool,
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
            asked: None,
        }
    }

    /// The form for starting `kinds` in new worktrees of the repository
    /// holding `dir`, each asked `prompt`; the branch is named from
    /// `words` (`ask/why-is-it-slow`), ready to edit.
    #[must_use]
    pub fn for_ask(
        dir: impl Into<String>,
        kinds: Vec<AgentKind>,
        prompt: String,
        words: &str,
    ) -> Self {
        let several = kinds.len() > 1;
        Self {
            name: suggest(words),
            asked: Some(Asked {
                kinds,
                prompt,
                several,
            }),
            ..Self::new(dir)
        }
    }

    /// The agents it starts, when it came from the ask composer.
    #[must_use]
    pub fn asked_agents(&self) -> Option<&[AgentKind]> {
        self.asked.as_ref().map(|a| a.kinds.as_slice())
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
            OverlayKey::Tab | OverlayKey::Right if self.asked.is_none() => {
                self.choice = (self.choice + 1) % count;
            }
            OverlayKey::BackTab | OverlayKey::Left if self.asked.is_none() => {
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

    /// Every request to send: the one worktree, or one per agent still to
    /// start from an ask, each asked its prompt.
    #[must_use]
    pub fn requests(&self, root: Option<String>) -> Vec<NewWorktree> {
        let Some(asked) = &self.asked else {
            return vec![self.request(root)];
        };
        asked
            .kinds
            .iter()
            .map(|kind| NewWorktree {
                name: self.branch_for(*kind),
                agent: Some(*kind),
                prompt: Some(asked.prompt.clone()),
                ..self.request(root.clone())
            })
            .collect()
    }

    /// The server made the worktree for `branch`: its agent is done.
    /// Whether every one asked for is made, so the form can close.
    pub fn made(&mut self, branch: &str) -> bool {
        let Some(kinds) = self.asked.as_ref().map(|a| a.kinds.clone()) else {
            return true;
        };
        let left: Vec<AgentKind> = kinds
            .into_iter()
            .filter(|k| self.branch_for(*k) != branch)
            .collect();
        let done = left.is_empty();
        if let Some(asked) = self.asked.as_mut() {
            asked.kinds = left;
        }
        done
    }

    /// The branch `kind` gets: the name, ending in the agent's name when
    /// the ask starts several (`ask/why-claude`, `ask/why-codex`).
    fn branch_for(&self, kind: AgentKind) -> String {
        match &self.asked {
            Some(asked) if asked.several => format!("{}-{}", self.name, kind.name()),
            _ => self.name.clone(),
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

/// `ask/` and the first words of `words`, lower case, joined by dashes:
/// `ask/why-is-it-slow`; `ask/agent` when there are none.
fn suggest(words: &str) -> String {
    let lower = words.to_lowercase();
    let parts = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty());
    let mut slug = String::new();
    for part in parts {
        if slug.len() + part.len() + 1 > SUGGESTED {
            break;
        }
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(part);
    }
    if slug.is_empty() {
        slug.push_str("agent");
    }
    format!("ask/{slug}")
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

    #[test]
    fn an_ask_names_the_branch_from_its_words_and_starts_its_agent_asked() {
        let mut form = WorktreeForm::for_ask(
            "/src/app",
            vec![AgentKind::Codex],
            "why?\n```x```".into(),
            "Why is it SLOW?",
        );
        assert_eq!(form.name(), "ask/why-is-it-slow");
        assert_eq!(form.asked_agents(), Some(&[AgentKind::Codex][..]));
        form.key(OverlayKey::Tab);
        assert_eq!(form.choice(), 0, "the agent is fixed");
        typed(&mut form, "-x");
        assert_eq!(form.key(OverlayKey::Enter), WorktreeStep::Submit);
        let sent = form.requests(None);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].name, "ask/why-is-it-slow-x");
        assert_eq!(sent[0].agent, Some(AgentKind::Codex));
        assert_eq!(sent[0].prompt.as_deref(), Some("why?\n```x```"));
        assert!(form.made("ask/why-is-it-slow-x"));
    }

    #[test]
    fn several_agents_get_a_worktree_each_and_a_retry_makes_only_the_rest() {
        let mut form = WorktreeForm::for_ask("/r", AgentKind::ALL.to_vec(), "go".into(), "go");
        let names: Vec<String> = form.requests(None).into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["ask/go-claude", "ask/go-codex"]);
        assert!(!form.made("ask/go-codex"), "claude still to come");
        assert!(!form.made("ask/other"), "not one of ours");
        let left: Vec<String> = form.requests(None).into_iter().map(|r| r.name).collect();
        assert_eq!(left, ["ask/go-claude"]);
        assert!(form.made("ask/go-claude"));
        let plain = WorktreeForm::new("/r");
        assert_eq!(plain.requests(None)[0].prompt, None);
        assert_eq!(plain.asked_agents(), None);
    }

    #[test]
    fn suggested_branches_are_short_and_never_empty() {
        assert_eq!(suggest("?!"), "ask/agent");
        let long = suggest(&"word ".repeat(30));
        assert!(long.len() <= "ask/".len() + SUGGESTED, "{long}");
        assert!(!long.ends_with('-'));
        assert_eq!(suggest("fix the café's API"), "ask/fix-the-caf-s-api");
    }
}
