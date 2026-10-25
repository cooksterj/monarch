//! Checks of what Monarch installs or reads: its config, the agents'
//! hooks and the Claude Code skill.

use std::path::Path;

use monarch_core::LoadedConfig;

use super::super::config::warnings;
use super::super::setup::plan;
use super::{DoctorEnv, Finding, Status};
use crate::HookCommand;
use crate::setup::Change;

/// The config file: loads, and without warnings.
pub(super) fn config(loaded: Result<&LoadedConfig, String>) -> Finding {
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(e) => {
            return Finding::new(Status::Fail, "config", format!("doesn't load: {e}"))
                .fix("fix the file, or move it aside to use the defaults");
        }
    };
    let path = loaded.path.display();
    if !loaded.found {
        let dir = loaded.path.parent().unwrap_or(&loaded.path).display();
        return Finding::new(
            Status::Note,
            "config",
            format!("none at {path}; using defaults"),
        )
        .fix(format!(
            "to start one: mkdir -p {dir} && monarch config default > {path}"
        ));
    }
    match warnings(loaded).as_slice() {
        [] => Finding::new(Status::Ok, "config", path.to_string()),
        [one] => Finding::new(Status::Warn, "config", one.clone()).fix("monarch config check"),
        many => Finding::new(
            Status::Warn,
            "config",
            format!("{} warnings in {path}", many.len()),
        )
        .fix("monarch config check"),
    }
}

/// Each agent's hooks: what `monarch setup` would still change.
pub(super) fn hooks(env: &DoctorEnv) -> Vec<Finding> {
    HookCommand::ALL
        .into_iter()
        .map(|agent| hook(agent, env))
        .collect()
}

fn hook(agent: HookCommand, env: &DoctorEnv) -> Finding {
    let topic = agent.title();
    let file = env.setup.files(agent).1;
    match plan(agent, &env.setup) {
        // Only the path: a parser's message can quote the file, which may
        // hold secrets.
        Err(_) => Finding::new(
            Status::Fail,
            topic,
            format!("{} can't be read", file.display()),
        )
        .fix("monarch setup --dry-run names the problem"),
        Ok(None) => Finding::new(Status::Note, topic, "not installed"),
        Ok(Some(changes)) => hook_changes(topic, &file, &changes),
    }
}

fn hook_changes(topic: &str, file: &Path, changes: &[Change]) -> Finding {
    let pending = changes.iter().filter(|c| c.kind.edits()).count();
    let skipped = changes
        .iter()
        .filter(|c| !c.kind.edits() && !matches!(c.kind, crate::setup::ChangeKind::Unchanged))
        .count();
    if pending > 0 {
        let text = format!(
            "{pending} hooks missing or out of date in {}",
            file.display()
        );
        return Finding::new(Status::Fail, topic, text)
            .fix("monarch setup (its state won't show in Monarch until then)");
    }
    if skipped > 0 {
        let text = format!(
            "{skipped} events in {} have a shape setup can't edit",
            file.display()
        );
        return Finding::new(Status::Warn, topic, text).fix("monarch setup --dry-run lists them");
    }
    Finding::new(
        Status::Ok,
        topic,
        format!("hooks installed ({} events)", changes.len()),
    )
}

/// The skill that teaches Claude Code to drive Monarch (optional).
pub(super) fn skill(env: &DoctorEnv) -> Finding {
    let dir = env.setup.claude.join("skills/monarch");
    if dir.join("SKILL.md").is_file() {
        return Finding::new(
            Status::Ok,
            "skill",
            format!("installed at {}", dir.display()),
        );
    }
    Finding::new(
        Status::Note,
        "skill",
        "not installed (optional: lets agents use Monarch)",
    )
    .fix(format!(
        "copy skills/monarch/SKILL.md from the repository to {}",
        dir.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::commands::setup::SetupEnv;

    fn env(home: &Path) -> DoctorEnv {
        DoctorEnv {
            setup: SetupEnv {
                claude: home.join(".claude"),
                codex: home.join(".codex"),
                program: "/opt/monarch".into(),
                stamp: 0,
            },
            vars: BTreeMap::new(),
            path: Vec::new(),
            git: None,
            sessions: Vec::new(),
        }
    }

    #[test]
    fn hooks_are_missing_then_installed() {
        let home = tempfile::tempdir().expect("dir");
        let env = env(home.path());
        std::fs::create_dir(&env.setup.claude).expect("dir");
        let found = hooks(&env);
        assert_eq!(found[0].status, Status::Fail, "{found:?}");
        assert_eq!(found[1].status, Status::Note, "codex isn't installed");
        let args = crate::SetupArgs::default();
        crate::commands::setup::run(args, &env.setup, &mut Vec::new()).expect("setup");
        let found = hooks(&env);
        assert_eq!(found[0].status, Status::Ok, "{found:?}");
        assert!(found[0].text.starts_with("hooks installed"));
    }

    #[test]
    fn unreadable_settings_fail_without_quoting_them() {
        let home = tempfile::tempdir().expect("dir");
        let env = env(home.path());
        std::fs::create_dir(&env.setup.claude).expect("dir");
        let secret = "{\"env\": {\"TOKEN\": \"hunter2\"";
        std::fs::write(env.setup.claude.join("settings.json"), secret).expect("write");
        let found = &hooks(&env)[0];
        assert_eq!(found.status, Status::Fail);
        assert!(!format!("{found:?}").contains("hunter2"), "{found:?}");
    }

    #[test]
    fn config_findings_follow_the_file() {
        let loaded = LoadedConfig {
            config: monarch_core::Config::default(),
            path: "/u/config.toml".into(),
            found: false,
            unknown_keys: Vec::new(),
        };
        assert_eq!(config(Ok(&loaded)).status, Status::Note);
        let found = LoadedConfig {
            found: true,
            unknown_keys: vec!["ui.x".into()],
            ..loaded
        };
        assert_eq!(config(Ok(&found)).status, Status::Warn);
        assert_eq!(config(Err("bad".into())).status, Status::Fail);
        assert_eq!(skill(&env(Path::new("/nonexistent"))).status, Status::Note);
    }
}
