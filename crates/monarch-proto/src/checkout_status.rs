//! How a git checkout differs from its last commit and its upstream.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::BadgePart;

/// Uncommitted lines and files, and commits not yet pushed or pulled;
/// shown as a badge beside the checkout (`+12 −4 ?2 ↑2 ↓1`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckoutStatus {
    /// Lines added since the last commit, staged or not.
    pub added: u32,
    /// Lines removed since the last commit, staged or not.
    pub removed: u32,
    /// Files git doesn't track yet (their lines aren't in `added`).
    pub untracked: u32,
    /// Commits the branch has that its upstream lacks (0 without one).
    pub ahead: u32,
    /// Commits its upstream has that the branch lacks (0 without one).
    pub behind: u32,
}

impl CheckoutStatus {
    /// Whether there is nothing to show: clean and level with upstream.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        *self == Self::default()
    }

    /// The badge's parts that aren't zero, in order, with their text.
    #[must_use]
    pub fn parts(&self) -> Vec<(BadgePart, String)> {
        [
            (BadgePart::Added, "+", self.added),
            (BadgePart::Removed, "−", self.removed),
            (BadgePart::Untracked, "?", self.untracked),
            (BadgePart::Ahead, "↑", self.ahead),
            (BadgePart::Behind, "↓", self.behind),
        ]
        .into_iter()
        .filter(|(_, _, n)| *n > 0)
        .map(|(part, mark, n)| (part, format!("{mark}{n}")))
        .collect()
    }
}

/// `+added −removed ?untracked ↑ahead ↓behind`, leaving out the zeros;
/// empty when quiet.
impl fmt::Display for CheckoutStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.parts().into_iter().map(|(_, text)| text).collect();
        f.write_str(&parts.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(added: u32, removed: u32, untracked: u32, ahead: u32, behind: u32) -> CheckoutStatus {
        CheckoutStatus {
            added,
            removed,
            untracked,
            ahead,
            behind,
        }
    }

    #[test]
    fn badge_leaves_out_zeros() {
        assert_eq!(status(12, 4, 2, 2, 1).to_string(), "+12 −4 ?2 ↑2 ↓1");
        assert_eq!(status(0, 0, 0, 0, 4).to_string(), "↓4");
        assert_eq!(status(0, 7, 0, 0, 0).to_string(), "−7");
        assert_eq!(status(0, 0, 0, 0, 0).to_string(), "", "quiet");
        assert!(status(0, 0, 0, 0, 0).is_quiet() && !status(0, 0, 1, 0, 0).is_quiet());
    }

    #[test]
    fn parts_name_what_each_number_is() {
        let parts = status(3, 0, 1, 0, 2).parts();
        let kinds: Vec<_> = parts.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            kinds,
            [BadgePart::Added, BadgePart::Untracked, BadgePart::Behind]
        );
        assert_eq!(parts[2].1, "↓2");
    }

    #[test]
    fn survives_the_attach_stream_encoding() {
        let status = status(5, 1, 0, 0, 9);
        let bytes = postcard::to_allocvec(&status).expect("encode");
        assert_eq!(postcard::from_bytes(&bytes).ok(), Some(status));
    }
}
