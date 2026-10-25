//! `monarch new`: a git worktree for a branch, and a tab working in it.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use monarch_core::LoadedConfig;
use monarch_proto::{NewWorktree, WorktreeCreated};
use monarch_server::SocketPath;

use super::attach;
use super::auto_start::ensure_server;
use crate::hook::ApiClient;
use crate::{AttachArgs, NewArgs};

/// How long git may take to check the worktree out.
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Creates the worktree through the session's server (starting it if
/// needed), then attaches unless already inside that session.
pub(crate) fn run(
    args: &NewArgs,
    loaded: &LoadedConfig,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let config = &loaded.config;
    let inside = std::env::var("MONARCH_PANE")
        .is_ok()
        .then(|| std::env::var("MONARCH_SESSION").ok())
        .flatten();
    let session = super::session_name(args.session.clone(), config);
    let socket = SocketPath::for_session(&session)?;
    ensure_server(&session, &socket, &[])?;
    let request = NewWorktree {
        dir: std::env::current_dir()
            .context("reading the current directory")?
            .to_string_lossy()
            .into_owned(),
        name: args.name.clone(),
        base: args.base.clone(),
        agent: args.agent,
        root: (config.projects.worktree_root.as_ref()).map(|r| r.to_string_lossy().into_owned()),
    };
    let mut client = ApiClient::connect(&socket)?;
    client.set_timeout(GIT_TIMEOUT)?;
    let created: WorktreeCreated = client
        .request(NewWorktree::METHOD, &request)
        .context("cannot create worktree")?;
    writeln!(out, "{}", summary(&created, &session))?;
    if args.detached || inside.is_some() {
        return Ok(ExitCode::SUCCESS);
    }
    let attach = AttachArgs {
        session: Some(session),
        agents: Vec::new(),
    };
    attach::run(&attach, loaded, out)
}

/// What was made, in one line.
fn summary(created: &WorktreeCreated, session: &str) -> String {
    let branch = if created.new_branch {
        "new branch"
    } else {
        "branch"
    };
    format!(
        "worktree {} on {branch} {}; opened in pane {} of session {session}",
        created.path, created.branch, created.pane
    )
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;

    use super::*;

    #[test]
    fn the_summary_says_whether_the_branch_is_new() {
        let mut created = WorktreeCreated {
            path: "/r-worktrees/feat".into(),
            branch: "feat".into(),
            new_branch: true,
            pane: PaneId(4),
        };
        assert_eq!(
            summary(&created, "main"),
            "worktree /r-worktrees/feat on new branch feat; opened in pane p4 of session main"
        );
        created.new_branch = false;
        assert!(summary(&created, "w").contains("on branch feat"));
    }
}
