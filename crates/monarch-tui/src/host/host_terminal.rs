//! The host terminal held across attaches, so switching sessions never
//! leaves the alternate screen.
//!
//! Each attach used to take the terminal over and give it back, so a
//! `prefix S` switch showed the shell underneath for a moment. An attach
//! that ends in a switch now hands the terminal back here instead, and the
//! next one draws over what is already on screen.

use std::io::{self, Stdout, Write};

use ratatui::Terminal;

use crate::HostBackend;

/// The taken-over host terminal between attaches, if one is held. Dropping
/// it while it holds a terminal gives the terminal back.
#[derive(Debug)]
pub struct HostTerminal<W: Write = Stdout> {
    held: Option<Terminal<HostBackend<W>>>,
}

impl<W: Write> Default for HostTerminal<W> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl HostTerminal<Stdout> {
    /// The held terminal, or the host freshly taken over.
    ///
    /// # Errors
    /// The terminal refuses raw mode or cannot be written.
    pub fn take_or_init(&mut self) -> io::Result<Terminal<HostBackend>> {
        match self.held.take() {
            Some(ui) => Ok(ui),
            None => HostBackend::init(),
        }
    }
}

impl<W: Write> HostTerminal<W> {
    /// Holds `ui` for the next attach.
    pub fn keep(&mut self, ui: Terminal<HostBackend<W>>) {
        self.held = Some(ui);
    }

    /// Whether a terminal is held.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }

    /// Takes the held terminal without giving the host back.
    pub fn release(&mut self) -> Option<Terminal<HostBackend<W>>> {
        self.held.take()
    }
}

impl<W: Write> Drop for HostTerminal<W> {
    fn drop(&mut self) {
        if self.held.take().is_some() {
            ratatui::restore();
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::{TerminalOptions, Viewport};

    use super::*;

    fn terminal() -> Terminal<HostBackend<Vec<u8>>> {
        let viewport = Viewport::Fixed(Rect::new(0, 0, 10, 2));
        Terminal::with_options(HostBackend::new(Vec::new()), TerminalOptions { viewport })
            .expect("terminal")
    }

    #[test]
    fn nothing_is_held_at_first() {
        let mut host = HostTerminal::<Vec<u8>>::default();
        assert!(!host.is_held());
        assert!(host.release().is_none());
    }

    #[test]
    fn a_kept_terminal_is_handed_back_once() {
        let mut host = HostTerminal::default();
        host.keep(terminal());
        assert!(host.is_held());
        assert!(host.release().is_some());
        assert!(!host.is_held(), "taken, so dropping restores nothing");
    }
}
