//! Applying the config file again, when it changes or on `prefix R`,
//! without restarting: keys, mouse, sidebar and window title.

use std::time::Instant;

use monarch_core::ConfigLoader;

use super::{AttachApp, WARNING};
use crate::WhichKeyEntry;
use crate::client::{AttachOptions, InputKeys};

impl AttachApp {
    /// Reloads when the config file changed (looked at once a second).
    pub(super) fn watch_config(&mut self) {
        if self.config.changed(Instant::now()) {
            self.reload_config();
        }
    }

    /// Reads the config file and applies it; a file that does not load
    /// leaves every setting as it was and says why.
    pub(super) fn reload_config(&mut self) {
        let Some(path) = self.opts.config_path.clone() else {
            return;
        };
        let reloaded = ConfigLoader::at(&path)
            .load()
            .map_err(|e| e.to_string())
            .and_then(|loaded| {
                let mut opts = self.opts.clone();
                opts.configure(&loaded).map_err(|e| e.to_string())?;
                Ok(opts)
            });
        let text = match reloaded {
            Ok(opts) if opts.warnings.is_empty() => {
                self.apply_options(opts);
                "config reloaded".to_owned()
            }
            Ok(opts) => {
                let text = format!("config reloaded: {}", opts.warnings.join("; "));
                self.apply_options(opts);
                text
            }
            Err(e) => format!("config not reloaded: {e}"),
        };
        self.notice = Some((text, Instant::now() + WARNING));
        self.dirty = true;
    }

    /// Switches to `opts`. A sidebar hidden with `prefix t` stays hidden
    /// unless the file's `sidebar` setting itself changed.
    fn apply_options(&mut self, opts: AttachOptions) {
        if opts.sidebar != self.opts.sidebar {
            self.sidebar = opts.sidebar;
        }
        if !self.passthrough {
            self.mirror.set_mouse(opts.mouse);
        }
        self.which_key = WhichKeyEntry::from_keymap(&opts.keymap);
        let _ = self.rekey.send(InputKeys::from(&opts));
        self.opts = opts;
        self.relayout();
    }
}
