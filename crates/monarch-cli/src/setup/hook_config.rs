//! The hooks section both agents share (event → matcher groups → hooks),
//! and the install/uninstall rules, written once over it.

use super::{Change, ChangeKind, HookLine, HookSlot};
use crate::HookCommand;

/// An agent's hooks, editable in place. Implementors provide the
/// primitive edits; installing and uninstalling are shared.
pub(crate) trait HookConfig {
    /// Every event with hooks, in file order.
    fn events(&self) -> Vec<String>;

    /// Every hook command under `event`, in file order.
    fn slots(&self, event: &str) -> Vec<HookSlot>;

    /// Whether `event`'s entry is absent or has a shape this can edit.
    fn editable(&self, event: &str) -> bool;

    /// Replaces the command at `slot`.
    fn set_command(&mut self, event: &str, slot: &HookSlot, command: &str);

    /// Removes the hook at `slot`, then its group and event if emptied.
    fn remove(&mut self, event: &str, slot: &HookSlot);

    /// Adds a group under `event` holding one command hook.
    fn append(&mut self, event: &str, command: &str, timeout: u32);

    /// The file's new text.
    fn render(&self) -> String;

    /// Makes every event of `line`'s agent run `line` exactly once,
    /// keeping a correct hook as it is (agents may have trusted it), and
    /// removes the agent's Monarch hooks from events it no longer uses.
    fn install(&mut self, line: &HookLine) -> Vec<Change> {
        let agent = line.agent();
        let mut changes: Vec<Change> = agent
            .events()
            .iter()
            .map(|event| Change::new(*event, self.install_event(event, line)))
            .collect();
        for event in self.events() {
            if !agent.events().contains(&event.as_str()) && self.drop_monarch(&event, agent, 0) {
                changes.push(Change::new(event, ChangeKind::Removed));
            }
        }
        changes
    }

    /// Removes every Monarch hook of `agent`.
    fn uninstall(&mut self, agent: HookCommand) -> Vec<Change> {
        self.events()
            .into_iter()
            .filter(|event| self.drop_monarch(event, agent, 0))
            .map(|event| Change::new(event, ChangeKind::Removed))
            .collect()
    }

    /// One event's part of [`Self::install`].
    fn install_event(&mut self, event: &str, line: &HookLine) -> ChangeKind {
        if !self.editable(event) {
            return ChangeKind::Skipped;
        }
        let mine = monarch_slots(self, event, line.agent());
        let Some(first) = mine.first() else {
            self.append(event, line.text(), line.agent().timeout(event));
            return ChangeKind::Added;
        };
        let duplicates = self.drop_monarch(event, line.agent(), 1);
        if first.command != line.text() {
            self.set_command(event, first, line.text());
            return ChangeKind::Updated;
        }
        if duplicates {
            ChangeKind::Updated
        } else {
            ChangeKind::Unchanged
        }
    }

    /// Removes `agent`'s Monarch hooks under `event` after the first
    /// `keep`; whether any went.
    fn drop_monarch(&mut self, event: &str, agent: HookCommand, keep: usize) -> bool {
        let mut mine = monarch_slots(self, event, agent);
        let doomed = mine.split_off(keep.min(mine.len()));
        // Last first, so earlier positions stay valid.
        for slot in doomed.iter().rev() {
            self.remove(event, slot);
        }
        !doomed.is_empty()
    }
}

