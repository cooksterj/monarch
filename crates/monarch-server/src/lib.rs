//! Monarch session server: owns PTYs, emulators and attached clients.
//!
//! [`Server`] listens on the session socket. Each pane is a
//! [`PaneRuntime`] (PTY reader and writer threads around a shared
//! emulator); [`SessionState`] holds panes, clients and layout behind one
//! mutex; the [`FrameCoalescer`] task renders visible dirty panes into
//! per-client snapshots and deltas at most once per frame budget.

mod attach_connection;
mod cli_git_runner;
mod client_handle;
mod command_watcher;
mod daemon;
mod error;
mod event_loop;
mod frame_coalescer;
mod frame_encoder;
mod frame_header;
mod git_runner;
mod pane_agent;
mod pane_env;
mod pane_event;
mod pane_io;
mod pane_runtime;
mod pane_shared;
mod pane_spec;
mod persisted_pane;
mod persisted_session;
mod persister;
mod place_watcher;
mod placement;
mod process_probe;
mod repo_probe;
mod server;
mod server_config;
mod server_context;
mod session_state;
mod socket_path;
mod state_file;
mod system_probe;
#[cfg(test)]
mod test_support;

pub use attach_connection::AttachConnection;
pub use cli_git_runner::CliGitRunner;
pub use client_handle::{ClientHandle, RENDER_LANE};
pub use command_watcher::{COMMAND_INTERVAL, CommandWatcher};
pub use daemon::Daemon;
pub use error::{BoxedError, ServerError};
pub use event_loop::{EventLoop, MAX_CLIPBOARD};
pub use frame_coalescer::{FRAME_BUDGET, FrameCoalescer};
pub use frame_encoder::FrameEncoder;
pub use frame_header::FrameHeader;
pub use git_runner::GitRunner;
pub use pane_agent::PaneAgent;
pub use pane_env::PaneEnv;
pub use pane_event::PaneEvent;
pub use pane_io::PaneIo;
pub use pane_runtime::{PaneRuntime, SpawnParams};
pub use pane_shared::PaneShared;
pub use pane_spec::PaneSpec;
pub use persisted_pane::PersistedPane;
pub use persisted_session::PersistedSession;
pub use persister::{PERSIST_INTERVAL, Persister};
pub use place_watcher::{PLACE_INTERVAL, PlaceWatcher};
pub use placement::Placement;
pub use process_probe::ProcessProbe;
pub use repo_probe::RepoProbe;
pub use server::Server;
pub use server_config::ServerConfig;
pub use server_context::ServerContext;
pub use session_state::{ActionEffect, DEFAULT_SIZE, SessionState};
pub use socket_path::{MAX_SOCKET_PATH, SocketPath};
pub use state_file::StateFile;
pub use system_probe::SystemProbe;
