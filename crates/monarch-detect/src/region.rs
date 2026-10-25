//! Which part of a pane a rule reads: some lines of the screen, a part
//! found by its shape (a prompt box, the text after a rule line), or the
//! OSC title or progress.

mod lines;
mod prompt;

use std::fmt;
use std::str::FromStr;

use crate::DetectionInput;

/// A region, as named in a manifest (`bottom_non_empty_lines(8)`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Region {
    /// The whole screen.
    #[default]
    WholeRecent,
    /// The last `n` lines.
    BottomLines(usize),
    /// From the `n`th-last non-blank line to the end.
    BottomNonEmptyLines(usize),
    /// From the start to the `n`th non-blank line (engine version 3).
    TopNonEmptyLines(usize),
    /// After the last Codex `›` prompt line (all of it if none).
    AfterLastPromptMarker,
    /// Before the current Codex prompt (all of it if none).
    BeforeCurrentPromptMarker,
    /// The screen, or nothing while a Codex prompt is current.
    WholeRecentWithoutCurrentPromptMarker,
    /// The last block marker line (`•`, `■`, `✗`, `✓`) above the prompt.
    CurrentPromptBlockMarker,
    /// From that block marker to the end.
    AfterCurrentPromptBlockMarker,
    /// Inside the prompt box (between the last two `─` rules).
    PromptBoxBody,
    /// Everything above the prompt box (all of it if none).
    AbovePromptBox,
    /// The last non-blank line above the prompt box.
    LastNonEmptyAbovePromptBox,
    /// After the last `─` rule line.
    AfterLastHorizontalRule,
    /// The OSC window title.
    OscTitle,
    /// The OSC 9;4 progress payload.
    OscProgress,
}

/// The regions named without a count.
const NAMED: [(&str, Region); 12] = [
    ("whole_recent", Region::WholeRecent),
    ("after_last_prompt_marker", Region::AfterLastPromptMarker),
    (
        "before_current_prompt_marker",
        Region::BeforeCurrentPromptMarker,
    ),
    (
        "whole_recent_without_current_prompt_marker",
        Region::WholeRecentWithoutCurrentPromptMarker,
    ),
    (
        "current_prompt_block_marker",
        Region::CurrentPromptBlockMarker,
    ),
    (
        "after_current_prompt_block_marker",
        Region::AfterCurrentPromptBlockMarker,
    ),
    ("prompt_box_body", Region::PromptBoxBody),
    ("above_prompt_box", Region::AbovePromptBox),
    (
        "last_non_empty_above_prompt_box",
        Region::LastNonEmptyAbovePromptBox,
    ),
    (
        "after_last_horizontal_rule",
        Region::AfterLastHorizontalRule,
    ),
    ("osc_title", Region::OscTitle),
    ("osc_progress", Region::OscProgress),
];

/// The largest count `top_non_empty_lines` accepts.
const MAX_TOP_LINES: usize = u16::MAX as usize;

impl Region {
    /// The engine version that first understood this region.
    #[must_use]
    pub fn engine_version(self) -> u32 {
        match self {
            Self::TopNonEmptyLines(_) => 3,
            _ => 1,
        }
    }

    /// The text this region covers in `input`.
    #[must_use]
    pub fn slice<'a>(self, input: DetectionInput<'a>) -> &'a str {
        let screen = input.screen;
        match self {
            Self::OscTitle => input.osc_title,
            Self::OscProgress => input.osc_progress,
            Self::WholeRecent => screen,
            Self::BottomLines(n) => lines::bottom(screen, n),
            Self::BottomNonEmptyLines(n) => lines::bottom_non_empty(screen, n),
            Self::TopNonEmptyLines(n) => lines::top_non_empty(screen, n),
            Self::AfterLastHorizontalRule => lines::after_last_rule(screen),
            Self::AfterLastPromptMarker => prompt::after_last_marker(screen),
            Self::BeforeCurrentPromptMarker => prompt::before_current(screen),
            Self::WholeRecentWithoutCurrentPromptMarker => prompt::unless_current(screen),
            Self::CurrentPromptBlockMarker => prompt::block_marker(screen),
            Self::AfterCurrentPromptBlockMarker => prompt::after_block_marker(screen),
            Self::PromptBoxBody => prompt::box_body(screen),
            Self::AbovePromptBox => prompt::above_box(screen),
            Self::LastNonEmptyAbovePromptBox => lines::last_non_empty(prompt::above_box(screen)),
        }
    }
}

