//! How a git checkout differs from its last commit and its upstream.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Uncommitted files and commits not yet pushed or pulled; shown as a
/// badge beside the checkout (`±3 ↑2 ↓1`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckoutStatus {
    /// Files with uncommitted changes, untracked ones included.
    pub changed: u32,
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
}

/// `±changed ↑ahead ↓behind`, leaving out the zeros; empty when quiet.
impl fmt::Display for CheckoutStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = [("±", self.changed), ("↑", self.ahead), ("↓", self.behind)];
        let mut first = true;
        for (mark, n) in parts.into_iter().filter(|(_, n)| *n > 0) {
            if !first {
                f.write_str(" ")?;
            }
            first = false;
            write!(f, "{mark}{n}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_leaves_out_zeros() {
        let status = |changed, ahead, behind| CheckoutStatus {
            changed,
            ahead,
            behind,
        };
        assert_eq!(status(3, 2, 1).to_string(), "±3 ↑2 ↓1");
        assert_eq!(status(0, 0, 4).to_string(), "↓4");
        assert_eq!(status(12, 0, 0).to_string(), "±12");
        assert_eq!(status(0, 0, 0).to_string(), "", "quiet");
        assert!(status(0, 0, 0).is_quiet() && !status(0, 1, 0).is_quiet());
    }

    #[test]
    fn survives_the_attach_stream_encoding() {
        let status = CheckoutStatus {
            changed: 5,
            ahead: 0,
            behind: 9,
        };
        let bytes = postcard::to_allocvec(&status).expect("encode");
        assert_eq!(postcard::from_bytes(&bytes).ok(), Some(status));
    }
}
