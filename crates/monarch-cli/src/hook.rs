//! Turning agents' hook calls into `agent.report` requests.

mod api_client;
mod claude_hook;
mod tool_activity;

pub(crate) use api_client::ApiClient;
pub(crate) use claude_hook::ClaudeHook;
