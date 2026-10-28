//! What the help page says: the keymap's bindings by area, then the mouse,
//! pop-ups and the command line.

use monarch_core::{Action, ActionGroup, Keymap};

use crate::{HelpLine, WhichKeyEntry};

/// Every line, from the keymap in use.
pub(super) fn help_lines(keymap: &Keymap) -> Vec<HelpLine> {
    let entries = WhichKeyEntry::from_keymap(keymap);
    let mut lines = Vec::new();
    for group in ActionGroup::ALL {
        lines.push(HelpLine::Heading(group.title().into()));
        lines.extend(bound(&entries, group));
        lines.extend(unbound(keymap, group));
    }
    section(&mut lines, "Mouse", &MOUSE);
    section(&mut lines, "In pop-ups", &POPUPS);
    section(&mut lines, "Command line", &COMMANDS);
    lines
}

/// The group's bound keys, in the key popup's order.
fn bound(entries: &[WhichKeyEntry], group: ActionGroup) -> impl Iterator<Item = HelpLine> + '_ {
    entries
        .iter()
        .filter(move |e| e.action.group() == group)
        .map(|e| HelpLine::entry(e.keys.clone(), e.action.summary()))
}

/// The group's actions with no key, named so they can be bound (or run
/// from the palette). Directions share a line.
fn unbound(keymap: &Keymap, group: ActionGroup) -> Vec<HelpLine> {
    let mut seen: Vec<&str> = keymap.bindings().map(|(_, a)| a.summary()).collect();
    let mut lines = Vec::new();
    for action in Action::catalogue() {
        let summary = action.summary();
        if action.group() != group || seen.contains(&summary) {
            continue;
        }
        seen.push(summary);
        lines.push(HelpLine::entry(
            "",
            format!("{summary} (unbound: {action})"),
        ));
    }
    lines
}

fn section(lines: &mut Vec<HelpLine>, title: &str, entries: &[(&str, &str)]) {
    lines.push(HelpLine::Heading(title.into()));
    lines.extend(entries.iter().map(|(k, t)| HelpLine::entry(*k, *t)));
}

const MOUSE: [(&str, &str); 7] = [
    ("click", "focus a pane, here or in the sidebar"),
    ("drag", "select text; it is copied when you let go"),
    ("double-click", "select a word"),
    ("triple-click", "select a line"),
    ("wheel", "scroll the pane's history, or the sidebar"),
    ("right-click", "pane and tab menu"),
    ("shift+drag", "select even where the program uses the mouse"),
];

const POPUPS: [(&str, &str); 4] = [
    ("type", "search"),
    ("↑ ↓", "move"),
    ("enter", "choose"),
    ("esc", "close"),
];

const COMMANDS: [(&str, &str); 9] = [
    (
        "monarch SESSION",
        "attach to a session, starting it if needed",
    ),
    ("monarch ls", "running sessions"),
    (
        "monarch new BRANCH",
        "worktree and tab (--agent claude|codex)",
    ),
    ("monarch agent explain", "each pane's agent state and why"),
    ("monarch agent wait PANE", "wait until an agent needs you"),
    (
        "monarch pane send PANE TEXT",
        "type into a pane (--enter submits)",
    ),
    ("monarch config check", "check your config file"),
    ("monarch setup", "install the agent hooks and skill"),
    ("monarch doctor", "check the setup, with fixes"),
];

#[cfg(test)]
mod tests {
    use monarch_core::KeymapPreset;

    use super::*;

    fn text(lines: &[HelpLine]) -> String {
        lines
            .iter()
            .map(|l| match l {
                HelpLine::Heading(t) => format!("# {t}"),
                HelpLine::Entry { keys, text } => format!("{keys} = {text}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn bindings_sit_under_their_area() {
        let all = text(&help_lines(&Keymap::default()));
        let agents = all.find("# Agents").expect("agents");
        let inbox = all.find("a = agents waiting for you").expect("inbox");
        let projects = all.find("# Projects").expect("projects");
        assert!(agents < inbox && inbox < projects, "{all}");
        assert!(all.contains("hjkl ←↓↑→ = focus the pane in that direction"));
        assert!(all.contains("? = this help"));
        assert!(all.contains("# Command line"));
        assert!(all.contains("(unbound: focus-prev)"), "{all}");
        assert!(!all.contains("(unbound: zoom)"));
    }

    #[test]
    fn unbound_actions_are_named_once() {
        let tmux = text(&help_lines(&Keymap::preset(KeymapPreset::Tmux)));
        assert_eq!(tmux.matches("move the divider that way").count(), 1);
        assert!(tmux.contains("(unbound: resize-left)"), "{tmux}");
        assert!(tmux.contains("(unbound: equalize)"));
    }
}
