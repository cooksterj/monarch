//! The pane's `Term` as vte's handler, with emoji presentation widths.
//!
//! alacritty sizes each character on its own, so the presentation selector
//! `U+FE0F` is zero width and `⚠️` (`U+26A0 U+FE0F`) takes one cell. Hosts
//! (iTerm2, Terminal.app, kitty, WezTerm, Ghostty) and the programs that
//! lay text out for them (Ink's `string-width`, so Claude Code) take two,
//! so everything after such an emoji landed one cell left of where the
//! program put it, and its later cursor moves overwrote the wrong cells.
//! Here a `U+FE0F` after a narrow glyph widens that glyph to two cells, as
//! if it had been a wide character. Every other call goes to `Term`
//! unchanged; this sits under the `Processor`, so it also applies to bytes
//! held back by a synchronized update.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink, KeyboardModes,
    KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, PrivateMode, Rgb,
    ScpCharPath, ScpUpdateMode, StandardCharset, TabulationClearMode,
};

/// Emoji presentation selector.
const VS16: char = '\u{FE0F}';

/// `Term` seen through vte's [`Handler`], widening `X U+FE0F` to two cells.
pub(crate) struct PresentationHandler<'a, T> {
    term: &'a mut Term<T>,
}

impl<'a, T: EventListener> PresentationHandler<'a, T> {
    pub(crate) fn new(term: &'a mut Term<T>) -> Self {
        Self { term }
    }

    /// After `U+FE0F` joined the glyph just written: when that glyph is
    /// narrow and the next cell is on the same row, write a blank there and
    /// make the pair a wide glyph and its spacer, the cursor after both. A
    /// glyph in the last column stays narrow (the host would have wrapped
    /// it first; the program's next move puts things right).
    fn widen_last(&mut self) {
        let cursor = &self.term.grid().cursor;
        if cursor.input_needs_wrap || cursor.point.column.0 == 0 {
            return;
        }
        let (line, base) = (cursor.point.line, cursor.point.column - 1);
        let cell = &self.term.grid()[line][base];
        let joined = cell.zerowidth().is_some_and(|z| z.last() == Some(&VS16));
        if !joined
            || cell
                .flags
                .intersects(Flags::WIDE_CHAR | Flags::WIDE_CHAR_SPACER)
        {
            return;
        }
        // Through `Term`, so the blank takes the pen, clears any wide glyph
        // it lands on, shifts in insert mode and marks the row damaged.
        Handler::input(&mut *self.term, ' ');
        let row = &mut self.term.grid_mut()[line];
        row[base].flags.insert(Flags::WIDE_CHAR);
        row[base + Column(1)].flags.insert(Flags::WIDE_CHAR_SPACER);
    }
}

/// Forwards each listed [`Handler`] method to `Term`'s.
macro_rules! forward {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            fn $name(&mut self, $($arg: $ty),*) {
                Handler::$name(&mut *self.term, $($arg),*);
            }
        )*
    };
}

impl<T: EventListener> Handler for PresentationHandler<'_, T> {
    fn input(&mut self, c: char) {
        Handler::input(&mut *self.term, c);
        if c == VS16 {
            self.widen_last();
        }
    }

    // Every other method of vte 0.15's `Handler`: one missing here would
    // fall back to the trait's empty default and silently drop a sequence,
    // so the list must follow vte when alacritty_terminal is upgraded.
    // (`set_mouse_cursor_icon` is left out: `Term` keeps the default.)
    forward! {
        set_title(a0: Option<String>);
        set_cursor_style(a0: Option<CursorStyle>);
        set_cursor_shape(a0: CursorShape);
        goto(a0: i32, a1: usize);
        goto_line(a0: i32);
        goto_col(a0: usize);
        insert_blank(a0: usize);
        move_up(a0: usize);
        move_down(a0: usize);
        identify_terminal(a0: Option<char>);
        device_status(a0: usize);
        move_forward(a0: usize);
        move_backward(a0: usize);
        move_down_and_cr(a0: usize);
        move_up_and_cr(a0: usize);
        put_tab(a0: u16);
        backspace();
        carriage_return();
        linefeed();
        bell();
        substitute();
        newline();
        set_horizontal_tabstop();
        scroll_up(a0: usize);
        scroll_down(a0: usize);
        insert_blank_lines(a0: usize);
        delete_lines(a0: usize);
        erase_chars(a0: usize);
        delete_chars(a0: usize);
        move_backward_tabs(a0: u16);
        move_forward_tabs(a0: u16);
        save_cursor_position();
        restore_cursor_position();
        clear_line(a0: LineClearMode);
        clear_screen(a0: ClearMode);
        clear_tabs(a0: TabulationClearMode);
        set_tabs(a0: u16);
        reset_state();
        reverse_index();
        terminal_attribute(a0: Attr);
        set_mode(a0: Mode);
        unset_mode(a0: Mode);
        report_mode(a0: Mode);
        set_private_mode(a0: PrivateMode);
        unset_private_mode(a0: PrivateMode);
        report_private_mode(a0: PrivateMode);
        set_scrolling_region(a0: usize, a1: Option<usize>);
        set_keypad_application_mode();
        unset_keypad_application_mode();
        set_active_charset(a0: CharsetIndex);
        configure_charset(a0: CharsetIndex, a1: StandardCharset);
        set_color(a0: usize, a1: Rgb);
        dynamic_color_sequence(a0: String, a1: usize, a2: &str);
        reset_color(a0: usize);
        clipboard_store(a0: u8, a1: &[u8]);
        clipboard_load(a0: u8, a1: &str);
        decaln();
        push_title();
        pop_title();
        text_area_size_pixels();
        text_area_size_chars();
        set_hyperlink(a0: Option<Hyperlink>);
        report_keyboard_mode();
        push_keyboard_mode(a0: KeyboardModes);
        pop_keyboard_modes(a0: u16);
        set_keyboard_mode(a0: KeyboardModes, a1: KeyboardModesApplyBehavior);
        set_modify_other_keys(a0: ModifyOtherKeys);
        report_modify_other_keys();
        set_scp(a0: ScpCharPath, a1: ScpUpdateMode);
    }
}

#[cfg(test)]
mod tests;
