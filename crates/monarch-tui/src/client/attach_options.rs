//! What `monarch attach` needs to know, and how a config file sets it (at
//! startup and on every reload).

use std::path::PathBuf;
use std::time::Duration;

use monarch_core::{Keymap, LoadedConfig, MouseMode};

use crate::TuiError;

/// Where the session is, which key is the prefix and how to title the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachOptions {
    /// The session server's socket.
    pub socket: PathBuf,
    /// Session name, shown in the status line and window title.
    pub session: String,
    /// The config file, watched for changes; `None` never reloads.
    pub config_path: Option<PathBuf>,
    /// The prefix control byte (`0x02` for Ctrl-b).
    pub prefix: u8,
    /// The prefix as the user wrote it, for hints (`ctrl+b`).
    pub prefix_label: String,
    /// `[ui].window_title` template.
    pub window_title: String,
    /// `[ui].mouse` capture policy.
    pub mouse: MouseMode,
    /// `[ui].sidebar`: start with the sidebar shown.
    pub sidebar: bool,
    /// `[ui].sidebar_width` in columns, its divider included.
    pub sidebar_width: u16,
    /// `[ui].sidebar_min_columns`: narrower hosts hide the sidebar.
    pub sidebar_min_columns: u16,
    /// What each key after the prefix does.
    pub keymap: Keymap,
    /// How long the prefix waits before the key popup appears.
    pub which_key_delay: Duration,
    /// How long a repeatable key keeps working without the prefix.
    pub repeat: Duration,
    /// Problems with the config, shown once the client is up.
    pub warnings: Vec<String>,
}

impl AttachOptions {
    /// Options for attaching to `session` at `socket`, set from `loaded`.
    ///
    /// # Errors
    /// The config's prefix is not a control chord.
    pub fn new(socket: PathBuf, session: String, loaded: &LoadedConfig) -> Result<Self, TuiError> {
        let ui = &loaded.config.ui;
        let mut opts = Self {
            socket,
            session,
            config_path: Some(loaded.path.clone()),
            prefix: 0,
            prefix_label: String::new(),
            window_title: ui.window_title.clone(),
            mouse: ui.mouse,
            sidebar: ui.sidebar,
            sidebar_width: ui.sidebar_width,
            sidebar_min_columns: ui.sidebar_min_columns,
            keymap: Keymap::default(),
            which_key_delay: Duration::ZERO,
            repeat: Duration::ZERO,
            warnings: Vec::new(),
        };
        opts.configure(loaded)?;
        Ok(opts)
    }

    /// Takes every setting from `loaded`; on error nothing changes.
    ///
    /// # Errors
    /// The config's prefix is not a control chord.
    pub fn configure(&mut self, loaded: &LoadedConfig) -> Result<(), TuiError> {
        let config = &loaded.config;
        let prefix = config.keys.prefix_byte().ok_or_else(|| {
            TuiError::Config(format!(
                "prefix `{}` is not a ctrl+key chord",
                config.keys.prefix
            ))
        })?;
        let (keymap, mut warnings) = Keymap::from_config(&config.keys);
        warnings.extend(
            loaded
                .unknown_keys
                .iter()
                .map(|k| format!("unknown key `{k}`")),
        );
        self.prefix = prefix;
        self.prefix_label.clone_from(&config.keys.prefix);
        self.window_title.clone_from(&config.ui.window_title);
        self.mouse = config.ui.mouse;
        self.sidebar = config.ui.sidebar;
        self.sidebar_width = config.ui.sidebar_width;
        self.sidebar_min_columns = config.ui.sidebar_min_columns;
        self.keymap = keymap;
        self.which_key_delay = Duration::from_millis(config.keys.which_key_delay_ms);
        self.repeat = Duration::from_millis(config.keys.repeat_ms);
        self.warnings = warnings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{Action, Config, Key};

    use super::*;

    fn loaded(config: Config) -> LoadedConfig {
        LoadedConfig {
            config,
            path: "/home/u/.config/monarch/config.toml".into(),
            found: true,
            unknown_keys: vec!["ui.sidebar_widht".into()],
        }
    }

    #[test]
    fn takes_every_setting_from_the_config() {
        let mut config = Config::default();
        config.keys.prefix = "ctrl+a".into();
        config.keys.preset = "tmux".into();
        config.ui.sidebar_width = 40;
        let opts = AttachOptions::new("/tmp/m.sock".into(), "main".into(), &loaded(config))
            .expect("valid");
        assert_eq!((opts.prefix, opts.prefix_label.as_str()), (0x01, "ctrl+a"));
        assert_eq!(opts.keymap.action(Key::Char('w')), Some(Action::GoToPane));
        assert_eq!(opts.sidebar_width, 40);
        assert_eq!(opts.warnings, ["unknown key `ui.sidebar_widht`"]);
        assert!(opts.config_path.is_some_and(|p| p.ends_with("config.toml")));
    }

    #[test]
    fn a_bad_prefix_changes_nothing() {
        let mut opts = AttachOptions::new(
            "/tmp/m.sock".into(),
            "main".into(),
            &loaded(Config::default()),
        )
        .expect("valid");
        let before = opts.clone();
        let mut config = Config::default();
        config.keys.prefix = "alt+b".into();
        config.ui.sidebar = false;
        let err = opts.configure(&loaded(config)).expect_err("rejected");
        assert!(err.to_string().contains("alt+b"), "{err}");
        assert_eq!(opts, before);
    }
}
