//! Reads facts about a pane's process, behind a trait so callers are
//! testable without real processes.

use std::path::PathBuf;

/// What the server asks the OS about a process.
pub trait ProcessProbe: Send + Sync {
    /// The working directory of process `pid`, if it can be read.
    fn cwd(&self, pid: u32) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    struct Fixed(BTreeMap<u32, PathBuf>);

    impl ProcessProbe for Fixed {
        fn cwd(&self, pid: u32) -> Option<PathBuf> {
            self.0.get(&pid).cloned()
        }
    }

    #[test]
    fn probes_are_usable_as_trait_objects() {
        let probe: Box<dyn ProcessProbe> = Box::new(Fixed(BTreeMap::from([(7, "/w".into())])));
        assert_eq!(probe.cwd(7), Some(PathBuf::from("/w")));
        assert_eq!(probe.cwd(8), None, "unknown process");
    }
}
