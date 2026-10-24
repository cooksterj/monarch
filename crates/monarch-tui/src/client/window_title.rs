//! The host window title: `[ui].window_title` with its placeholders filled.

use monarch_core::PaneId;

/// Expands `{title}`, `{session}` and `{pane}` in a title template.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowTitle;

impl WindowTitle {
    /// The title for the host window. Separators left dangling by an empty
    /// pane title are trimmed, so `{title} — monarch:{session}` becomes
    /// `monarch:main` rather than ` — monarch:main`.
    #[must_use]
    pub fn format(
        template: &str,
        session: &str,
        pane: Option<PaneId>,
        title: Option<&str>,
    ) -> String {
        let pane = pane.map(|p| p.0.to_string()).unwrap_or_default();
        template
            .replace("{title}", title.unwrap_or_default())
            .replace("{session}", session)
            .replace("{pane}", &pane)
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, '—' | '-' | '|' | '│' | ':'))
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "{title} — monarch:{session}";

    #[test]
    fn fills_every_placeholder() {
        let t = WindowTitle::format(
            "{pane}:{title}@{session}",
            "main",
            Some(PaneId(3)),
            Some("vim"),
        );
        assert_eq!(t, "3:vim@main");
    }

    #[test]
    fn default_template_with_and_without_a_pane_title() {
        assert_eq!(
            WindowTitle::format(DEFAULT, "main", None, Some("vim")),
            "vim — monarch:main"
        );
        assert_eq!(
            WindowTitle::format(DEFAULT, "main", None, None),
            "monarch:main"
        );
        assert_eq!(
            WindowTitle::format(DEFAULT, "w", None, Some("")),
            "monarch:w"
        );
    }
}
