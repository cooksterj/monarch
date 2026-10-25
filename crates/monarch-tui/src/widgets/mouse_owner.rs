//! Who the mouse belongs to, as the status line shows it.

/// Who receives clicks, drags and the wheel right now: Monarch (which
/// routes them to panes, selects and opens menus) or the host terminal
/// (its native selection and menus, after `prefix m` or `mouse = off`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseOwner {
    /// Monarch captures the mouse.
    #[default]
    Monarch,
    /// The host terminal keeps the mouse.
    Terminal,
}

impl MouseOwner {
    /// The owner when Monarch does or doesn't capture the mouse.
    #[must_use]
    pub fn new(captured: bool) -> Self {
        if captured {
            Self::Monarch
        } else {
            Self::Terminal
        }
    }

    /// The status-line badge, e.g. ` MOUSE: MONARCH │`.
    #[must_use]
    pub fn badge(self) -> &'static str {
        match self {
            Self::Monarch => " MOUSE: MONARCH │",
            Self::Terminal => " MOUSE: TERMINAL │",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monarch_owns_the_mouse_by_default() {
        assert_eq!(MouseOwner::default(), MouseOwner::Monarch);
        assert_eq!(MouseOwner::new(true), MouseOwner::Monarch);
    }

    #[test]
    fn without_capture_the_terminal_owns_it() {
        assert_eq!(MouseOwner::new(false), MouseOwner::Terminal);
        assert_eq!(MouseOwner::Terminal.badge(), " MOUSE: TERMINAL │");
    }
}
