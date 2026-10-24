//! Attaching to a session server: the socket link, a pure model of the
//! session built from frames, the raw-input thread and the main loop.

mod applied;
mod attach_app;
mod attach_options;
mod attach_outcome;
mod attach_state;
mod attach_view;
mod click_counter;
mod client_event;
mod focus_view;
mod grid_point;
mod input_thread;
mod mouse_action;
mod mouse_router;
mod pane_replica;
mod select_event;
mod select_unit;
mod selection;
mod server_link;
mod shared_focus;
mod window_title;

pub use applied::Applied;
pub use attach_app::AttachApp;
pub use attach_options::AttachOptions;
pub use attach_outcome::AttachOutcome;
pub use attach_state::AttachState;
pub use attach_view::AttachView;
pub use click_counter::{ClickCounter, MULTI_CLICK};
pub use client_event::ClientEvent;
pub use focus_view::FocusView;
pub use grid_point::GridPoint;
pub use input_thread::{InputParams, InputThread, PROBE_TIMEOUT};
pub use mouse_action::MouseAction;
pub use mouse_router::{MouseRouter, WHEEL_LINES};
pub use pane_replica::PaneReplica;
pub use select_event::SelectEvent;
pub use select_unit::SelectUnit;
pub use selection::Selection;
pub use server_link::ServerLink;
pub use shared_focus::SharedFocus;
pub use window_title::WindowTitle;
