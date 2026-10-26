//! `monarch doctor`: checks the config, the agents' hooks, git, the
//! terminal and running sessions, and says how to fix what isn't right.

mod doctor_env;
mod finding;
mod install_checks;
mod status;
mod system_checks;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use monarch_core::LoadedConfig;

pub(crate) use doctor_env::DoctorEnv;
pub(crate) use finding::Finding;
pub(crate) use status::Status;

/// Every finding, in report order.
pub(crate) fn findings(
    env: &DoctorEnv,
    config: Result<&LoadedConfig, String>,
    exe: &Path,
) -> Vec<Finding> {
    let mut found = vec![
        system_checks::binary(env, exe),
        install_checks::config(config),
    ];
    found.extend(install_checks::hooks(env));
    found.extend(install_checks::skills(env));
    found.push(system_checks::agents(env));
    found.push(system_checks::git(env));
    found.extend(system_checks::terminal(env));
    found.extend(system_checks::colour(env));
    found.push(system_checks::sessions(env));
    found
}

/// Prints the report; fails when anything is broken.
pub(crate) fn run(findings: &[Finding], out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let width = findings
        .iter()
        .map(|f| f.topic.chars().count())
        .max()
        .unwrap_or(0);
    for f in findings {
        writeln!(out, "{} {:width$}  {}", f.status.mark(), f.topic, f.text)?;
        if let Some(fix) = f.fix.as_ref().filter(|_| f.status != Status::Ok) {
            writeln!(out, "  {:width$}  → {fix}", "")?;
        }
    }
    let count = |s: Status| findings.iter().filter(|f| f.status == s).count();
    let (failed, warned) = (count(Status::Fail), count(Status::Warn));
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let summary = match (failed, warned) {
        (0, 0) => "Everything Monarch needs is in place.".to_owned(),
        (0, w) => format!("Works, with {}.", plural(w, "warning")),
        (f, w) => format!("{} to fix, {}.", plural(f, "problem"), plural(w, "warning")),
    };
    writeln!(out, "\n{summary}")?;
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(findings: &[Finding]) -> (String, ExitCode) {
        let mut out = Vec::new();
        let code = run(findings, &mut out).expect("writes");
        (String::from_utf8(out).expect("utf8"), code)
    }

    #[test]
    fn the_report_lines_up_and_fails_on_problems() {
        let (text, code) = report(&[
            Finding::new(Status::Ok, "git", "git version 2.50").fix("never shown"),
            Finding::new(Status::Fail, "Codex", "3 hooks missing").fix("monarch setup"),
        ]);
        let expected = "✓ git    git version 2.50\n\
                        ✗ Codex  3 hooks missing\n\
                        \x20        → monarch setup\n\
                        \n\
                        1 problem to fix, 0 warnings.\n";
        assert_eq!(text, expected);
        assert_eq!(code, ExitCode::FAILURE);
        let (text, code) = report(&[Finding::new(Status::Warn, "colour", "no")]);
        assert!(text.ends_with("Works, with 1 warning.\n"), "{text}");
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