impl FromStr for Region {
    type Err = String;

    /// Reads a region name; the error is the name it couldn't read.
    fn from_str(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if let Some((_, region)) = NAMED.iter().find(|(name, _)| *name == spec) {
            return Ok(*region);
        }
        if let Some(n) = counted(spec, "bottom_lines") {
            return Ok(Self::BottomLines(n));
        }
        if let Some(n) = counted(spec, "bottom_non_empty_lines") {
            return Ok(Self::BottomNonEmptyLines(n));
        }
        top_count(spec)
            .map(Self::TopNonEmptyLines)
            .ok_or_else(|| spec.to_owned())
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BottomLines(n) => write!(f, "bottom_lines({n})"),
            Self::BottomNonEmptyLines(n) => write!(f, "bottom_non_empty_lines({n})"),
            Self::TopNonEmptyLines(n) => write!(f, "top_non_empty_lines({n})"),
            named => {
                let name = NAMED.iter().find(|(_, r)| r == named).map(|(n, _)| *n);
                f.write_str(name.unwrap_or("?"))
            }
        }
    }
}

/// The count in `name(count)`.
fn counted(spec: &str, name: &str) -> Option<usize> {
    spec.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?
        .parse()
        .ok()
}

/// A `top_non_empty_lines` count: written plainly (no sign, no leading
/// zero), positive, and at most [`MAX_TOP_LINES`].
fn top_count(spec: &str) -> Option<usize> {
    let count = spec
        .strip_prefix("top_non_empty_lines(")?
        .strip_suffix(')')?;
    if count.starts_with('0') || !count.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    count.parse().ok().filter(|n| *n <= MAX_TOP_LINES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for spec in [
            "whole_recent",
            "bottom_lines(3)",
            "bottom_non_empty_lines(8)",
            "top_non_empty_lines(20)",
            "after_last_prompt_marker",
            "last_non_empty_above_prompt_box",
            "osc_title",
            "osc_progress",
        ] {
            let region: Region = spec.parse().expect(spec);
            assert_eq!(region.to_string(), spec);
        }
        assert_eq!(" osc_title ".parse(), Ok(Region::OscTitle), "trimmed");
    }

    #[test]
    fn bad_names_and_counts_are_refused() {
        for spec in [
            "everything",
            "bottom_lines(x)",
            "bottom_lines(3",
            "top_non_empty_lines(0)",
            "top_non_empty_lines(07)",
            "top_non_empty_lines(+7)",
            "top_non_empty_lines(65536)",
        ] {
            assert_eq!(spec.parse::<Region>(), Err(spec.to_owned()));
        }
        assert_eq!(
            "top_non_empty_lines(65535)".parse(),
            Ok(Region::TopNonEmptyLines(65_535))
        );
    }

    #[test]
    fn osc_regions_read_their_own_inputs() {
        let input = DetectionInput {
            screen: "a\nb",
            osc_title: "⠋ claude",
            osc_progress: "4;3",
        };
        assert_eq!(Region::OscTitle.slice(input), "⠋ claude");
        assert_eq!(Region::OscProgress.slice(input), "4;3");
        assert_eq!(Region::WholeRecent.slice(input), "a\nb");
        assert_eq!(Region::BottomLines(1).slice(input), "b");
        assert_eq!(Region::TopNonEmptyLines(1).engine_version(), 3);
        assert_eq!(Region::WholeRecent.engine_version(), 1);
    }
}
