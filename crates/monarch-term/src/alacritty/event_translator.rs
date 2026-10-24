//! Turns queued alacritty events into [`EngineEvent`]s, answering colour and
//! size queries from the pane's own colours or the host profile.

use alacritty_terminal::event::{Event, WindowSize};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::Rgb as AlacRgb;
use monarch_proto::ScreenSize;

use crate::{EngineEvent, HostProfile};

/// What the translator needs to know about the pane right now.
pub(crate) struct TranslateContext<'a> {
    /// Colours the program inside the pane has overridden (OSC 4/10/11).
    pub(crate) pane_colors: &'a Colors,
    /// Host colours and cell size.
    pub(crate) host: &'a HostProfile,
    /// Current grid size.
    pub(crate) size: ScreenSize,
}

/// Stateless event translation.
pub(crate) struct EventTranslator;

impl EventTranslator {
    /// Translates one event; `None` for events Monarch does not act on
    /// (wakeups, cursor blink changes, clipboard loads — OSC 52 reads are
    /// disabled by design).
    pub(crate) fn translate(event: Event, ctx: &TranslateContext<'_>) -> Option<EngineEvent> {
        match event {
            Event::PtyWrite(text) => Some(EngineEvent::Reply(text.into_bytes())),
            Event::Title(title) => Some(EngineEvent::Title(Some(title))),
            Event::ResetTitle => Some(EngineEvent::Title(None)),
            Event::Bell => Some(EngineEvent::Bell),
            Event::ClipboardStore(_, text) => Some(EngineEvent::ClipboardStore(text)),
            Event::ColorRequest(index, format) => {
                let rgb = ctx.pane_colors[index].unwrap_or_else(|| {
                    let c = ctx.host.palette.get(index);
                    AlacRgb {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                    }
                });
                Some(EngineEvent::Reply(format(rgb).into_bytes()))
            }
            Event::TextAreaSizeRequest(format) => {
                let size = WindowSize {
                    num_lines: ctx.size.rows,
                    num_cols: ctx.size.cols,
                    cell_width: ctx.host.cell_width_px,
                    cell_height: ctx.host.cell_height_px,
                };
                Some(EngineEvent::Reply(format(size).into_bytes()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::Palette;
    use monarch_proto::Rgb;

    fn host() -> HostProfile {
        HostProfile {
            palette: Palette {
                background: Rgb::new(0x10, 0x20, 0x30),
                ..Palette::default()
            },
            ..HostProfile::default()
        }
    }

    fn translate(event: Event, colors: &Colors) -> Option<EngineEvent> {
        let host = host();
        let ctx = TranslateContext {
            pane_colors: colors,
            host: &host,
            size: ScreenSize::new(80, 24),
        };
        EventTranslator::translate(event, &ctx)
    }

    #[test]
    fn colour_query_uses_host_background_when_pane_has_not_set_one() {
        let format = Arc::new(|c: AlacRgb| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b));
        let reply = translate(Event::ColorRequest(257, format), &Colors::default());
        assert_eq!(reply, Some(EngineEvent::Reply(b"102030".to_vec())));
    }

    #[test]
    fn colour_query_prefers_pane_override() {
        let mut colors = Colors::default();
        colors[257] = Some(AlacRgb { r: 1, g: 1, b: 1 });
        let format = Arc::new(|c: AlacRgb| format!("{}", c.r));
        assert_eq!(
            translate(Event::ColorRequest(257, format), &colors),
            Some(EngineEvent::Reply(b"1".to_vec()))
        );
    }

    #[test]
    fn size_query_uses_grid_and_cell_pixels() {
        let format =
            Arc::new(|s: WindowSize| format!("{}x{}@{}", s.num_cols, s.num_lines, s.cell_width));
        let reply = translate(Event::TextAreaSizeRequest(format), &Colors::default());
        assert_eq!(reply, Some(EngineEvent::Reply(b"80x24@8".to_vec())));
    }

    #[test]
    fn ignores_wakeups() {
        assert_eq!(translate(Event::Wakeup, &Colors::default()), None);
    }
}
