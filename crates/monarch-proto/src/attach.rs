//! Types carried on the binary attach stream between server and client.
//!
//! The server emulates every pane and sends cell-grid changes, not PTY
//! bytes: a [`Snapshot`] when a client first sees a pane (or must resync),
//! then [`Delta`]s holding only the rows that changed. Rows travel as style
//! runs ([`Run`]), and styles are interned per connection ([`StyleTable`] on
//! the server, [`StyleCache`] on the client). Frames are postcard messages
//! behind a big-endian `u32` length ([`FrameCodec`]).

mod client_msg;
mod delta;
mod frame_codec;
mod host_info;
mod layout_msg;
mod pane_layout;
mod pane_state;
mod pane_summary;
mod row_codec;
mod row_delta;
mod run;
mod server_msg;
mod snapshot;
mod style_cache;
mod style_def;
mod style_id;
mod style_table;
mod tab_summary;

pub use client_msg::ClientMsg;
pub use delta::Delta;
pub use frame_codec::FrameCodec;
pub use host_info::HostInfo;
pub use layout_msg::LayoutMsg;
pub use pane_layout::PaneLayout;
pub use pane_state::PaneState;
pub use pane_summary::PaneSummary;
pub use row_codec::RowCodec;
pub use row_delta::RowDelta;
pub use run::Run;
pub use server_msg::ServerMsg;
pub use snapshot::Snapshot;
pub use style_cache::StyleCache;
pub use style_def::StyleDef;
pub use style_id::StyleId;
pub use style_table::StyleTable;
pub use tab_summary::TabSummary;
