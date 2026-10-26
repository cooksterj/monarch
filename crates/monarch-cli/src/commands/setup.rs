//! `monarch setup`: installs (or removes) Monarch's hooks in each agent's
//! settings, leaving everything else in those files alone, and the skill
//! that teaches agents to use Monarch.
//!
//! Re-running is safe: hooks that are already right are not touched (Codex
//! remembers which hooks the user trusted), wrong paths are fixed in
//! place and duplicates dropped. Every write is preceded by a backup.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

use crate::setup::{
    Change, ChangeKind, ClaudeSettings, CodexConfig, ConfigFile, HookConfig, HookLine, SkillFile,
};
use crate::{HookCommand, SetupArgs};

/// Where the agents keep their settings and what the hooks should run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetupEnv {
    /// Claude Code's config directory (`~/.claude`).
    pub claude: PathBuf,
    /// Codex's home (`~/.codex`).
    pub codex: PathBuf,
    /// The shared agents directory (`~/.agents`), where Codex finds
    /// skills.
    pub agents: PathBuf,
    /// What hooks run: `monarch` when that is this binary, else its path.
    pub program: PathBuf,
    /// Distinguishes this run's backups.
    pub stamp: u64,
}

impl SetupEnv {
    /// Reads `HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PATH` and this
    /// binary's location.
    pub fn from_env() -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let dir = |var: &str, default: &str| {
            std::env::var_os(var).map_or_else(|| home.join(default), PathBuf::from)
        };
        let exe = std::env::current_exe()
            .and_then(|p| p.canonicalize())
            .unwrap_or_else(|_| PathBuf::from("monarch"));
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH);
        Self {
            claude: dir("CLAUDE_CONFIG_DIR", ".claude"),
            codex: dir("CODEX_HOME", ".codex"),
            agents: home.join(".agents"),
            program: program(&exe, std::env::var_os("PATH").as_deref()),
            stamp: stamp.map_or(0, |d| d.as_secs()),
        }
    }

    /// `agent`'s settings directory and file.
    pub(crate) fn files(&self, agent: HookCommand) -> (&Path, PathBuf) {
        match agent {
            HookCommand::Claude => (&self.claude, self.claude.join("settings.json")),
            HookCommand::Codex => (&self.codex, self.codex.join("config.toml")),
        }
    }

    /// The directory `agent` finds user skills in.
    pub(crate) fn skills(&self, agent: HookCommand) -> PathBuf {
        match agent {
            HookCommand::Claude => self.claude.join("skills"),
            HookCommand::Codex => self.agents.join("skills"),
        }
    }
}

/// `monarch` if the first `monarch` on `path` is `exe`, else `exe`.
fn program(exe: &Path, path: Option<&std::ffi::OsStr>) -> PathBuf {
    let first = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join("monarch"))
        .find(|p| p.is_file());
    match first.and_then(|p| p.canonicalize().ok()) {
        Some(found) if found == exe => PathBuf::from("monarch"),
        _ => exe.to_owned(),
    }
}

/// Sets up every installed agent; reports each, and fails if any did.
pub(crate) fn run(
    args: SetupArgs,
    env: &SetupEnv,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    if args.dry_run {
        writeln!(out, "Dry run: nothing is written.")?;
    }
    let mut code = ExitCode::SUCCESS;
    for agent in HookCommand::ALL {
        if let Err(e) = setup_agent(agent, args, env, out) {
            writeln!(out, "{}: {e:#}", agent.title())?;
            code = ExitCode::FAILURE;
        }
    }
    Ok(code)
}

fn setup_agent(
    agent: HookCommand,
    args: SetupArgs,
    env: &SetupEnv,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let (dir, path) = env.files(agent);
    if !dir.is_dir() {
        writeln!(
            out,
            "{}: not installed ({} not found), skipped",
            agent.title(),
            dir.display()
        )?;
        return Ok(());
    }
    let file = ConfigFile::read(&path)?;
    let mut config =
        parse(agent, file.text.as_deref()).with_context(|| path.display().to_string())?;
    let changes = if args.uninstall {
        config.uninstall(agent)
    } else {
        config.install(&HookLine::new(&env.program, agent))
    };
    writeln!(out, "{} ({})", agent.title(), path.display())?;
    report(&changes, args, out)?;
    let edited = changes.iter().any(|c| c.kind.edits());
    if edited && !args.dry_run {
        if let Some(backup) = file.replace(&config.render(), env.stamp)? {
            writeln!(out, "  backup     {}", backup.display())?;
        }
        if agent == HookCommand::Codex && !args.uninstall {
            writeln!(
                out,
                "  Codex asks you to trust new or changed hooks: open /hooks in Codex."
            )?;
        }
    }
    setup_skill(agent, args, env, out)
}

/// Installs, updates or removes Monarch's skill for `agent`.
fn setup_skill(
    agent: HookCommand,
    args: SetupArgs,
    env: &SetupEnv,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let skill = SkillFile::read(&env.skills(agent))?;
    let kind = skill.plan(args.uninstall);
    if args.uninstall && kind == ChangeKind::Unchanged {
        return Ok(());
    }
    let verb = kind.verb(args.dry_run);
    writeln!(out, "  {verb:<10} skill {}", skill.path().display())?;
    if kind.edits()
        && !args.dry_run
        && let Some(backup) = skill.apply(kind, env.stamp)?
    {
        writeln!(out, "  backup     {}", backup.display())?;
    }
    Ok(())
}

