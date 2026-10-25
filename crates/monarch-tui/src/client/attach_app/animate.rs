//! Animating the sidebar's agent glyphs: dressing each row with its look
//! for this frame, and asking for frames only while one is moving.

use std::time::Instant;

use monarch_core::{Rgb, Theme, ThemeConfig};

use super::AttachApp;
use crate::SidebarGroup;
use crate::client::{ANIMATION_STEP, GlyphAnimator};

/// The app's animation state: what is animating, in which colours, and
/// when the next frame is due.
#[derive(Debug)]
pub(super) struct Glyphs {
    animator: GlyphAnimator,
    /// `None` under `NO_COLOR`.
    theme: Option<Theme>,
    /// When the next animation frame is due.
    pub(super) next_frame: Instant,
}

impl Glyphs {
    /// The colours in use; `None` under `NO_COLOR`.
    pub(super) fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }

    /// Colours chosen by `config` for a host with `background`.
    pub(super) fn new(background: Option<Rgb>, config: &ThemeConfig) -> Self {
        let now = Instant::now();
        Self {
            animator: GlyphAnimator::new(now),
            theme: theme_for(background, config),
            next_frame: now,
        }
    }

    /// Switches to the colours `config` now chooses.
    pub(super) fn retheme(&mut self, background: Option<Rgb>, config: &ThemeConfig) {
        self.theme = theme_for(background, config);
    }
}

impl AttachApp {
    /// Requests a frame when a glyph would change: animations are on, the
    /// sidebar is shown and the mouse is Monarch's (frames while the host
    /// terminal owns the mouse could disturb its native selection).
    pub(super) fn animate(&mut self) {
        if self.animate_welcome() {
            return;
        }
        if !self.opts.animations || self.passthrough || self.state.left() == 0 {
            return;
        }
        let now = Instant::now();
        let glyphs = &mut self.glyphs;
        if now >= glyphs.next_frame && glyphs.animator.moving(now) {
            self.dirty = true;
            glyphs.next_frame = now + ANIMATION_STEP;
        }
    }

    /// Sets every row's glyph look for this frame.
    pub(super) fn dress(&mut self, groups: &mut [SidebarGroup]) {
        let now = Instant::now();
        let glyphs = &mut self.glyphs;
        let rows = groups.iter().flat_map(|g| &g.rows);
        glyphs
            .animator
            .observe(rows.map(|r| (r.pane, r.state)), now);
        let animate = self.opts.animations && !self.passthrough;
        for row in groups.iter_mut().flat_map(|g| &mut g.rows) {
            let look = glyphs
                .animator
                .look(row.pane, now, glyphs.theme.as_ref(), animate);
            row.look = Some(look);
        }
    }
}

/// The theme `config` picks for a host with `background`, or none under
/// `NO_COLOR`. Its warnings were shown when the config was read.
fn theme_for(background: Option<Rgb>, config: &ThemeConfig) -> Option<Theme> {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    (!no_color).then(|| config.resolve(background).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_follows_the_host_background() {
        if std::env::var_os("NO_COLOR").is_some() {
            return;
        }
        let config = ThemeConfig::default();
        let light = theme_for(Some(Rgb::new(255, 255, 255)), &config);
        assert_eq!(light, Some(Theme::light()));
        assert_eq!(theme_for(None, &config), Some(Theme::dark()));
        let fixed = ThemeConfig {
            auto_switch: false,
            name: "catppuccin-latte".into(),
            ..config
        };
        assert_eq!(theme_for(None, &fixed), Theme::named("catppuccin-latte"));
    }
}
