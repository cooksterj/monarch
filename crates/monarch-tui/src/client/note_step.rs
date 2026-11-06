//! What a key did in a review-note prompt.

/// The result of one key in a note prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteStep {
    /// Still open.
    Open,
    /// Closed without sending (Esc, or Enter on a blank note).
    Close,
    /// Close and send this note.
    Send(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_carry_their_text() {
        assert_eq!(NoteStep::Send("a".into()), NoteStep::Send("a".into()));
        assert_ne!(NoteStep::Send(String::new()), NoteStep::Close);
    }
}
