//! Finding the running sessions, behind a trait so the client needs no
//! socket code of its own (the CLI asks each session's socket).

use crate::client::SessionEntry;

/// Lists running sessions. Called off the drawing thread: it may wait on
/// each server.
pub trait SessionLister: Send + Sync {
    /// Every running session, in name order.
    fn list(&self) -> Vec<SessionEntry>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    struct Two;

    impl SessionLister for Two {
        fn list(&self) -> Vec<SessionEntry> {
            ["a", "b"]
                .map(|name| SessionEntry {
                    name: name.into(),
                    info: None,
                })
                .to_vec()
        }
    }

    #[test]
    fn listers_are_shared_across_threads() {
        let lister: Arc<dyn SessionLister> = Arc::new(Two);
        let other = Arc::clone(&lister);
        let names = std::thread::spawn(move || other.list().len())
            .join()
            .expect("thread");
        assert_eq!((names, lister.list()[1].name.as_str()), (2, "b"));
    }
}
