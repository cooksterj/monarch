//! Codex's `config.toml`, edited with `toml_edit` so comments, layout and
//! everything outside the hooks (including `[hooks.state]`) stay as they
//! were.

use anyhow::Context;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::{HookConfig, HookSlot};

/// The parsed config file.
#[derive(Debug, Clone)]
pub(crate) struct CodexConfig {
    doc: DocumentMut,
}

impl CodexConfig {
    /// Parses `text`; a missing file is an empty document.
    pub fn parse(text: Option<&str>) -> anyhow::Result<Self> {
        let doc = text
            .unwrap_or_default()
            .parse::<DocumentMut>()
            .context("not valid TOML")?;
        Ok(Self { doc })
    }

    fn hooks(&self) -> Option<&Table> {
        self.doc.get("hooks")?.as_table()
    }

    fn groups(&self, event: &str) -> Option<&ArrayOfTables> {
        self.hooks()?.get(event)?.as_array_of_tables()
    }

    fn groups_mut(&mut self, event: &str) -> Option<&mut ArrayOfTables> {
        let hooks = self.doc.get_mut("hooks")?.as_table_mut()?;
        hooks.get_mut(event)?.as_array_of_tables_mut()
    }

    fn hooks_mut(&mut self, event: &str, group: usize) -> Option<&mut ArrayOfTables> {
        let group = self.groups_mut(event)?.get_mut(group)?;
        group.get_mut("hooks")?.as_array_of_tables_mut()
    }
}

impl HookConfig for CodexConfig {
    fn events(&self) -> Vec<String> {
        let events = self.hooks().into_iter().flat_map(Table::iter);
        events
            .filter(|(_, item)| item.is_array_of_tables())
            .map(|(event, _)| event.to_owned())
            .collect()
    }

    fn slots(&self, event: &str) -> Vec<HookSlot> {
        let mut slots = Vec::new();
        for (g, group) in self
            .groups(event)
            .into_iter()
            .flat_map(ArrayOfTables::iter)
            .enumerate()
        {
            let hooks = group.get("hooks").and_then(Item::as_array_of_tables);
            for (i, hook) in hooks.into_iter().flat_map(ArrayOfTables::iter).enumerate() {
                if let Some(command) = hook.get("command").and_then(Item::as_str) {
                    slots.push(HookSlot::new(g, i, command));
                }
            }
        }
        slots
    }

    fn editable(&self, event: &str) -> bool {
        match self.doc.get("hooks") {
            None => true,
            Some(Item::Table(hooks)) => hooks.get(event).is_none_or(Item::is_array_of_tables),
            Some(_) => false,
        }
    }

    fn set_command(&mut self, event: &str, slot: &HookSlot, command: &str) {
        let hook = self
            .hooks_mut(event, slot.group)
            .and_then(|hooks| hooks.get_mut(slot.index));
        if let Some(hook) = hook {
            hook["command"] = value(command);
        }
    }

    fn remove(&mut self, event: &str, slot: &HookSlot) {
        let Some(hooks) = self.hooks_mut(event, slot.group) else {
            return;
        };
        if slot.index < hooks.len() {
            hooks.remove(slot.index);
        }
        if hooks.is_empty()
            && let Some(groups) = self.groups_mut(event)
        {
            groups.remove(slot.group);
        }
        let Some(all) = self.doc.get_mut("hooks").and_then(Item::as_table_mut) else {
            return;
        };
        if all
            .get(event)
            .and_then(Item::as_array_of_tables)
            .is_some_and(ArrayOfTables::is_empty)
        {
            all.remove(event);
        }
        if all.is_empty() {
            self.doc.remove("hooks");
        }
    }

    fn append(&mut self, event: &str, command: &str, timeout: u32) {
        let mut hook = Table::new();
        hook["type"] = value("command");
        hook["command"] = value(command);
        hook["timeout"] = value(i64::from(timeout));
        let mut inner = ArrayOfTables::new();
        inner.push(hook);
        let mut group = Table::new();
        group.insert("hooks", Item::ArrayOfTables(inner));
        let hooks = self.doc.entry("hooks").or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        });
        let Some(hooks) = hooks.as_table_mut() else {
            return;
        };
        let groups = hooks
            .entry(event)
            .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()));
        if let Some(groups) = groups.as_array_of_tables_mut() {
            groups.push(group);
        }
    }

    fn render(&self) -> String {
        self.doc.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::HookCommand;
    use crate::setup::{ChangeKind, HookLine};

    const MINE: &str = r#"model = "gpt-5" # keep me

# Monarch: report agent state.
[[hooks.Stop]]

[[hooks.Stop.hooks]]
type = "command"
command = "/old/monarch hook codex"
timeout = 5

[hooks.state."/c.toml:stop:0:0"]
trusted_hash = "sha256:abc"

[tui]
theme = "dark"
"#;

    fn line() -> HookLine {
        HookLine::new(Path::new("monarch"), HookCommand::Codex)
    }

    #[test]
    fn install_edits_in_place_and_keeps_the_rest() {
        let mut config = CodexConfig::parse(Some(MINE)).expect("parses");
        let changes = config.install(&line());
        let kind = |e: &str| changes.iter().find(|c| c.event == e).map(|c| c.kind);
        assert_eq!(kind("Stop"), Some(ChangeKind::Updated));
        assert_eq!(kind("Interrupt"), Some(ChangeKind::Added));
        let text = config.render();
        assert!(text.starts_with(
            "model = \"gpt-5\" # keep me\n\n# Monarch: report agent state.\n[[hooks.Stop]]"
        ));
        assert!(text.contains("command = \"monarch hook codex\"\ntimeout = 5"));
        assert!(text.contains("trusted_hash = \"sha256:abc\""));
        assert!(text.contains("[tui]\ntheme = \"dark\""));
        let reparsed = CodexConfig::parse(Some(&text)).expect("valid TOML");
        assert_eq!(
            reparsed.slots("Interrupt"),
            [HookSlot::new(0, 0, "monarch hook codex")]
        );
        assert!(reparsed.render().contains("timeout = 3"), "Codex's cap");
        assert!(!reparsed.events().contains(&"state".to_owned()));
    }

    #[test]
    fn untouched_files_render_byte_for_byte() {
        let mut config = CodexConfig::parse(Some(MINE)).expect("parses");
        config.uninstall(HookCommand::Claude);
        assert_eq!(config.render(), MINE);
    }

    #[test]
    fn uninstall_keeps_trust_state_and_other_tables() {
        let mut config = CodexConfig::parse(Some(MINE)).expect("parses");
        let removed = config.uninstall(HookCommand::Codex);
        assert_eq!(removed.len(), 1);
        let text = config.render();
        assert!(!text.contains("[[hooks.Stop"));
        assert!(text.contains("trusted_hash") && text.contains("[tui]"));
    }

    #[test]
    fn fresh_and_odd_files() {
        let mut fresh = CodexConfig::parse(None).expect("empty");
        fresh.install(&line());
        let text = fresh.render();
        assert!(text.contains("[[hooks.SessionStart]]\n\n[[hooks.SessionStart.hooks]]"));
        assert!(CodexConfig::parse(Some("x = ")).is_err());
        let mut inline = CodexConfig::parse(Some("hooks = { Stop = 1 }\n")).expect("parses");
        let changes = inline.install(&line());
        assert!(changes.iter().all(|c| c.kind == ChangeKind::Skipped));
        assert_eq!(inline.render(), "hooks = { Stop = 1 }\n");
    }
}
