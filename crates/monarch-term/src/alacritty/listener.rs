//! Receives alacritty events during parsing without doing any work.

use std::sync::mpsc::{Receiver, Sender, channel};

use alacritty_terminal::event::{Event, EventListener};

/// Queues alacritty events. Events fire synchronously inside
/// `Processor::advance` while the terminal is borrowed, so the listener only
/// enqueues; [`super::event_translator::EventTranslator`] handles them after
/// parsing, when the terminal (and its colours) can be read.
pub(crate) struct PaneListener {
    tx: Sender<Event>,
}

impl PaneListener {
    /// A listener and the receiving end of its queue.
    pub(crate) fn new() -> (Self, Receiver<Event>) {
        let (tx, rx) = channel();
        (Self { tx }, rx)
    }
}

impl EventListener for PaneListener {
    fn send_event(&self, event: Event) {
        // The receiver lives as long as the engine; a send can only fail
        // during teardown, when dropping the event is correct.
        let _ = self.tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_events_in_order() {
        let (listener, rx) = PaneListener::new();
        listener.send_event(Event::Bell);
        listener.send_event(Event::Title("t".into()));
        assert!(matches!(rx.try_recv(), Ok(Event::Bell)));
        assert!(matches!(rx.try_recv(), Ok(Event::Title(t)) if t == "t"));
    }

    #[test]
    fn sending_after_receiver_drop_is_harmless() {
        let (listener, rx) = PaneListener::new();
        drop(rx);
        listener.send_event(Event::Bell);
    }
}
