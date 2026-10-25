//! Fixture tests for the bundled manifests.
//!
//! `fixtures/<manifest id>/<rule id>.txt` is a screen on which that rule
//! must win (`no_match.txt`: none may match); `<rule id>--<note>.txt`
//! adds another case for the same rule. A fixture may start with
//! `@title: …` and `@progress: …` lines for the OSC inputs. Every
//! manifest needs a winning fixture for some blocked rule and some working
//! rule, if it has them.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use monarch_core::AgentState;
use monarch_detect::{DetectionInput, Manifest, ManifestRegistry};

struct Fixture {
    expected: String,
    title: String,
    progress: String,
    screen: String,
}

fn read(path: &Path) -> Fixture {
    let text = fs::read_to_string(path).expect("readable fixture");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("utf-8 name");
    let expected = stem.split("--").next().unwrap_or(stem).to_owned();
    let mut fixture = Fixture {
        expected,
        title: String::new(),
        progress: String::new(),
        screen: String::new(),
    };
    let mut lines = text
        .strip_suffix('\n')
        .unwrap_or(&text)
        .split('\n')
        .peekable();
    while let Some(line) = lines.peek() {
        if let Some(title) = line.strip_prefix("@title: ") {
            fixture.title = title.to_owned();
        } else if let Some(progress) = line.strip_prefix("@progress: ") {
            fixture.progress = progress.to_owned();
        } else {
            break;
        }
        lines.next();
    }
    fixture.screen = lines.collect::<Vec<_>>().join("\n");
    fixture
}

/// Checks every fixture of `manifest`; returns the states they covered.
fn check(manifest: &Manifest, dir: &Path) -> BTreeSet<String> {
    let mut covered = BTreeSet::new();
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|_| panic!("{}: no fixtures", manifest.id))
        .map(|e| e.expect("entry").path())
        .collect();
    paths.sort();
    for path in paths {
        let f = read(&path);
        let input = DetectionInput {
            screen: &f.screen,
            osc_title: &f.title,
            osc_progress: &f.progress,
        };
        let (verdict, checks) = manifest.explain(input);
        let won = verdict.rule_id().unwrap_or("no_match");
        let matched: Vec<_> = checks
            .iter()
            .filter(|c| c.matched)
            .map(|c| &c.rule.id)
            .collect();
        assert_eq!(won, f.expected, "{}: matched {matched:?}", path.display());
        let state = verdict.state().map_or("hold".into(), |s| format!("{s:?}"));
        covered.insert(state);
    }
    covered
}

#[test]
fn every_manifest_detects_its_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    for manifest in ManifestRegistry::bundled().manifests() {
        let covered = check(manifest, &root.join(&manifest.id));
        for state in [AgentState::Blocked, AgentState::Working] {
            let has_rule = manifest.rules.iter().any(|r| r.state == state);
            let name = format!("{state:?}");
            assert!(
                !has_rule || covered.contains(&name),
                "{}: no fixture shows it {name}",
                manifest.id
            );
        }
    }
}
