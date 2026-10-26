//! The built-in themes: Monarch's own, plus Catppuccin, Tokyo Night and
//! Gruvbox, each in a dark and a light variant. State and diff colours
//! come from each palette's blue, yellow, orange, green, red and cyan.

use super::Theme;
use crate::Rgb;

/// Shorthand for the tables below.
const fn c(rgb: u32) -> Rgb {
    Rgb::hex(rgb)
}

/// A real monarch's orange, black and white, which every built-in
/// theme's butterflies share.
const WING: Rgb = c(0xe8761b);
const WING_EDGE: Rgb = c(0x1e1612);
const WING_SPOT: Rgb = c(0xf5f0e6);

/// Monarch's own theme for dark hosts: monarch-butterfly orange accents.
pub(super) const MONARCH_DARK: Theme = Theme {
    bar: c(0x2b2d3a),
    bar_text: c(0xd0d3e0),
    accent: c(0xe8a33d),
    accent_text: c(0x1a1a1a),
    working: c(0x5fafff),
    blocked_from: c(0xc8a000),
    blocked_to: c(0x7b4f1d),
    done_from: c(0xa8e6a3),
    done_to: c(0x2ee65b),
    error: c(0xff5f5f),
    added: c(0x7ed37e),
    removed: c(0xf07178),
    hunk: c(0x6fb3d2),
    wing: WING,
    wing_edge: WING_EDGE,
    wing_spot: WING_SPOT,
};

/// Monarch's own theme for light hosts, darker so it stays readable.
pub(super) const MONARCH_LIGHT: Theme = Theme {
    bar: c(0xe4e2dc),
    bar_text: c(0x2b2b2b),
    accent: c(0xc25e00),
    accent_text: c(0xffffff),
    working: c(0x1f6fd1),
    blocked_from: c(0xa67c00),
    blocked_to: c(0x5c3a12),
    done_from: c(0x6cbf6a),
    done_to: c(0x0a8a2a),
    error: c(0xc62828),
    added: c(0x1a7f37),
    removed: c(0xb31d28),
    hunk: c(0x0550ae),
    wing: WING,
    wing_edge: WING_EDGE,
    wing_spot: WING_SPOT,
};

/// One palette's colours, in the order [`from`] reads them.
struct Palette {
    surface: u32,
    text: u32,
    accent: u32,
    base: u32,
    blue: u32,
    yellow: u32,
    orange: u32,
    green: u32,
    green_soft: u32,
    red: u32,
    cyan: u32,
}

/// A theme from a palette: the bar on its surface colour, the accent on
/// its base, states and diffs in its hues.
const fn from(p: &Palette) -> Theme {
    Theme {
        bar: c(p.surface),
        bar_text: c(p.text),
        accent: c(p.accent),
        accent_text: c(p.base),
        working: c(p.blue),
        blocked_from: c(p.yellow),
        blocked_to: c(p.orange),
        done_from: c(p.green_soft),
        done_to: c(p.green),
        error: c(p.red),
        added: c(p.green),
        removed: c(p.red),
        hunk: c(p.cyan),
        wing: WING,
        wing_edge: WING_EDGE,
        wing_spot: WING_SPOT,
    }
}

/// Every built-in theme by name; the first two are the defaults.
pub(super) const THEMES: [(&str, Theme); 8] = [
    ("monarch-dark", MONARCH_DARK),
    ("monarch-light", MONARCH_LIGHT),
    (
        "catppuccin-mocha",
        from(&Palette {
            surface: 0x313244,
            text: 0xcdd6f4,
            accent: 0xcba6f7,
            base: 0x1e1e2e,
            blue: 0x89b4fa,
            yellow: 0xf9e2af,
            orange: 0xfab387,
            green: 0xa6e3a1,
            green_soft: 0xd2f1d0,
            red: 0xf38ba8,
            cyan: 0x89dceb,
        }),
    ),
    (
        "catppuccin-latte",
        from(&Palette {
            surface: 0xccd0da,
            text: 0x4c4f69,
            accent: 0x8839ef,
            base: 0xeff1f5,
            blue: 0x1e66f5,
            yellow: 0xdf8e1d,
            orange: 0xfe640b,
            green: 0x40a02b,
            green_soft: 0x8cc67f,
            red: 0xd20f39,
            cyan: 0x04a5e5,
        }),
    ),
    (
        "tokyonight-night",
        from(&Palette {
            surface: 0x292e42,
            text: 0xc0caf5,
            accent: 0xbb9af7,
            base: 0x1a1b26,
            blue: 0x7aa2f7,
            yellow: 0xe0af68,
            orange: 0xff9e64,
            green: 0x9ece6a,
            green_soft: 0xc8e3a8,
            red: 0xf7768e,
            cyan: 0x7dcfff,
        }),
    ),
    (
        "tokyonight-day",
        from(&Palette {
            surface: 0xc4c8da,
            text: 0x3760bf,
            accent: 0x9854f1,
            base: 0xe1e2e7,
            blue: 0x2e7de9,
            yellow: 0x8c6c3e,
            orange: 0xb15c00,
            green: 0x587539,
            green_soft: 0x8fa872,
            red: 0xf52a65,
            cyan: 0x007197,
        }),
    ),
    (
        "gruvbox-dark",
        from(&Palette {
            surface: 0x3c3836,
            text: 0xebdbb2,
            accent: 0xfe8019,
            base: 0x282828,
            blue: 0x83a598,
            yellow: 0xfabd2f,
            orange: 0xd65d0e,
            green: 0xb8bb26,
            green_soft: 0xd5d78a,
            red: 0xfb4934,
            cyan: 0x8ec07c,
        }),
    ),
    (
        "gruvbox-light",
        from(&Palette {
            surface: 0xebdbb2,
            text: 0x3c3836,
            accent: 0xaf3a03,
            base: 0xfbf1c7,
            blue: 0x076678,
            yellow: 0xb57614,
            orange: 0xaf3a03,
            green: 0x79740e,
            green_soft: 0xa8a35c,
            red: 0x9d0006,
            cyan: 0x427b58,
        }),
    ),
];
