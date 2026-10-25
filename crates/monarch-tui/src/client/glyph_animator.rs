//! Animates the sidebar's agent-state glyphs: a slowly pulsing dot while
//! working, a diamond fading between dark yellow and brown while waiting,
//! and a check that brightens once when an agent finishes.
//!
//! Pure: the caller passes the time, so frames are testable. Only glyph
//! cells change between frames, so an animation frame costs the host a
//! few bytes.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use monarch_core::{AgentState, PaneId, Theme};

use crate::GlyphLook;

/// Time between animation frames.
pub const ANIMATION_STEP: Duration = Duration::from_millis(100);
/// One full pulse of the working dot, small to large and back.
const PULSE: Duration = Duration::from_millis(1600);
/// One fade of the waiting diamond, there and back.
const FADE: Duration = Duration::from_millis(2000);
/// How long a finished check takes to reach full brightness.
const SETTLE: Duration = Duration::from_millis(1200);
/// The working dot, smallest to largest.
const DOTS: [char; 4] = ['·', '•', '●', '•'];

/// What each shown pane's agent is doing and since when.
#[derive(Debug, Clone)]
pub struct GlyphAnimator {
    epoch: Instant,
    seen: BTreeMap<PaneId, (AgentState, Instant)>,
}

impl GlyphAnimator {
    /// An animator whose shared pulse starts at `now`.
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self {
            epoch: now,
            seen: BTreeMap::new(),
        }
    }

    /// Records the shown panes' states; a changed state restarts its
    /// one-off animation, and panes not listed are forgotten.
    pub fn observe(&mut self, panes: impl IntoIterator<Item = (PaneId, AgentState)>, now: Instant) {
        let mut seen = BTreeMap::new();
        for (pane, state) in panes {
            let since = match self.seen.get(&pane) {
                Some(&(old, since)) if old == state => since,
                _ => now,
            };
            seen.insert(pane, (state, since));
        }
        self.seen = seen;
    }

    /// Whether a later frame would look different.
    #[must_use]
    pub fn moving(&self, now: Instant) -> bool {
        self.seen.values().any(|&(state, since)| match state {
            AgentState::Working | AgentState::Blocked => true,
            // One frame past the end, so the settled look is drawn.
            AgentState::Done => now.duration_since(since) <= SETTLE + ANIMATION_STEP,
            _ => false,
        })
    }

    /// How `pane`'s glyph looks at `now`: in `theme`'s colours (none for
    /// `NO_COLOR`), animated or at rest.
    #[must_use]
    pub fn look(
        &self,
        pane: PaneId,
        now: Instant,
        theme: Option<&Theme>,
        animate: bool,
    ) -> GlyphLook {
        let Some(&(state, since)) = self.seen.get(&pane) else {
            return GlyphLook::plain(AgentState::Unknown.glyph());
        };
        let run = now.duration_since(self.epoch);
        let mut look = GlyphLook::plain(state.glyph());
        let (color, bold) = match (state, theme) {
            (AgentState::Working, theme) => {
                let wave = if animate { wave(run, PULSE) } else { 1.0 };
                if animate {
                    look.symbol = DOTS[phase(run, PULSE, DOTS.len())];
                }
                (
                    theme.map(|t| t.working.lerp(dim(t.working), 1.0 - wave)),
                    false,
                )
            }
            (AgentState::Blocked, theme) => {
                let wave = if animate { wave(run, FADE) } else { 0.0 };
                (theme.map(|t| t.blocked_from.lerp(t.blocked_to, wave)), true)
            }
            (AgentState::Done, theme) => {
                let t = if animate {
                    fraction(now.duration_since(since), SETTLE)
                } else {
                    1.0
                };
                (
                    theme.map(|th| th.done_from.lerp(th.done_to, t)),
                    t >= 1.0 || theme.is_none(),
                )
            }
            (AgentState::Error, theme) => (theme.map(|t| t.error), true),
            _ => (None, false),
        };
        look.color = color;
        look.bold = bold;
        look
    }
}

/// Half brightness of `c`, the pulse's low point.
fn dim(c: monarch_core::Rgb) -> monarch_core::Rgb {
    c.lerp(monarch_core::Rgb::new(0, 0, 0), 0.5)
}

