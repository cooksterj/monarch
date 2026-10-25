//! `monarch setup`'s building blocks: the hook command Monarch installs,
//! the two agents' config files it edits, and what changed.

mod change;
mod change_kind;
mod claude_settings;
mod codex_config;
mod config_file;
mod hook_config;
mod hook_line;
mod hook_slot;

pub(crate) use change::Change;
pub(crate) use change_kind::ChangeKind;
pub(crate) use claude_settings::ClaudeSettings;
pub(crate) use codex_config::CodexConfig;
pub(crate) use config_file::ConfigFile;
pub(crate) use hook_config::HookConfig;
pub(crate) use hook_line::HookLine;
pub(crate) use hook_slot::HookSlot;
