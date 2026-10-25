//! Where one hook command sits in an agent's config.

/// A hook command under an event: its matcher group, its place in the
/// group, and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookSlot {
    /// Index of the matcher group in the event's list.
    pub group: usize,
    /// Index of the hook in the group's `hooks`.
    pub index: usize,
    /// The hook's `command`.
    pub command: String,
}

impl HookSlot {
    /// A slot at (`group`, `index`) running `command`.
    pub fn new(group: usize, index: usize, command: impl Into<String>) -> Self {
        Self {
            group,
            index,
            command: command.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_order_by_position() {
        let mut slots = [HookSlot::new(1, 0, "b"), HookSlot::new(0, 2, "a")];
        slots.sort_by_key(|s| (s.group, s.index));
        assert_eq!(slots[0].command, "a");
        assert_ne!(slots[0], HookSlot::new(0, 2, "other"));
    }
}
