//! A piece of pane-bound host input: key bytes or a mouse report.

use super::MouseEvent;

/// Keys and mouse reports in the order the host sent them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostInput {
    /// Bytes for the focused pane (keys, paste).
    Keys(Vec<u8>),
    /// A decoded mouse report.
    Mouse(MouseEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_compare_by_value() {
        assert_eq!(
            HostInput::Keys(b"a".to_vec()),
            HostInput::Keys(b"a".to_vec())
        );
        assert_ne!(
            HostInput::Keys(b"a".to_vec()),
            HostInput::Keys(b"b".to_vec())
        );
    }
}
