//! `worktree.create`: make a git worktree and open a pane in it.

use monarch_core::AgentKind;
use serde::{Deserialize, Serialize};

/// Asks the server to add a worktree for branch `name` to the repository
/// holding `dir` and open a tab in it, running `agent` if given.
///
/// It also travels in the attach stream, whose postcard encoding has no
/// field names: every field is always written (JSON callers may still
/// leave the optional ones out).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewWorktree {
    /// Any directory inside the repository; a new branch starts from its
    /// checked-out commit.
    pub dir: String,
    /// The branch, checked out if it exists, else created; the worktree's
    /// directory is named after it.
    pub name: String,
    /// What a new branch starts from instead of `dir`'s `HEAD`.
    #[serde(default)]
    pub base: Option<String>,
    /// The agent to start in the new tab; a shell when `None`.
    #[serde(default)]
    pub agent: Option<AgentKind>,
    /// `[projects].worktree_root` from the caller's config.
    #[serde(default)]
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

    #[test]
    fn survives_the_attach_stream_encoding() {
        let request = NewWorktree {
            dir: "/r".into(),
            name: "feat".into(),
            agent: Some(AgentKind::Codex),
            ..NewWorktree::default()
        };
        let bytes = postcard::to_stdvec(&request).expect("encode");
        assert_eq!(
            postcard::from_bytes::<NewWorktree>(&bytes).expect("decode"),
            request
        );
    }
}
