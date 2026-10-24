//! Monarch domain types, configuration, keymap and theme.
//!
//! This crate performs no IO beyond reading the config file through
//! [`ConfigLoader`]; everything else is pure data so it can be shared by the
//! server, the TUI client and the CLI and tested without a terminal.

mod action;
mod agent_kind;
mod agent_state;
mod config;
mod error;
mod ids;
mod layout;

pub use action::{Action, RESIZE_STEP};
pub use agent_kind::AgentKind;
pub use agent_state::AgentState;
pub use config::{
    AgentConfig, Config, ConfigLoader, KeysConfig, LoadedConfig, MouseMode, NotificationsConfig,
    ProjectsConfig, SessionConfig, TerminalConfig, ThemeConfig, UiConfig,
};
pub use error::CoreError;
pub use ids::{ClientId, PaneId, TabId};
pub use layout::{Direction, Rect, SplitAxis, SplitTree, Tab};
