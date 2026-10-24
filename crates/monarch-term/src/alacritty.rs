//! The default [`crate::Engine`] backend, built on `alacritty_terminal`.

mod cell_convert;
mod engine;
mod event_translator;
mod listener;
mod mode_convert;

pub use engine::AlacrittyEngine;
