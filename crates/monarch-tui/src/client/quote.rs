//! What an ask quotes to the agent: text selected in a pane, or a hunk
//! from the review; and how quoted text is fenced so the agent can't
//! mistake where it ends.

use monarch_core::PaneId;

use super::ReviewNote;

/// Most selected lines quoted to an agent; the rest are counted.
pub const MAX_QUOTE: usize = 400;

/// The context an ask carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quote {
    /// Text selected in a pane's screen or scrollback.
    Screen {
        /// The pane it was selected in.
        pane: PaneId,
        /// The selected text, wrapped lines whole.
        text: String,
    },
    /// The hunk shown in the review.
    Hunk(ReviewNote),
}

impl Quote {
    /// The pane the quote came from (for a hunk, the reviewed pane).
    #[must_use]
    pub fn pane(&self) -> PaneId {
        match self {
            Self::Screen { pane, .. } => *pane,
            Self::Hunk(note) => note.pane,
        }
    }

    /// `17 lines from pane 3` or `src/a.rs @@ -1,3 +1,4 @@`, for the
    /// composer's title.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Screen { pane, text } => {
                let lines = text.trim_end().lines().count().max(1);
                let unit = if lines == 1 { "line" } else { "lines" };
                format!("{lines} {unit} from pane {}", pane.0)
            }
            Self::Hunk(note) => match note.hunk() {
                Some(hunk) => format!("{} {hunk}", note.path),
                None => note.path.clone(),
            },
        }
    }

    /// The prompt to type into the agent's pane: what the user asked,
    /// then the quote, fenced.
    #[must_use]
    pub fn message(&self, ask: &str) -> String {
        let ask = ask.trim();
        match self {
            Self::Screen { pane, text } => {
                let lines: Vec<&str> = text.trim_end().lines().collect();
                let excerpt = Self::fenced("", &lines, MAX_QUOTE);
                format!("{ask}\n\nSelected in pane {}:\n{excerpt}", pane.0)
            }
            Self::Hunk(note) => {
                let place = match note.hunk() {
                    Some(hunk) => format!("{} at {hunk}", note.path),
                    None => note.path.clone(),
                };
                format!("{ask}\n\nAbout {place}:\n{}", note.excerpt())
            }
        }
    }

    /// `lines` (at most `max`, then a count of the rest) in a code fence
    /// tagged `lang`, longer than any run of backticks inside so the
    /// quote can't close it early.
    #[must_use]
    pub fn fenced<S: AsRef<str>>(lang: &str, lines: &[S], max: usize) -> String {
        let longest = lines
            .iter()
            .map(|l| longest_backticks(l.as_ref()))
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(longest.max(2) + 1);
        let mut out = format!("{fence}{lang}\n");
        for line in lines.iter().take(max) {
            out.push_str(line.as_ref());
            out.push('\n');
        }
        if lines.len() > max {
            out.push_str(&format!("… {} more lines\n", lines.len() - max));
        }
        out.push_str(&fence);
        out
    }
}

/// The longest run of backticks in `line`.
fn longest_backticks(line: &str) -> usize {
    line.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use monarch_proto::{DiffLine, DiffLineKind};

    use super::*;

    fn screen(text: &str) -> Quote {
        Quote::Screen {
            pane: PaneId(3),
            text: text.into(),
        }
    }

    #[test]
    fn a_selection_is_quoted_after_the_ask() {
        let quote = screen("fn main() {\n    run();\n}\n\n");
        assert_eq!(quote.pane(), PaneId(3));
        assert_eq!(quote.summary(), "3 lines from pane 3");
        assert_eq!(
            quote.message("  Why does this panic?  "),
            "Why does this panic?\n\nSelected in pane 3:\n```\nfn main() {\n    run();\n}\n```"
        );
        assert_eq!(screen("x").summary(), "1 line from pane 3");
        assert_eq!(screen("").summary(), "1 line from pane 3", "never 0");
    }

    #[test]
    fn backticks_inside_lengthen_the_fence() {
        let quote = screen("```rust\nlet a = 1;\n```");
        let message = quote.message("Fix it.");
        assert!(message.ends_with("````\n```rust\nlet a = 1;\n```\n````"));
    }

    #[test]
    fn long_selections_are_cut_and_counted() {
        let text: Vec<String> = (0..MAX_QUOTE + 3).map(|i| format!("l{i}")).collect();
        let message = screen(&text.join("\n")).message("Summarise.");
        assert!(message.contains(&format!("l{}\n… 3 more lines\n```", MAX_QUOTE - 1)));
        assert!(!message.contains(&format!("l{MAX_QUOTE}\n")));
    }

    #[test]
    fn a_hunk_names_its_place_and_quotes_the_diff() {
        let quote = Quote::Hunk(ReviewNote {
            pane: PaneId(5),
            path: "src/a.rs".into(),
            lines: vec![
                DiffLine::new(DiffLineKind::Hunk, "@@ -1 +1 @@"),
                DiffLine::new(DiffLineKind::Added, "new();"),
            ],
        });
        assert_eq!(quote.pane(), PaneId(5));
        assert_eq!(quote.summary(), "src/a.rs @@ -1 +1 @@");
        assert_eq!(
            quote.message("Is this safe?"),
            "Is this safe?\n\nAbout src/a.rs at @@ -1 +1 @@:\n```diff\n@@ -1 +1 @@\n+new();\n```"
        );
    }
}
