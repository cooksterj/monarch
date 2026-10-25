//! Why a detection manifest was rejected.

use thiserror::Error;

/// A manifest that could not be read, or that breaks the format's rules.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DetectError {
    /// Not valid TOML, or not a manifest's shape (unknown fields included).
    #[error("manifest is not valid: {0}")]
    Parse(String),
    /// Needs a newer engine than this one.
    #[error("manifest needs engine version {needs}; this is version {have}")]
    Engine {
        /// What the manifest declares.
        needs: u32,
        /// [`crate::ENGINE_VERSION`].
        have: u32,
    },
    /// Well-formed but breaks a rule of the format (limits, regions,
    /// regexes, gates).
    #[error("{0}")]
    Invalid(String),
}

impl From<toml::de::Error> for DetectError {
    fn from(err: toml::de::Error) -> Self {
        Self::Parse(err.message().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_say_what_is_wrong() {
        let engine = DetectError::Engine { needs: 9, have: 3 };
        assert_eq!(
            engine.to_string(),
            "manifest needs engine version 9; this is version 3"
        );
        let parse: DetectError = toml::from_str::<toml::Table>("= 1")
            .expect_err("refused")
            .into();
        assert!(matches!(parse, DetectError::Parse(_)));
    }
}
