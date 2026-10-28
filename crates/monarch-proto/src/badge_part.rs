//! The parts of a checkout's badge, so each can be drawn its own way.

/// One part of a [`crate::CheckoutStatus`] badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgePart {
    /// `+12`: lines added.
    Added,
    /// `−4`: lines removed.
    Removed,
    /// `?2`: untracked files.
    Untracked,
    /// `↑2`: commits to push.
    Ahead,
    /// `↓1`: commits to pull.
    Behind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_are_distinct() {
        assert_ne!(BadgePart::Ahead, BadgePart::Behind);
        assert_eq!(BadgePart::Added, BadgePart::Added);
    }
}
