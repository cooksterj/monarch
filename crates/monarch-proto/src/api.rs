//! The public JSON-lines API on the session socket.
//!
//! After a `{"monarch":1,"proto":"json",…}` hello, each line is one
//! [`Request`] answered by one [`Response`] with the same `id`. Methods
//! are named `namespace.verb`; their params and results are typed here.

mod agent_report;
mod explain_panes;
mod kill_session;
mod new_worktree;
mod pane_explanation;
mod request;
mod response;
mod rpc_error;
mod rule_outcome;
mod session_info;
mod status_source;
mod worktree_created;

pub use agent_report::AgentReport;
pub use explain_panes::ExplainPanes;
pub use kill_session::KillSession;
pub use new_worktree::NewWorktree;
pub use pane_explanation::PaneExplanation;
pub use request::Request;
pub use response::Response;
pub use rpc_error::RpcError;
pub use rule_outcome::RuleOutcome;
pub use session_info::SessionInfo;
pub use status_source::StatusSource;
pub use worktree_created::WorktreeCreated;
