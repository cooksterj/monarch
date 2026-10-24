//! Monarch's headless terminal layer.
//!
//! [`Terminal`] is what the server holds per pane: an [`Engine`] (the VT
//! emulator, [`AlacrittyEngine`] by default) composed with an [`OscTap`]
//! that captures the OSC sequences the engine ignores (notifications, cwd,
//! prompt marks). Engines produce the engine-neutral
//! [`monarch_proto::Screen`].

mod alacritty;
mod damage;
mod engine;
mod engine_event;
mod host_profile;
mod osc;
mod osc_tap;
mod palette;
mod scroll_request;
mod terminal;

pub use alacritty::AlacrittyEngine;
pub use damage::Damage;
pub use engine::Engine;
pub use engine_event::EngineEvent;
pub use host_profile::HostProfile;
pub use osc::Osc;
pub use osc_tap::OscTap;
pub use palette::Palette;
pub use scroll_request::ScrollRequest;
pub use terminal::Terminal;
