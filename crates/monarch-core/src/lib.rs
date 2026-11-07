//! Monarch domain types, configuration, keymap and theme.
//!
//! This crate performs no IO beyond reading the config file through
//! [`ConfigLoader`]; everything else is pure data so it can be shared by the
//! server, the TUI client and the CLI and tested without a terminal.

mod action;
mod action_group;
mod age_label;
mod agent_kind;
mod agent_state;
mod clock;
mod config;
mod error;
mod given_name;
mod ids;
mod keymap;
mod layout;
mod rgb;
mod session_name;
mod system_clock;
mod theme;
mod unix_millis;

pub use action::{Action, RESIZE_STEP};
pub use action_group::ActionGroup;
pub use age_label::age_label;
pub use agent_kind::AgentKind;
pub use agent_state::AgentState;
pub use clock::Clock;
pub use config::{
    AgentConfig, Config, ConfigLoader, KeysConfig, LoadedConfig, MouseMode, ProjectsConfig,
    SESSION_ID, SessionConfig, TerminalConfig, ThemeConfig, UiConfig,
};
pub use error::CoreError;
pub use given_name::given_name;
pub use ids::{ClientId, PaneId, TabId};
pub use keymap::{Key, Keymap, KeymapPreset};
pub use layout::{Direction, Rect, SplitAxis, SplitTree, Tab};
pub use rgb::Rgb;
pub use session_name::is_session_name;
pub use system_clock::SystemClock;
pub use theme::{THEME_TOKENS, Theme};
pub use unix_millis::UnixMillis;
