//! Claude Code's `settings.json`, edited as JSON with its key order kept
//! so only the hooks change.

use anyhow::{Context, bail};
use serde_json::{Map, Value, json};

use super::{HookConfig, HookSlot};

/// The parsed settings file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClaudeSettings {
    root: Map<String, Value>,
    newline: bool,
}

impl ClaudeSettings {
    /// Parses `text`; an empty or missing file is an empty object.
    pub fn parse(text: Option<&str>) -> anyhow::Result<Self> {
        let text = text.unwrap_or_default();
        if text.trim().is_empty() {
            return Ok(Self {
                root: Map::new(),
                newline: true,
            });
        }
        let Value::Object(root) = serde_json::from_str(text).context("not valid JSON")? else {
            bail!("expected a JSON object");
        };
        Ok(Self {
            root,
            newline: text.ends_with('\n'),
        })
    }

    fn groups(&self, event: &str) -> Option<&Vec<Value>> {
        self.root.get("hooks")?.get(event)?.as_array()
    }

    fn groups_mut(&mut self, event: &str) -> Option<&mut Vec<Value>> {
        self.root.get_mut("hooks")?.get_mut(event)?.as_array_mut()
    }

    fn hooks_mut(&mut self, event: &str, group: usize) -> Option<&mut Vec<Value>> {
        self.groups_mut(event)?
            .get_mut(group)?
            .get_mut("hooks")?
            .as_array_mut()
    }
}

impl HookConfig for ClaudeSettings {
    fn events(&self) -> Vec<String> {
        let hooks = self.root.get("hooks").and_then(Value::as_object);
        hooks.into_iter().flat_map(|h| h.keys().cloned()).collect()
    }

    fn slots(&self, event: &str) -> Vec<HookSlot> {
        let mut slots = Vec::new();
        for (g, group) in self.groups(event).into_iter().flatten().enumerate() {
            let hooks = group.get("hooks").and_then(Value::as_array);
            for (i, hook) in hooks.into_iter().flatten().enumerate() {
                if let Some(command) = hook.get("command").and_then(Value::as_str) {
                    slots.push(HookSlot::new(g, i, command));
                }
            }
        }
        slots
    }

    fn editable(&self, event: &str) -> bool {
        match self.root.get("hooks") {
            None => true,
            Some(Value::Object(hooks)) => hooks.get(event).is_none_or(Value::is_array),
            Some(_) => false,
        }
    }

    fn set_command(&mut self, event: &str, slot: &HookSlot, command: &str) {
        let hook = self
            .hooks_mut(event, slot.group)
            .and_then(|hooks| hooks.get_mut(slot.index));
        if let Some(Value::Object(hook)) = hook {
            hook.insert("command".into(), command.into());
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
        let Some(Value::Object(all)) = self.root.get_mut("hooks") else {
            return;
        };
        if all
            .get(event)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            all.shift_remove(event);
        }
        if all.is_empty() {
            self.root.shift_remove("hooks");
        }
    }

    fn append(&mut self, event: &str, command: &str, timeout: u32) {
        let group =
            json!({ "hooks": [{ "type": "command", "command": command, "timeout": timeout }] });
        let hooks = self.root.entry("hooks").or_insert_with(|| json!({}));
        if let Some(hooks) = hooks.as_object_mut() {
            let groups = hooks.entry(event).or_insert_with(|| json!([]));
            if let Some(groups) = groups.as_array_mut() {
                groups.push(group);
            }
        }
    }

    fn render(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.root).unwrap_or_default();
        if self.newline {
            text.push('\n');
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::HookCommand;
    use crate::setup::{ChangeKind, HookLine};

    const MINE: &str = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "say done"
          },
          {
            "type": "command",
            "command": "/old/monarch hook claude",
            "timeout": 9
          }
        ]
      }
    ]
  },
  "tui": "default"
}
"#;

    fn line() -> HookLine {
        HookLine::new(Path::new("monarch"), HookCommand::Claude)
    }

    #[test]
    fn install_keeps_order_and_other_hooks() {
        let mut settings = ClaudeSettings::parse(Some(MINE)).expect("parses");
        let changes = settings.install(&line());
        let stop = changes.iter().find(|c| c.event == "Stop").expect("stop");
        assert_eq!(stop.kind, ChangeKind::Updated);
        let text = settings.render();
        assert!(
            text.find("\"model\"") < text.find("\"hooks\""),
            "order kept"
        );
        assert!(text.find("\"hooks\"") < text.find("\"tui\""));
        assert!(text.contains("\"command\": \"say done\""));
        assert!(text.contains("\"command\": \"monarch hook claude\",\n            \"timeout\": 9"));
        assert!(text.contains("\"SessionEnd\""));
        let reparsed = ClaudeSettings::parse(Some(&text)).expect("valid");
        assert_eq!(
            reparsed.slots("SessionStart"),
            [HookSlot::new(0, 0, "monarch hook claude")]
        );
    }

    #[test]
    fn unchanged_files_render_byte_for_byte() {
        let settings = ClaudeSettings::parse(Some(MINE)).expect("parses");
        assert_eq!(settings.render(), MINE);
    }

    #[test]
    fn uninstall_leaves_the_rest() {
        let mut settings = ClaudeSettings::parse(Some(MINE)).expect("parses");
        settings.uninstall(HookCommand::Claude);
        let text = settings.render();
        assert!(!text.contains("monarch"));
        assert!(text.contains("say done"));
        let mut empty = ClaudeSettings::parse(None).expect("empty");
        empty.install(&line());
        empty.uninstall(HookCommand::Claude);
        assert_eq!(empty.render(), "{}\n", "hooks key removed once empty");
    }

    #[test]
    fn bad_files_are_refused_or_skipped() {
        assert!(ClaudeSettings::parse(Some("[1]")).is_err());
        assert!(ClaudeSettings::parse(Some("{ nope")).is_err());
        let mut odd = ClaudeSettings::parse(Some(r#"{"hooks": {"Stop": "x"}}"#)).expect("parses");
        let changes = odd.install(&line());
        let stop = changes.iter().find(|c| c.event == "Stop").expect("stop");
        assert_eq!(stop.kind, ChangeKind::Skipped);
        assert!(odd.render().contains("\"Stop\": \"x\""));
    }
}
