//! Turning agents' hook calls into `agent.report` requests.

mod agent_hook;
mod api_client;
mod tool_activity;

pub(crate) use agent_hook::AgentHook;
pub(crate) use api_client::ApiClient;
