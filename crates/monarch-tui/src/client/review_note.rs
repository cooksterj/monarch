//! A note from the review pane, worded for the agent that made the
//! change: which file, which hunk, and what the user said.

use monarch_core::PaneId;
use monarch_proto::{DiffLine, DiffLineKind};

/// Most diff lines quoted back to the agent.
pub const MAX_EXCERPT: usize = 40;

/// What a note is about, before the user has written it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewNote {
    /// The agent's pane.
    pub pane: PaneId,
    /// The file, from the checkout's top.
    pub path: String,
    /// The hunk the note is on (its header first, when it has one).
    pub lines: Vec<DiffLine>,
}

impl ReviewNote {
    /// The hunk's header, if the lines start with one.
    #[must_use]
    pub fn hunk(&self) -> Option<&str> {
        self.lines
            .first()
            .filter(|l| l.kind == DiffLineKind::Hunk)
            .map(|l| l.text.as_str())
    }

    /// `note on src/a.rs @@ -1,3 +1,4 @@`, for the prompt's title.
    #[must_use]
    pub fn title(&self) -> String {
        match self.hunk() {
            Some(hunk) => format!("note on {} {hunk}", self.path),
            None => format!("note on {}", self.path),
        }
    }

    /// The message to type into the agent's pane: where the note is,
    /// `text`, and the hunk quoted as a diff so the agent can't mistake
    /// the place.
    #[must_use]
    pub fn message(&self, text: &str) -> String {
        let mut out = match self.hunk() {
            Some(hunk) => format!("Review note on {} at {hunk}:\n\n", self.path),
            None => format!("Review note on {}:\n\n", self.path),
        };
        out.push_str(text.trim());
        out.push_str("\n\nThe change in question:\n```diff\n");
        for line in self.lines.iter().take(MAX_EXCERPT) {
            out.push_str(line.kind.marker());
            out.push_str(&line.text);
            out.push('\n');
        }
        if self.lines.len() > MAX_EXCERPT {
            let more = self.lines.len() - MAX_EXCERPT;
            out.push_str(&format!("… {more} more lines\n"));
        }
        out.push_str("```");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(lines: Vec<DiffLine>) -> ReviewNote {
        ReviewNote {
            pane: PaneId(4),
            path: "src/parse.rs".into(),
            lines,
        }
    }

    #[test]
    fn a_hunk_note_names_the_place_and_quotes_the_diff() {
        let note = note(vec![
            DiffLine::new(DiffLineKind::Hunk, "@@ -40,3 +40,4 @@"),
            DiffLine::new(DiffLineKind::Context, "fn parse() {"),
            DiffLine::new(DiffLineKind::Removed, "    old();"),
            DiffLine::new(DiffLineKind::Added, "    new();"),
        ]);
        assert_eq!(note.title(), "note on src/parse.rs @@ -40,3 +40,4 @@");
        assert_eq!(
            note.message("  Handle the empty case.  "),
            "Review note on src/parse.rs at @@ -40,3 +40,4 @@:\n\n\
             Handle the empty case.\n\n\
             The change in question:\n```diff\n\
             @@ -40,3 +40,4 @@\n fn parse() {\n-    old();\n+    new();\n```"
        );
    }

    #[test]
    fn a_new_file_has_no_hunk_and_long_excerpts_are_cut() {
        let lines = (0..MAX_EXCERPT + 2)
            .map(|i| DiffLine::new(DiffLineKind::Added, format!("line {i}")))
            .collect();
        let note = note(lines);
        assert_eq!(note.hunk(), None);
        assert_eq!(note.title(), "note on src/parse.rs");
        let message = note.message("Too long.");
        assert!(message.starts_with("Review note on src/parse.rs:\n\nToo long.\n"));
        assert!(message.contains("+line 39\n… 2 more lines\n```"));
        assert!(!message.contains("line 40"));
    }
}
