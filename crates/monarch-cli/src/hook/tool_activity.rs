//! A tool call as one short activity line (`Edit src/main.rs`).

use std::path::Path;

use serde_json::Value;

/// Names a tool call by its tool and the input that matters most.
pub(crate) struct ToolActivity<'a> {
    /// The tool (`Edit`, `Bash`, `mcp__github__create_pr`).
    pub tool: &'a str,
    /// Its input as the hook received it.
    pub input: Option<&'a Value>,
    /// The agent's directory, which file paths are shown relative to.
    pub cwd: Option<&'a str>,
}

impl ToolActivity<'_> {
    /// `<tool> <detail>`, or just the tool when nothing stands out;
    /// Codex's `apply_patch` reads as `Edit <file>`.
    pub fn line(&self) -> String {
        let tool = if self.tool == "apply_patch" {
            "Edit"
        } else {
            self.tool
        };
        match self.detail() {
            Some(detail) => format!("{tool} {detail}"),
            None => tool.to_owned(),
        }
    }

    fn detail(&self) -> Option<String> {
        let field = |key: &str| self.input?.get(key)?.as_str();
        if self.tool == "apply_patch" {
            return field("command")
                .and_then(patched_file)
                .map(|p| self.relative(p));
        }
        let text = match self.tool {
            "Edit" | "MultiEdit" | "Write" | "Read" => {
                return field("file_path").map(|p| self.relative(p));
            }
            "NotebookEdit" => return field("notebook_path").map(|p| self.relative(p)),
            "Bash" => field("command"),
            "Grep" | "Glob" => field("pattern"),
            "WebFetch" => field("url"),
            "WebSearch" => field("query"),
            "Task" | "Agent" => field("description"),
            _ => None,
        };
        text.map(str::to_owned)
    }

    /// `path` below the agent's directory, else as given.
    fn relative(&self, path: &str) -> String {
        self.cwd
            .and_then(|cwd| Path::new(path).strip_prefix(cwd).ok())
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| path.to_owned(), |p| p.to_string_lossy().into_owned())
    }
}

/// The first file a Codex patch touches (`*** Update File: src/x.rs`).
fn patched_file(patch: &str) -> Option<&str> {
    patch.lines().find_map(|line| {
        ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
            .iter()
            .find_map(|head| line.strip_prefix(head))
            .map(str::trim)
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn line(tool: &str, input: &Value) -> String {
        ToolActivity {
            tool,
            input: Some(input),
            cwd: Some("/w/api"),
        }
        .line()
    }

    #[test]
    fn files_are_shown_below_the_agents_directory() {
        assert_eq!(
            line("Edit", &json!({"file_path": "/w/api/src/x.rs"})),
            "Edit src/x.rs"
        );
        assert_eq!(
            line("Read", &json!({"file_path": "/etc/hosts"})),
            "Read /etc/hosts"
        );
        assert_eq!(
            line("Write", &json!({"file_path": "/w/api"})),
            "Write /w/api"
        );
    }

    #[test]
    fn known_tools_show_their_main_input() {
        assert_eq!(
            line("Bash", &json!({"command": "cargo test"})),
            "Bash cargo test"
        );
        assert_eq!(line("Grep", &json!({"pattern": "fn main"})), "Grep fn main");
        assert_eq!(
            line("Task", &json!({"description": "Find uses"})),
            "Task Find uses"
        );
    }

    #[test]
    fn codex_patches_name_their_first_file() {
        let patch = json!({"command": "*** Begin Patch\n*** Add File: /w/api/new.rs\n+x\n"});
        assert_eq!(line("apply_patch", &patch), "Edit new.rs");
        assert_eq!(line("apply_patch", &json!({"command": "garbage"})), "Edit");
    }

    #[test]
    fn other_tools_or_missing_input_show_the_tool() {
        assert_eq!(line("mcp__gh__pr", &json!({"title": "x"})), "mcp__gh__pr");
        assert_eq!(line("Bash", &json!({"command": 3})), "Bash");
        let bare = ToolActivity {
            tool: "Edit",
            input: None,
            cwd: None,
        };
        assert_eq!(bare.line(), "Edit");
    }
}
