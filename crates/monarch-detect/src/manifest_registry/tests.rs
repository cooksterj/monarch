use super::*;

#[test]
fn every_bundled_manifest_is_valid() {
    for (name, text) in BUNDLED {
        let manifest = Manifest::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            name.starts_with(&manifest.id) || manifest.answers_to(name.trim_end_matches(".toml")),
            "{name}"
        );
    }
    assert_eq!(ManifestRegistry::bundled().manifests().len(), BUNDLED.len());
}

#[test]
fn programs_find_their_manifest() {
    let registry = ManifestRegistry::bundled();
    let id = |name| registry.for_program(name).map(|m| m.id.as_str());
    assert_eq!(id("claude"), Some("claude"));
    assert_eq!(id("/opt/bin/codex"), Some("codex"));
    assert_eq!(id("cursor-agent"), Some("cursor"));
    assert_eq!(id("zsh"), None);
    assert!(ManifestRegistry::default().for_program("claude").is_none());
}
