//! Locates, reads and parses the config file.
//!
//! Unknown keys are reported as warnings rather than errors, so a typo never
//! stops Monarch from starting; type errors are real errors.

use std::path::{Path, PathBuf};

use super::Config;
use crate::CoreError;

/// A parsed config plus everything worth telling the user about it.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    /// The effective configuration.
    pub config: Config,
    /// The file that was read, or would have been read if it existed.
    pub path: PathBuf,
    /// Whether the file existed.
    pub found: bool,
    /// Unknown keys, as dotted paths (e.g. `ui.sidebar_widht`).
    pub unknown_keys: Vec<String>,
}

/// Resolves the config path from the environment and loads it.
#[derive(Debug, Clone, Default)]
pub struct ConfigLoader {
    explicit: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl ConfigLoader {
    /// A loader that reads `MONARCH_CONFIG`, `XDG_CONFIG_HOME` and `HOME`.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |k| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            explicit: var("MONARCH_CONFIG"),
            xdg_config_home: var("XDG_CONFIG_HOME"),
            home: var("HOME"),
        }
    }

    /// A loader that reads exactly `path`.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            explicit: Some(path.into()),
            ..Self::default()
        }
    }

    /// The config file path: `MONARCH_CONFIG`, else
    /// `$XDG_CONFIG_HOME/monarch/config.toml`, else `~/.config/monarch/config.toml`
    /// (the same on macOS and Linux so dotfiles and SSH behave predictably).
    pub fn path(&self) -> Result<PathBuf, CoreError> {
        if let Some(p) = &self.explicit {
            return Ok(p.clone());
        }
        let base = match (&self.xdg_config_home, &self.home) {
            (Some(xdg), _) => xdg.clone(),
            (None, Some(home)) => home.join(".config"),
            (None, None) => return Err(CoreError::NoHome),
        };
        Ok(base.join("monarch").join("config.toml"))
    }

    /// Loads the config; a missing file yields the defaults.
    pub fn load(&self) -> Result<LoadedConfig, CoreError> {
        let path = self.path()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text, path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LoadedConfig {
                config: Config::default(),
                path,
                found: false,
                unknown_keys: Vec::new(),
            }),
            Err(source) => Err(CoreError::ConfigRead { path, source }),
        }
    }

    /// Parses config text, collecting unknown keys.
    pub fn parse(text: &str, path: PathBuf) -> Result<LoadedConfig, CoreError> {
        let mut unknown_keys = Vec::new();
        let deserializer = toml::Deserializer::parse(text).map_err(|e| parse_error(&path, &e))?;
        let config = serde_ignored::deserialize(deserializer, |p| unknown_keys.push(p.to_string()))
            .map_err(|e| parse_error(&path, &e))?;
        Ok(LoadedConfig {
            config,
            path,
            found: true,
            unknown_keys,
        })
    }
}

fn parse_error(path: &Path, e: &toml::de::Error) -> CoreError {
    CoreError::ConfigParse {
        path: path.to_owned(),
        message: e.to_string().trim_end().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loader(xdg: Option<&str>, home: Option<&str>) -> ConfigLoader {
        ConfigLoader {
            explicit: None,
            xdg_config_home: xdg.map(PathBuf::from),
            home: home.map(PathBuf::from),
        }
    }

    #[test]
    fn path_precedence() {
        let p = |l: ConfigLoader| l.path().ok();
        assert_eq!(p(ConfigLoader::at("/x.toml")), Some("/x.toml".into()));
        assert_eq!(
            p(loader(Some("/xdg"), Some("/h"))),
            Some("/xdg/monarch/config.toml".into())
        );
        assert_eq!(
            p(loader(None, Some("/h"))),
            Some("/h/.config/monarch/config.toml".into())
        );
        assert!(matches!(loader(None, None).path(), Err(CoreError::NoHome)));
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let loaded = ConfigLoader::at(dir.path().join("nope.toml"))
            .load()
            .expect("loads");
        assert!(!loaded.found);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn unknown_keys_are_warnings_not_errors() {
        let text = "[ui]\nsidebar_widht = 40\n[keys]\nprefix = \"ctrl+a\"\n";
        let loaded = ConfigLoader::parse(text, "c.toml".into()).expect("parses");
        assert_eq!(loaded.unknown_keys, ["ui.sidebar_widht"]);
        assert_eq!(loaded.config.keys.prefix, "ctrl+a");
    }

    #[test]
    fn type_errors_are_errors_with_location() {
        let err = ConfigLoader::parse("[ui]\nsidebar = \"yes\"\n", "c.toml".into());
        let msg = err.expect_err("type error").to_string();
        assert!(msg.contains("c.toml"), "{msg}");
        assert!(msg.contains("sidebar"), "{msg}");
    }

    #[test]
    fn reads_file_from_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[session]\ndefault_name = \"work\"\n").expect("write");
        let loaded = ConfigLoader::at(&path).load().expect("loads");
        assert!(loaded.found);
        assert_eq!(loaded.config.session.default_name, "work");
    }
}