/// `agent`'s Monarch hooks under `event`.
fn monarch_slots<C: HookConfig + ?Sized>(
    config: &C,
    event: &str,
    agent: HookCommand,
) -> Vec<HookSlot> {
    config
        .slots(event)
        .into_iter()
        .filter(|s| HookLine::is_monarch(&s.command, agent))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Events → groups → commands, in memory.
    #[derive(Default)]
    struct Fake(Vec<(String, Vec<Vec<String>>)>);

    impl Fake {
        fn groups(&mut self, event: &str) -> Option<&mut Vec<Vec<String>>> {
            self.0.iter_mut().find(|(e, _)| e == event).map(|(_, g)| g)
        }
    }

    impl HookConfig for Fake {
        fn events(&self) -> Vec<String> {
            self.0.iter().map(|(e, _)| e.clone()).collect()
        }
        fn slots(&self, event: &str) -> Vec<HookSlot> {
            let groups = self.0.iter().find(|(e, _)| e == event).map(|(_, g)| g);
            groups
                .into_iter()
                .flatten()
                .enumerate()
                .flat_map(|(g, hooks)| {
                    hooks
                        .iter()
                        .enumerate()
                        .map(move |(i, c)| HookSlot::new(g, i, c.as_str()))
                })
                .collect()
        }
        fn editable(&self, event: &str) -> bool {
            event != "Weird"
        }
        fn set_command(&mut self, event: &str, slot: &HookSlot, command: &str) {
            self.groups(event).expect("event")[slot.group][slot.index] = command.into();
        }
        fn remove(&mut self, event: &str, slot: &HookSlot) {
            let groups = self.groups(event).expect("event");
            groups[slot.group].remove(slot.index);
            if groups[slot.group].is_empty() {
                groups.remove(slot.group);
            }
            self.0.retain(|(_, g)| !g.is_empty());
        }
        fn append(&mut self, event: &str, command: &str, _timeout: u32) {
            match self.groups(event) {
                Some(groups) => groups.push(vec![command.into()]),
                None => self.0.push((event.into(), vec![vec![command.into()]])),
            }
        }
        fn render(&self) -> String {
            format!("{:?}", self.0)
        }
    }

    fn line(path: &str) -> HookLine {
        HookLine::new(Path::new(path), HookCommand::Codex)
    }

    fn kinds(changes: &[Change]) -> Vec<(&str, ChangeKind)> {
        changes.iter().map(|c| (c.event.as_str(), c.kind)).collect()
    }

    #[test]
    fn install_adds_every_event_once_and_is_idempotent() {
        let mut config = Fake::default();
        let first = config.install(&line("monarch"));
        assert!(first.iter().all(|c| c.kind == ChangeKind::Added));
        assert_eq!(first.len(), HookCommand::Codex.events().len());
        let again = config.install(&line("monarch"));
        assert!(again.iter().all(|c| c.kind == ChangeKind::Unchanged));
        assert_eq!(
            config.slots("Stop"),
            [HookSlot::new(0, 0, "monarch hook codex")]
        );
    }

    #[test]
    fn install_fixes_paths_drops_duplicates_and_keeps_others() {
        let mut config = Fake(vec![
            (
                "Stop".into(),
                vec![vec!["say done".into(), "/old/monarch hook codex".into()]],
            ),
            (
                "PreToolUse".into(),
                vec![
                    vec!["monarch hook codex".into()],
                    vec!["monarch hook codex".into()],
                ],
            ),
            (
                "Legacy".into(),
                vec![vec![
                    "monarch hook codex".into(),
                    "monarch hook claude".into(),
                ]],
            ),
            ("Weird".into(), vec![]),
        ]);
        let changes = config.install(&line("monarch"));
        let kinds = kinds(&changes);
        assert!(kinds.contains(&("Stop", ChangeKind::Updated)));
        assert!(
            kinds.contains(&("PreToolUse", ChangeKind::Updated)),
            "duplicate dropped"
        );
        assert!(kinds.contains(&("Legacy", ChangeKind::Removed)));
        assert_eq!(
            config.slots("Stop")[0].command,
            "say done",
            "others kept in place"
        );
        assert_eq!(config.slots("Stop")[1].command, "monarch hook codex");
        assert_eq!(config.slots("PreToolUse").len(), 1);
        assert_eq!(
            config.slots("Legacy"),
            [HookSlot::new(0, 0, "monarch hook claude")]
        );
    }

    #[test]
    fn uninstall_removes_only_that_agents_monarch_hooks() {
        let mut config = Fake(vec![
            (
                "Stop".into(),
                vec![vec!["say done".into()], vec!["monarch hook codex".into()]],
            ),
            ("Start".into(), vec![vec!["monarch hook codex".into()]]),
            ("Other".into(), vec![vec!["monarch hook claude".into()]]),
        ]);
        let changes = config.uninstall(HookCommand::Codex);
        assert_eq!(
            kinds(&changes),
            [
                ("Stop", ChangeKind::Removed),
                ("Start", ChangeKind::Removed)
            ]
        );
        assert_eq!(config.events(), ["Stop", "Other"], "emptied events go");
        assert!(
            config.uninstall(HookCommand::Codex).is_empty(),
            "nothing left"
        );
    }
}
