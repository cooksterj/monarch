//! Keeps `crates/monarch-cli/skills/monarch/SKILL.md` true to the CLI: its front matter is
//! complete, and every `monarch …` command it shows still parses.

use std::path::Path;

use clap::{CommandFactory, Parser};
use monarch_cli::Cli;

fn skill() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/monarch/SKILL.md");
    std::fs::read_to_string(path).expect("skills/monarch/SKILL.md")
}

#[test]
fn front_matter_names_and_describes_the_skill() {
    let text = skill();
    let front = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(front, _)| front)
        .expect("front matter");
    assert!(front.lines().any(|l| l == "name: monarch"));
    let description = front
        .lines()
        .find_map(|l| l.strip_prefix("description: "))
        .expect("a description");
    assert!(description.len() <= 1024, "Claude Code caps descriptions");
    assert!(description.contains("MONARCH_PANE"), "says when it applies");
}

#[test]
fn every_command_shown_parses() {
    let text = skill();
    let mut in_sh = false;
    let mut shown = 0;
    for line in text.lines() {
        match line.trim() {
            "```sh" => in_sh = true,
            "```" => in_sh = false,
            command if in_sh && command.starts_with("monarch ") => {
                let words = command.split_whitespace();
                Cli::try_parse_from(words).unwrap_or_else(|e| panic!("{command}: {e}"));
                shown += 1;
            }
            _ => {}
        }
    }
    assert!(shown >= 5, "found only {shown} commands");
}

#[test]
fn every_command_named_exists() {
    let text = skill();
    let cli = Cli::command();
    let named = text.split('`').skip(1).step_by(2).filter_map(|code| {
        let rest = code.strip_prefix("monarch ")?;
        rest.split_whitespace().next()
    });
    for name in named {
        assert!(cli.find_subcommand(name).is_some(), "no `monarch {name}`");
    }
}
