//! Refuses `monarch <name>` when `name` looks like a mistyped command, so
//! `monarch doctr` doesn't quietly start a session called `doctr`.

use anyhow::bail;
use clap::CommandFactory;
use monarch_server::SocketPath;

use crate::Cli;

/// Fails when `session` is one letter away from a command and no session
/// by that name is running.
pub(crate) fn refuse_near_command(session: Option<&str>) -> anyhow::Result<()> {
    let Some(name) = session else {
        return Ok(());
    };
    let cli = Cli::command();
    let commands: Vec<&str> = cli
        .get_subcommands()
        .flat_map(|c| std::iter::once(c.get_name()).chain(c.get_all_aliases()))
        .collect();
    let running: Vec<String> = SocketPath::all()
        .iter()
        .filter_map(|s| Some(s.file_stem()?.to_string_lossy().into_owned()))
        .collect();
    if let Some(command) = near(name, &commands, &running) {
        bail!(
            "no command `{name}`; did you mean `monarch {command}`? \
             To start a session named {name}, run `monarch attach -s {name}`"
        );
    }
    Ok(())
}

/// The command `name` is one edit from, unless `name` is a running session.
fn near<'a>(name: &str, commands: &[&'a str], running: &[String]) -> Option<&'a str> {
    if running.iter().any(|r| r == name) {
        return None;
    }
    commands.iter().copied().find(|c| edits(name, c) == 1)
}

/// Single-character insertions, deletions and substitutions turning `a`
/// into `b` (Levenshtein distance).
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut row = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            row.push((prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1));
        }
        prev = row;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMANDS: [&str; 4] = ["doctor", "ls", "new", "pane"];

    #[test]
    fn typos_of_commands_are_caught() {
        assert_eq!(near("doctr", &COMMANDS, &[]), Some("doctor"));
        assert_eq!(near("nw", &COMMANDS, &[]), Some("new"));
        assert_eq!(near("lss", &COMMANDS, &[]), Some("ls"));
    }

    #[test]
    fn real_names_and_running_sessions_pass() {
        for name in ["concept", "main", "work", "pain-points"] {
            assert_eq!(near(name, &COMMANDS, &[]), None, "{name}");
        }
        assert_eq!(near("doctr", &COMMANDS, &["doctr".into()]), None, "running");
    }

    #[test]
    fn edits_counts_single_changes() {
        assert_eq!(edits("kitten", "sitting"), 3);
        assert_eq!(edits("", "ls"), 2);
        assert_eq!(edits("same", "same"), 0);
    }

    #[test]
    fn no_name_is_fine() {
        assert!(refuse_near_command(None).is_ok());
    }
}
