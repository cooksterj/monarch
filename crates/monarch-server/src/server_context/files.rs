//! The file viewer's requests: a pane's checkout's files, and one of them
//! read and highlighted.

use std::path::{Path, PathBuf};

use monarch_core::PaneId;
use monarch_proto::{CodeFile, ServerMsg, ViewFile};

use super::ServerContext;

impl ServerContext {
    /// The files in `pane`'s checkout. Runs git: call it off the async
    /// runtime.
    ///
    /// # Errors
    /// The pane isn't in a git checkout, or git failed.
    pub fn list_files(&self, pane: PaneId) -> Result<Vec<String>, String> {
        self.files.list(&self.checkout(pane)?)
    }

    /// `path` from `pane`'s checkout, highlighted. Reads the disk: call it
    /// off the async runtime.
    ///
    /// # Errors
    /// The pane isn't in a checkout, or the file can't be shown.
    pub fn open_file(&self, pane: PaneId, path: &str) -> Result<CodeFile, String> {
        self.files.open(&self.checkout(pane)?, path)
    }

    /// `pane.view`: reads `view.path` (absolute, resolved in the pane's
    /// shell) and has every attached client open the viewer over the pane
    /// on it. Reads the disk: call it off the async runtime.
    ///
    /// # Errors
    /// No such pane, a relative path, or the file can't be shown.
    pub fn view_file(&self, view: &ViewFile) -> Result<(), String> {
        if !self.lock().pane_ids().contains(&view.pane) {
            return Err(format!("no pane {} in this session", view.pane));
        }
        let path = Path::new(&view.path);
        if !path.is_absolute() {
            return Err(format!("{} is not an absolute path", view.path));
        }
        let failed = |e: std::io::Error| format!("can't read {}: {e}", view.path);
        // `monarch view src/a.rs:42` names a line, unless that is the file.
        let (real, line) = match (path.canonicalize(), named(&view.path)) {
            (Ok(real), _) => (real, None),
            (Err(e), Some((name, line @ Some(_)))) => {
                (Path::new(name).canonicalize().map_err(|_| failed(e))?, line)
            }
            (Err(e), _) => return Err(failed(e)),
        };
        let file = self.files.open_at(&real, &self.shown(view.pane, &real))?;
        let pane = view.pane;
        self.lock()
            .broadcast(&ServerMsg::ViewFile { pane, file, line });
        Ok(())
    }

