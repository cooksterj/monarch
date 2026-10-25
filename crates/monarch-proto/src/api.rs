//! The public JSON-lines API on the session socket.
//!
//! After a `{"monarch":1,"proto":"json",…}` hello, each line is one
//! [`Request`] answered by one [`Response`] with the same `id`. Methods
//! are named `namespace.verb`; their params are typed here (only
//! [`AgentReport`] and [`NewWorktree`] so far).

mod agent_report;
mod new_worktree;
mod request;
mod response;
mod rpc_error;
mod worktree_created;

pub use agent_report::AgentReport;
pub use new_worktree::NewWorktree;
pub use request::Request;
pub use response::Response;
pub use rpc_error::RpcError;
pub use worktree_created::WorktreeCreated;
