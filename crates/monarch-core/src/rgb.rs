//! A 24-bit colour.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A 24-bit RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// A colour from components.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// A colour from `0xrrggbb`.
    #[must_use]
    pub const fn hex(rgb: u32) -> Self {
        let [_, r, g, b] = rgb.to_be_bytes();
        Self { r, g, b }
    }

    /// Whether this colour reads as a light background (perceived luminance
    /// above one half), used for automatic light/dark theme switching.
    #[must_use]
    pub fn is_light(self) -> bool {
        let lum =
            0.2126 * f64::from(self.r) + 0.7152 * f64::from(self.g) + 0.0722 * f64::from(self.b);
        lum > 127.5
    }

    /// The colour `t` of the way from `self` to `to` (`t` in 0..=1).
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| {
            let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
            // Clamped to 0..=255 above, so the cast cannot truncate.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = v.round() as u8;
            v
        };
        Self::new(mix(self.r, to.r), mix(self.g, to.g), mix(self.b, to.b))
    }

    /// The X11 `rgb:rrrr/gggg/bbbb` form used in OSC 10/11/4 replies.
    #[must_use]
    pub fn to_x11(self) -> String {
        let w = |c: u8| u16::from(c) * 0x101;
        format!("rgb:{:04x}/{:04x}/{:04x}", w(self.r), w(self.g), w(self.b))
    }
}

impl FromStr for Rgb {
    type Err = String;

    /// Reads `#rrggbb`; the error says what was wrong.
    fn from_str(text: &str) -> Result<Self, String> {
        let digits = text.strip_prefix('#').unwrap_or_default();
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{text:?} is not a #rrggbb colour"));
        }
        u32::from_str_radix(digits, 16)
            .map(Self::hex)
            .map_err(|e| e.to_string())
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colours_parse_and_print() {
        assert_eq!(Rgb::hex(0x12_ab_ef), Rgb::new(0x12, 0xab, 0xef));
        assert_eq!("#12abEF".parse(), Ok(Rgb::new(0x12, 0xab, 0xef)));
        assert_eq!(Rgb::new(1, 2, 255).to_string(), "#0102ff");
        for bad in ["12abef", "#12abe", "#12abeg", "#+2abef"] {
            assert!(bad.parse::<Rgb>().is_err(), "{bad}");
        }
    }

    #[test]
    fn x11_form_scales_to_16_bits() {
        assert_eq!(Rgb::new(0xff, 0x00, 0x80).to_x11(), "rgb:ffff/0000/8080");
    }

    #[test]
    fn lerp_runs_from_one_colour_to_the_other() {
        let (a, b) = (Rgb::new(0, 100, 200), Rgb::new(200, 100, 0));
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), Rgb::new(100, 100, 100));
        assert_eq!(a.lerp(b, 7.0), b, "clamped");
    }

    #[test]
    fn lightness() {
        assert!(Rgb::new(250, 250, 250).is_light());
        assert!(!Rgb::new(20, 20, 30).is_light());
    }
}
