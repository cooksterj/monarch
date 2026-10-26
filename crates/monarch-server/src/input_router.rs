//! Where typed bytes go: each pane's input channel by pane id, behind its
//! own small lock, so a keystroke never waits for the session lock (which
//! a frame holds while it captures and encodes every visible pane).

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard, PoisonError};

use monarch_core::PaneId;

/// The input channel of every pane that has one.
#[derive(Debug, Default)]
pub struct InputRouter {
    panes: Mutex<HashMap<PaneId, Sender<Vec<u8>>>>,
}

impl InputRouter {
    /// Routes `pane`'s input to `input` from now on.
    pub fn add(&self, pane: PaneId, input: Sender<Vec<u8>>) {
        self.lock().insert(pane, input);
    }

    /// Forgets `pane` (it exited).
    pub fn remove(&self, pane: PaneId) {
        self.lock().remove(&pane);
    }

    /// Queues `bytes` for `pane`'s program, or hands them back when the
    /// pane has no channel here. A program that has exited drops them;
    /// its exit event follows.
    ///
    /// # Errors
    /// Returns `bytes` when `pane` is not routed here.
    pub fn send(&self, pane: PaneId, bytes: Vec<u8>) -> Result<(), Vec<u8>> {
        let input = self.lock().get(&pane).cloned();
        match input {
            Some(input) => {
                let _ = input.send(bytes);
                Ok(())
            }
            None => Err(bytes),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<PaneId, Sender<Vec<u8>>>> {
        self.panes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn bytes_reach_the_pane_in_order_and_unknown_panes_hand_them_back() {
        let router = InputRouter::default();
        let (tx, rx) = mpsc::channel();
        router.add(PaneId(1), tx);
        router.send(PaneId(1), b"ab".to_vec()).expect("routed");
        router.send(PaneId(1), b"c".to_vec()).expect("routed");
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            [b"ab".to_vec(), b"c".to_vec()]
        );
        assert_eq!(router.send(PaneId(2), b"x".to_vec()), Err(b"x".to_vec()));
        router.remove(PaneId(1));
        assert_eq!(router.send(PaneId(1), b"y".to_vec()), Err(b"y".to_vec()));
    }

    #[test]
    fn a_pane_whose_program_exited_swallows_input() {
        let router = InputRouter::default();
        let (tx, rx) = mpsc::channel();
        router.add(PaneId(1), tx);
        drop(rx);
        assert_eq!(router.send(PaneId(1), b"x".to_vec()), Ok(()));
    }
}
