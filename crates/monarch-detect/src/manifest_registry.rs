//! The manifests built into Monarch (vendored from Herdr; see
//! `manifests/ATTRIBUTION.md`) and finding the one for a program.

use crate::Manifest;

/// Every bundled manifest's file name and text.
const BUNDLED: [(&str, &str); 22] = [
    ("amp.toml", include_str!("../manifests/amp.toml")),
    (
        "antigravity.toml",
        include_str!("../manifests/antigravity.toml"),
    ),
    ("claude.toml", include_str!("../manifests/claude.toml")),
    ("cline.toml", include_str!("../manifests/cline.toml")),
    ("codex.toml", include_str!("../manifests/codex.toml")),
    ("cursor.toml", include_str!("../manifests/cursor.toml")),
    ("devin.toml", include_str!("../manifests/devin.toml")),
    ("droid.toml", include_str!("../manifests/droid.toml")),
    ("gemini.toml", include_str!("../manifests/gemini.toml")),
    (
        "github-copilot.toml",
        include_str!("../manifests/github-copilot.toml"),
    ),
    ("grok.toml", include_str!("../manifests/grok.toml")),
    ("hermes.toml", include_str!("../manifests/hermes.toml")),
    ("kilo.toml", include_str!("../manifests/kilo.toml")),
    ("kimi.toml", include_str!("../manifests/kimi.toml")),
    ("kiro.toml", include_str!("../manifests/kiro.toml")),
    ("letta.toml", include_str!("../manifests/letta.toml")),
    ("maki.toml", include_str!("../manifests/maki.toml")),
    ("muse.toml", include_str!("../manifests/muse.toml")),
    ("opencode.toml", include_str!("../manifests/opencode.toml")),
    ("pi.toml", include_str!("../manifests/pi.toml")),
    ("qodercli.toml", include_str!("../manifests/qodercli.toml")),
    ("qwen.toml", include_str!("../manifests/qwen.toml")),
];

/// A set of manifests, looked up by agent or program name.
#[derive(Debug, Clone, Default)]
pub struct ManifestRegistry {
    manifests: Vec<Manifest>,
}

impl ManifestRegistry {
    /// The bundled manifests. One that fails to parse is left out (the
    /// tests make sure none does).
    #[must_use]
    pub fn bundled() -> Self {
        let manifests = BUNDLED
            .iter()
            .filter_map(|(_, text)| Manifest::parse(text).ok())
            .collect();
        Self { manifests }
    }

    /// The manifest for a program called `name` (its id or an alias).
    #[must_use]
    pub fn for_program(&self, name: &str) -> Option<&Manifest> {
        self.manifests.iter().find(|m| m.answers_to(name))
    }

    /// Every manifest.
    #[must_use]
    pub fn manifests(&self) -> &[Manifest] {
        &self.manifests
    }
}

impl From<Vec<Manifest>> for ManifestRegistry {
    fn from(manifests: Vec<Manifest>) -> Self {
        Self { manifests }
    }
}

#[cfg(test)]
mod tests;
