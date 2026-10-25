//! A detection manifest: how one agent's screen looks in each state, as
//! prioritized rules. The format is Herdr's (see `manifests/`).

mod evaluate;

use serde::Deserialize;

use crate::gate::Budget;
use crate::rule::RuleSpec;
use crate::{DetectError, Rule};

/// The newest manifest format this engine reads.
pub const ENGINE_VERSION: u32 = 3;

const MAX_RULES: usize = 128;

/// A manifest as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSpec {
    id: String,
    version: Option<String>,
    min_engine_version: Option<u32>,
    #[serde(rename = "updated_at")]
    _updated_at: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    rules: Vec<RuleSpec>,
}

/// A validated, compiled manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    /// The agent's name (`claude`), which is also a program name.
    pub id: String,
    /// The manifest's own version (`2026.10.11.1`).
    pub version: Option<String>,
    /// Other program names the agent runs as.
    pub aliases: Vec<String>,
    /// Its rules, in file order.
    pub rules: Vec<Rule>,
}

impl Manifest {
    /// Reads and validates a manifest.
    ///
    /// # Errors
    /// Bad TOML or unknown fields, a newer engine version than
    /// [`ENGINE_VERSION`], no rules or too many, or an invalid rule.
    pub fn parse(text: &str) -> Result<Self, DetectError> {
        let spec: ManifestSpec = toml::from_str(text)?;
        let declared = spec.min_engine_version;
        if let Some(needs) = declared.filter(|v| *v > ENGINE_VERSION) {
            let have = ENGINE_VERSION;
            return Err(DetectError::Engine { needs, have });
        }
        if spec.rules.is_empty() || spec.rules.len() > MAX_RULES {
            let n = spec.rules.len();
            return Err(DetectError::Invalid(format!(
                "{n} rules; 1 to {MAX_RULES} allowed"
            )));
        }
        let mut budget = Budget::default();
        let rules = spec
            .rules
            .into_iter()
            .map(|r| Rule::compile(r, &mut budget))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(rule) = rules
            .iter()
            .find(|r| r.region.engine_version() > declared.unwrap_or(ENGINE_VERSION))
        {
            return Err(DetectError::Invalid(format!(
                "rule {}: {} needs min_engine_version {}",
                rule.id,
                rule.region,
                rule.region.engine_version()
            )));
        }
        Ok(Self {
            id: spec.id,
            version: spec.version,
            aliases: spec.aliases,
            rules,
        })
    }

    /// Whether a program called `name` (a path is reduced to its last
    /// part) is this agent.
    #[must_use]
    pub fn answers_to(&self, name: &str) -> bool {
        let name = name.rsplit('/').next().unwrap_or(name);
        self.id == name || self.aliases.iter().any(|a| a == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_RULE: &str = "[[rules]]\nid = \"r\"\ncontains = [\"x\"]";

    fn with(header: &str) -> Result<Manifest, DetectError> {
        Manifest::parse(&format!("id = \"a\"\n{header}\n{ONE_RULE}"))
    }

    #[test]
    fn a_minimal_manifest_reads() {
        let m = with("aliases = [\"a-cli\"]\nversion = \"1\"").expect("valid");
        assert_eq!((m.id.as_str(), m.version.as_deref()), ("a", Some("1")));
        assert!(m.answers_to("a") && m.answers_to("/usr/bin/a-cli"));
        assert!(!m.answers_to("ab"));
    }

    #[test]
    fn engine_versions_are_honoured() {
        assert_eq!(
            with("min_engine_version = 4").expect_err("refused"),
            DetectError::Engine { needs: 4, have: 3 }
        );
        let top = "id = \"a\"\nmin_engine_version = 2\n[[rules]]\nid = \"r\"\n\
                   region = \"top_non_empty_lines(3)\"\ncontains = [\"x\"]";
        let err = Manifest::parse(top).expect_err("refused").to_string();
        assert_eq!(
            err,
            "rule r: top_non_empty_lines(3) needs min_engine_version 3"
        );
        assert!(Manifest::parse(&top.replace("min_engine_version = 2\n", "")).is_ok());
    }

    #[test]
    fn rule_counts_and_unknown_fields_are_checked() {
        assert!(Manifest::parse("id = \"a\"").is_err(), "no rules");
        assert!(matches!(with("colour = 1"), Err(DetectError::Parse(_))));
        let many = format!("id = \"a\"\n{}", format!("{ONE_RULE}\n").repeat(129));
        assert!(Manifest::parse(&many).is_err());
        assert!(
            Manifest::parse(&many.replacen(ONE_RULE, "", 1)).is_ok(),
            "128 is fine"
        );
    }
}