/// What `monarch setup` would change for `agent`, without writing:
/// `None` when the agent isn't installed.
///
/// # Errors
/// Its settings file can't be read or parsed.
pub(crate) fn plan(agent: HookCommand, env: &SetupEnv) -> anyhow::Result<Option<Vec<Change>>> {
    let (dir, path) = env.files(agent);
    if !dir.is_dir() {
        return Ok(None);
    }
    let file = ConfigFile::read(&path)?;
    let mut config =
        parse(agent, file.text.as_deref()).with_context(|| path.display().to_string())?;
    Ok(Some(config.install(&HookLine::new(&env.program, agent))))
}

fn parse(agent: HookCommand, text: Option<&str>) -> anyhow::Result<Box<dyn HookConfig>> {
    Ok(match agent {
        HookCommand::Claude => Box::new(ClaudeSettings::parse(text)?),
        HookCommand::Codex => Box::new(CodexConfig::parse(text)?),
    })
}

/// One line per outcome, naming its events.
fn report(changes: &[Change], args: SetupArgs, out: &mut dyn Write) -> std::io::Result<()> {
    if changes.is_empty() {
        return writeln!(out, "  no Monarch hooks");
    }
    let mut by_kind: BTreeMap<ChangeKind, Vec<&str>> = BTreeMap::new();
    for change in changes {
        by_kind.entry(change.kind).or_default().push(&change.event);
    }
    for (kind, events) in by_kind {
        writeln!(
            out,
            "  {:<10} {}",
            kind.verb(args.dry_run),
            events.join(", ")
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn env(home: &Path) -> SetupEnv {
        SetupEnv {
            claude: home.join(".claude"),
            codex: home.join(".codex"),
            agents: home.join(".agents"),
            program: PathBuf::from("/opt/monarch"),
            stamp: 1,
        }
    }

    fn run_with(args: SetupArgs, env: &SetupEnv) -> (String, ExitCode) {
        let mut out = Vec::new();
        let code = run(args, env, &mut out).expect("runs");
        (String::from_utf8(out).expect("utf8"), code)
    }

    #[test]
    fn installs_then_finds_nothing_to_do() {
        let home = tempfile::tempdir().expect("dir");
        let env = env(home.path());
        fs::create_dir(&env.claude).expect("claude dir");
        let (text, code) = run_with(SetupArgs::default(), &env);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(
            text.contains("  added      SessionStart, UserPromptSubmit"),
            "{text}"
        );
        assert!(text.contains("Codex: not installed"), "{text}");
        assert!(text.contains("  added      skill "), "{text}");
        assert!(env.claude.join("skills/monarch/SKILL.md").is_file());
        assert!(
            !env.agents.exists(),
            "Codex isn't installed: no skill for it"
        );
        let written = fs::read_to_string(env.claude.join("settings.json")).expect("written");
        assert!(written.contains("\"command\": \"/opt/monarch hook claude\""));
        let (again, _) = run_with(SetupArgs::default(), &env);
        assert!(again.contains("  unchanged  SessionStart"), "{again}");
        assert!(again.contains("  unchanged  skill "), "{again}");
        assert!(!again.contains("backup"), "nothing written: {again}");
    }

    #[test]
    fn dry_runs_write_nothing_and_uninstall_backs_up() {
        let home = tempfile::tempdir().expect("dir");
        let env = env(home.path());
        fs::create_dir(&env.codex).expect("codex dir");
        let dry = SetupArgs {
            dry_run: true,
            ..SetupArgs::default()
        };
        let (text, _) = run_with(dry, &env);
        assert!(
            text.contains("would add") && !env.codex.join("config.toml").exists(),
            "{text}"
        );
        assert!(text.contains("  would add  skill "), "{text}");
        assert!(!env.agents.exists(), "a dry run writes no skill");
        let (text, _) = run_with(SetupArgs::default(), &env);
        assert!(text.contains("open /hooks in Codex"), "{text}");
        let remove = SetupArgs {
            uninstall: true,
            ..SetupArgs::default()
        };
        let (text, _) = run_with(remove, &env);
        assert!(
            text.contains("removed") && text.contains("config.toml.monarch-backup-1"),
            "{text}"
        );
        assert!(text.contains("  removed    skill "), "{text}");
        assert!(!env.agents.join("skills/monarch").exists(), "skill removed");
        assert_eq!(
            fs::read_to_string(env.codex.join("config.toml")).expect("file"),
            ""
        );
    }

    #[test]
    fn a_broken_file_fails_that_agent_only() {
        let home = tempfile::tempdir().expect("dir");
        let env = env(home.path());
        fs::create_dir(&env.claude).expect("claude dir");
        fs::create_dir(&env.codex).expect("codex dir");
        fs::write(env.claude.join("settings.json"), "{ broken").expect("write");
        let (text, code) = run_with(SetupArgs::default(), &env);
        assert_eq!(code, ExitCode::FAILURE);
        assert!(text.contains("settings.json: not valid JSON"), "{text}");
        assert!(text.contains("Codex ("), "Codex still set up: {text}");
    }

    #[test]
    fn bare_monarch_only_when_path_finds_this_binary() {
        let dir = tempfile::tempdir().expect("dir");
        let exe = dir.path().join("monarch");
        fs::write(&exe, "").expect("write");
        let exe = exe.canonicalize().expect("canonical");
        let path = std::env::join_paths([dir.path()]).expect("path");
        assert_eq!(program(&exe, Some(&path)), PathBuf::from("monarch"));
        let other = Path::new("/elsewhere/monarch");
        assert_eq!(program(other, Some(&path)), other);
        assert_eq!(program(&exe, None), exe);
    }
}