    /// The file `text` (selected in `pane`) names, read and highlighted:
    /// a path from the pane's directory or home (`~/`), a trailing
    /// `:line` or `:line:col` returned as the line to open on. Reads the
    /// disk: call it off the async runtime.
    ///
    /// # Errors
    /// The text names no readable file.
    pub fn open_named(
        &self,
        pane: PaneId,
        text: &str,
    ) -> Result<(CodeFile, Option<usize>), String> {
        let (name, line) = named(text).ok_or_else(|| format!("{text:?} is not a path"))?;
        let cwd = self.lock().place(pane).map(|p| p.cwd);
        let cwd = cwd.ok_or_else(|| "this pane's directory isn't known yet".to_owned())?;
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let path = match (name.strip_prefix("~/"), home) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => Path::new(&cwd).join(name),
        };
        let failed = |e: std::io::Error| format!("can't read {name}: {e}");
        let real = path.canonicalize().map_err(failed)?;
        let file = self.files.open_at(&real, &self.shown(pane, &real))?;
        Ok((file, line))
    }

    /// What to search the checkout for when `text` names no file: the
    /// path it seems to be (`a.rs` for `a.rs:42`), else the text trimmed.
    #[must_use]
    pub fn search_text(text: &str) -> String {
        named(text).map_or(text.trim(), |(name, _)| name).to_owned()
    }

    /// How the viewer names `real`: from the top of `pane`'s checkout when
    /// it is inside (so asks and edits use the checkout's paths), else
    /// whole.
    fn shown(&self, pane: PaneId, real: &Path) -> String {
        let top = self.checkout(pane).ok().and_then(|r| r.canonicalize().ok());
        let inside = top.and_then(|top| real.strip_prefix(top).ok().map(Path::to_path_buf));
        inside
            .unwrap_or_else(|| real.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    /// The top of `pane`'s checkout.
    fn checkout(&self, pane: PaneId) -> Result<PathBuf, String> {
        let place = self.lock().place(pane);
        let root = place.and_then(|p| p.worktree);
        root.map(PathBuf::from)
            .ok_or_else(|| "this pane isn't in a git checkout".to_owned())
    }
}

/// The path in a selection and the line it names: one line, without
/// surrounding quotes, a trailing `:line[:col]` (as compilers and agents
/// print them) taken as the line.
fn named(text: &str) -> Option<(&str, Option<usize>)> {
    let text = text.trim();
    if text.contains('\n') {
        return None;
    }
    let mut name = text.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    let mut line = None;
    for _ in 0..2 {
        if let Some((head, tail)) = name.rsplit_once(':')
            && let Ok(number) = tail.parse::<usize>()
        {
            name = head;
            line = Some(number);
        }
    }
    (!name.is_empty()).then_some((name, line))
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{PanePlace, ServerMsg, ViewFile};

    use crate::test_support::{FakePane, context};
    use crate::{PaneSpec, ServerContext};

    #[test]
    fn panes_outside_a_checkout_have_no_files() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let err = "this pane isn't in a git checkout".to_owned();
        assert_eq!(ctx.list_files(pane), Err(err.clone()));
        assert_eq!(ctx.open_file(pane, "a.rs"), Err(err));
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").expect("write");
        let root = dir.path().to_string_lossy().into_owned();
        let place = PanePlace {
            worktree: Some(root.clone()),
            ..PanePlace::outside(root)
        };
        ctx.lock().set_place(pane, Some(place));
        let file = ctx.open_file(pane, "a.rs").expect("opens");
        assert_eq!(file.path, "a.rs");
    }

    #[test]
    fn viewing_sends_the_file_to_clients_named_from_the_checkout_when_inside() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let dir = tempfile::tempdir().expect("tempdir");
        let elsewhere = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").expect("write");
        std::fs::write(elsewhere.path().join("notes.txt"), "hi").expect("write");
        let root = dir.path().to_string_lossy().into_owned();
        let place = PanePlace {
            worktree: Some(root.clone()),
            ..PanePlace::outside(root)
        };
        ctx.lock().set_place(pane, Some(place));
        let (lane, mut rx) = tokio::sync::mpsc::channel(8);
        ctx.lock().add_client(lane);
        while rx.try_recv().is_ok() {}
        let view = |path: &std::path::Path| ViewFile {
            pane,
            path: path.to_string_lossy().into_owned(),
        };
        let mut sent = || match rx.try_recv().expect("sent").pop() {
            Some(ServerMsg::ViewFile { file, line, .. }) => (file.path, line),
            other => panic!("{other:?}"),
        };
        let mut shown = || sent().0;
        ctx.view_file(&view(&dir.path().join("a.rs")))
            .expect("inside");
        assert_eq!(shown(), "a.rs", "from the checkout's top");
        let notes = elsewhere.path().join("notes.txt");
        ctx.view_file(&view(&notes)).expect("outside");
        let real = notes.canonicalize().expect("real");
        assert_eq!(shown(), real.to_string_lossy(), "whole");
        let at_line = dir.path().join("a.rs:1");
        ctx.view_file(&view(&at_line)).expect("a line");
        assert_eq!(sent(), ("a.rs".to_owned(), Some(1)));
    }

    #[test]
    fn viewing_refuses_relative_paths_and_unknown_panes() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let relative = ViewFile {
            pane,
            path: "a.rs".into(),
        };
        assert_eq!(
            ctx.view_file(&relative),
            Err("a.rs is not an absolute path".into())
        );
        let missing = ViewFile {
            pane: PaneId(99),
            path: "/x".into(),
        };
        let err = ctx.view_file(&missing).expect_err("no pane");
        assert!(err.starts_with("no pane"), "{err}");
    }

    #[test]
    fn a_selection_names_a_path_without_quotes_or_a_line_number() {
        use super::named;
        assert_eq!(named("  src/a.rs  "), Some(("src/a.rs", None)));
        assert_eq!(named("`src/a.rs:42`"), Some(("src/a.rs", Some(42))));
        assert_eq!(
            named("src/a.rs:42:7"),
            Some(("src/a.rs", Some(42))),
            "the line, not the column"
        );
        assert_eq!(named("C:a"), Some(("C:a", None)), "only digits are a line");
        assert_eq!(named("two\nlines"), None);
        assert_eq!(named(" "), None);
        assert_eq!(ServerContext::search_text(" a.rs:42 "), "a.rs");
        assert_eq!(ServerContext::search_text("x\ny"), "x\ny");
    }

    #[test]
    fn selected_names_are_found_from_the_panes_directory() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        assert!(ctx.open_named(pane, "a.rs").is_err(), "directory unknown");
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src/a.rs"), "fn a() {}").expect("write");
        let cwd = dir.path().join("src").to_string_lossy().into_owned();
        ctx.lock().set_place(pane, Some(PanePlace::outside(cwd)));
        let (file, line) = ctx.open_named(pane, "a.rs:3").expect("found");
        assert!(file.path.ends_with("src/a.rs"), "{}", file.path);
        assert_eq!(line, Some(3));
        let err = ctx.open_named(pane, "missing.rs").expect_err("missing");
        assert!(err.starts_with("can't read missing.rs"), "{err}");
    }
}
