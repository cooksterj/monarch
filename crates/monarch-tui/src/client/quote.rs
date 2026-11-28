//! What an ask quotes to the agent: text selected in a pane, lines of a
//! file from the viewer, or a hunk from the review; and how quoted text
//! is fenced so the agent can't mistake where it ends.

use monarch_core::PaneId;
use monarch_proto::QuoteSource;

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
    /// Lines of a file, selected in the file viewer.
    File {
        /// The pane whose checkout holds it.
        pane: PaneId,
        /// The file, from the checkout's top.
        path: String,
        /// Its language, to tag the fence (`rust`).
        language: Option<String>,
        /// The first line's number, counting from 1.
        start: usize,
        /// The lines.
        lines: Vec<String>,
    },
    /// The hunk shown in the review.
    Hunk(ReviewNote),
}

impl Quote {
    /// Text selected on `pane`'s screen: the file lines it shows when the
    /// server found them in the pane's checkout, else the text itself.
    #[must_use]
    pub fn selected(pane: PaneId, text: String, source: Option<QuoteSource>) -> Self {
        match source {
            Some(source) => Self::File {
                pane,
                path: source.path,
                language: source.language,
                start: source.start,
                lines: source.lines,
            },
            None => Self::Screen { pane, text },
        }
    }

    /// The pane the quote came from (for a hunk, the reviewed pane).
    #[must_use]
    pub fn pane(&self) -> PaneId {
        match self {
            Self::Screen { pane, .. } | Self::File { pane, .. } => *pane,
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
            Self::File { .. } => self.place(),
            Self::Hunk(note) => match note.hunk() {
                Some(hunk) => format!("{} {hunk}", note.path),
                None => note.path.clone(),
            },
        }
    }

    /// `src/a.rs:42-58` (or `:42` for one line) for a file quote; the
    /// summary for the rest.
    fn place(&self) -> String {
        match self {
            Self::File {
                path, start, lines, ..
            } => match lines.len() {
                0 | 1 => format!("{path}:{start}"),
                n => format!("{path}:{start}-{}", start + n - 1),
            },
            _ => self.summary(),
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
            Self::File {
                language, lines, ..
            } => {
                let lang = language.as_deref().unwrap_or_default();
                let excerpt = Self::fenced(lang, lines, MAX_QUOTE);
                format!("{ask}\n\nAbout {}:\n{excerpt}", self.place())
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
    fn a_selection_found_in_the_checkout_quotes_the_file_lines() {
        let source = QuoteSource {
            path: "src/a.rs".into(),
            language: Some("rust".into()),
            start: 7,
            lines: vec!["    run();".into()],
        };
        let quote = Quote::selected(PaneId(3), "  7 │ run();".into(), Some(source));
        assert_eq!(quote.pane(), PaneId(3));
        assert_eq!(quote.summary(), "src/a.rs:7");
        assert_eq!(
            quote.message("Why?"),
            "Why?\n\nAbout src/a.rs:7:\n```rust\n    run();\n```"
        );
        let unfound = Quote::selected(PaneId(3), "x".into(), None);
        assert_eq!(unfound, screen("x"), "else the screen's text");
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
    fn file_lines_name_the_path_and_range_and_tag_the_language() {
        let quote = |lines: &[&str]| Quote::File {
            pane: PaneId(2),
            path: "src/a.rs".into(),
            language: Some("rust".into()),
            start: 42,
            lines: lines.iter().map(|l| (*l).to_owned()).collect(),
        };
        let two = quote(&["fn a() {", "}"]);
        assert_eq!(two.pane(), PaneId(2));
        assert_eq!(two.summary(), "src/a.rs:42-43");
        assert_eq!(
            two.message("Why?"),
            "Why?\n\nAbout src/a.rs:42-43:\n```rust\nfn a() {\n}\n```"
        );
        assert_eq!(quote(&["x"]).summary(), "src/a.rs:42");
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
