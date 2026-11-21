//! The file viewer's requests: a pane's checkout's files, and one of them
//! read and highlighted.

use std::path::PathBuf;

use monarch_core::PaneId;
use monarch_proto::CodeFile;

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

    /// The top of `pane`'s checkout.
    fn checkout(&self, pane: PaneId) -> Result<PathBuf, String> {
        let place = self.lock().place(pane);
        let root = place.and_then(|p| p.worktree);
        root.map(PathBuf::from)
            .ok_or_else(|| "this pane isn't in a git checkout".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::PanePlace;

    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

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
}