/// 0 → 1 → 0 over each `period`.
fn wave(run: Duration, period: Duration) -> f32 {
    let x = fraction_of_cycle(run, period);
    1.0 - (2.0 * x - 1.0).abs()
}

/// Which of `n` equal steps of `period` `run` falls in.
fn phase(run: Duration, period: Duration, n: usize) -> usize {
    // A fraction in 0..1 times a small count, so the cast cannot overflow.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let step = (fraction_of_cycle(run, period) * n as f32) as usize;
    step.min(n - 1)
}

/// How far into the current `period` `run` is, 0..1.
fn fraction_of_cycle(run: Duration, period: Duration) -> f32 {
    let period_ms = period.as_millis().max(1);
    #[allow(clippy::cast_precision_loss)]
    let x = (run.as_millis() % period_ms) as f32 / period_ms as f32;
    x
}

/// `elapsed` as a fraction of `total`, capped at 1.
fn fraction(elapsed: Duration, total: Duration) -> f32 {
    (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animator(state: AgentState) -> (GlyphAnimator, Instant) {
        let start = Instant::now();
        let mut animator = GlyphAnimator::new(start);
        animator.observe([(PaneId(1), state)], start);
        (animator, start)
    }

    fn at(animator: &GlyphAnimator, start: Instant, ms: u64) -> GlyphLook {
        let theme = Theme::dark();
        animator.look(
            PaneId(1),
            start + Duration::from_millis(ms),
            Some(&theme),
            true,
        )
    }

    #[test]
    fn working_pulses_small_to_large_and_back() {
        let (animator, start) = animator(AgentState::Working);
        let symbols: Vec<char> = [0, 400, 800, 1200, 1600]
            .map(|ms| at(&animator, start, ms).symbol)
            .into();
        assert_eq!(symbols, ['·', '•', '●', '•', '·']);
        let (dim, bright) = (
            at(&animator, start, 0).color,
            at(&animator, start, 800).color,
        );
        assert_eq!(bright, Some(Theme::dark().working));
        assert_ne!(dim, bright);
        assert!(animator.moving(start));
    }

    #[test]
    fn waiting_fades_from_yellow_to_brown_and_back() {
        let (animator, start) = animator(AgentState::Blocked);
        let theme = Theme::dark();
        assert_eq!(at(&animator, start, 0).color, Some(theme.blocked_from));
        assert_eq!(at(&animator, start, 1000).color, Some(theme.blocked_to));
        assert_eq!(at(&animator, start, 2000).color, Some(theme.blocked_from));
        assert!(at(&animator, start, 500).bold);
    }

    #[test]
    fn done_brightens_once_then_rests_bold() {
        let (mut animator, start) = animator(AgentState::Done);
        let theme = Theme::dark();
        let first = at(&animator, start, 0);
        assert_eq!(
            (first.symbol, first.color, first.bold),
            ('✓', Some(theme.done_from), false)
        );
        let settled = at(&animator, start, 1500);
        assert_eq!((settled.color, settled.bold), (Some(theme.done_to), true));
        assert!(
            !animator.moving(start + Duration::from_millis(1500)),
            "stops redrawing"
        );
        animator.observe(
            [(PaneId(1), AgentState::Done)],
            start + Duration::from_secs(5),
        );
        assert!(
            at(&animator, start, 5000).bold,
            "an unchanged state keeps its start"
        );
    }

    #[test]
    fn still_and_colourless_modes() {
        let (animator, start) = animator(AgentState::Working);
        let still = animator.look(PaneId(1), start, Some(&Theme::dark()), false);
        assert_eq!(
            (still.symbol, still.color),
            ('●', Some(Theme::dark().working))
        );
        let plain = animator.look(PaneId(1), start, None, true);
        assert_eq!((plain.symbol, plain.color), ('·', None));
        let error = animator.look(PaneId(9), start, None, true);
        assert_eq!(error, GlyphLook::plain('$'), "unknown panes are shells");
        let (idle, _) = self::animator(AgentState::Idle);
        assert!(!idle.moving(start), "idle panes never redraw");
    }
}
