//! `worktree.create`: make a git worktree and open a pane in it.

use monarch_core::AgentKind;
use serde::{Deserialize, Serialize};

/// Asks the server to add a worktree for branch `name` to the repository
/// holding `dir` and open a tab in it, running `agent` if given.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewWorktree {
    /// Any directory inside the repository; a new branch starts from its
    /// checked-out commit.
    pub dir: String,
    /// The branch, checked out if it exists, else created; the worktree's
    /// directory is named after it.
    pub name: String,
    /// What a new branch starts from instead of `dir`'s `HEAD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The agent to start in the new tab; a shell when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentKind>,
    /// `[projects].worktree_root` from the caller's config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

impl NewWorktree {
    /// The API method name.
    pub const METHOD: &'static str = "worktree.create";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_fields_may_be_left_out() {
        let parsed: NewWorktree =
            serde_json::from_str(r#"{"dir":"/r","name":"feat"}"#).expect("parses");
        assert_eq!(parsed.name, "feat");
        assert_eq!((parsed.base, parsed.agent, parsed.root), (None, None, None));
        assert!(serde_json::from_str::<NewWorktree>(r#"{"dir":"/r"}"#).is_err());
    }
}
