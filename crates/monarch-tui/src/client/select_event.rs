//! Selection gestures, from the input thread to the main loop.

/// One step of a mouse selection, in pane-local cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectEvent {
    /// The left button went down: 1, 2 or 3 clicks in quick succession.
    Start {
        /// Pane column.
        col: u16,
        /// Pane row.
        row: u16,
        /// Click count.
        clicks: u8,
    },
    /// The pointer moved with the button held (clamped to the pane).
    Extend {
        /// Pane column.
        col: u16,
        /// Pane row.
        row: u16,
        /// Rows past the pane's top (negative) or bottom (positive); the
        /// pane scrolls while this is not 0.
        beyond: i16,
    },
    /// The button came up: copy what is selected.
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_compare_by_value() {
        assert_eq!(SelectEvent::End, SelectEvent::End);
        assert_ne!(
            SelectEvent::Extend {
                col: 1,
                row: 1,
                beyond: 0
            },
            SelectEvent::End
        );
    }
}
