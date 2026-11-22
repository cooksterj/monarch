//! `monarch view <file>`: opens the file viewer over the pane it runs in,
//! the file highlighted, as `prefix f` would.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use monarch_core::PaneId;
use monarch_proto::ViewFile;

use crate::ViewArgs;

/// Asks the session this runs in to show the file over this pane.
pub(crate) fn run(args: &ViewArgs) -> anyhow::Result<ExitCode> {
    let (Ok(pane), Ok(session)) = (
        std::env::var("MONARCH_PANE"),
        std::env::var("MONARCH_SESSION"),
    ) else {
        bail!("`monarch view` shows the file over the pane it runs in: run it inside Monarch");
    };
    let request = request(&args.path, &pane, &std::env::current_dir()?)?;
    let mut client = super::connect(&session)?;
    let _: serde_json::Value = super::older(client.request(ViewFile::METHOD, &request), "view")?;
    Ok(ExitCode::SUCCESS)
}

/// The request: `path` made absolute from `cwd`, which the server can't
/// see, for the pane named by `MONARCH_PANE`.
fn request(path: &Path, pane: &str, cwd: &Path) -> anyhow::Result<ViewFile> {
    let pane: PaneId = pane
        .parse()
        .map_err(|_| anyhow!("MONARCH_PANE is {pane:?}, not a pane"))?;
    Ok(ViewFile {
        pane,
        path: cwd.join(path).to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_made_absolute_from_the_shells_directory() {
        let cwd = Path::new("/w/crates");
        let view = request(Path::new("core/a.rs"), "p3", cwd).expect("request");
        assert_eq!(
            (view.pane, view.path.as_str()),
            (PaneId(3), "/w/crates/core/a.rs")
        );
        let abs = request(Path::new("/etc/hosts"), "p3", cwd).expect("request");
        assert_eq!(abs.path, "/etc/hosts", "absolute stays as typed");
        let err = request(Path::new("a"), "nonsense", cwd).expect_err("bad pane");
        assert!(err.to_string().contains("not a pane"), "{err}");
    }
}
