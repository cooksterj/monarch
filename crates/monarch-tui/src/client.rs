//! Attaching to a session server: the socket link, a pure model of the
//! session built from frames, the raw-input thread and the main loop.

mod applied;
mod attach_app;
mod attach_options;
mod attach_outcome;
mod attach_state;
mod client_event;
mod input_thread;
mod pane_replica;
mod server_link;
mod shared_focus;

pub use applied::Applied;
pub use attach_app::AttachApp;
pub use attach_options::AttachOptions;
pub use attach_outcome::AttachOutcome;
pub use attach_state::AttachState;
pub use client_event::ClientEvent;
pub use input_thread::{InputParams, InputThread, PROBE_TIMEOUT};
pub use pane_replica::PaneReplica;
pub use server_link::ServerLink;
pub use shared_focus::SharedFocus;
