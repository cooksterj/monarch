//! A 24-bit colour.

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

    /// Whether this colour reads as a light background (perceived luminance
    /// above one half), used for automatic light/dark theme switching.
    #[must_use]
    pub fn is_light(self) -> bool {
        let lum =
            0.2126 * f64::from(self.r) + 0.7152 * f64::from(self.g) + 0.0722 * f64::from(self.b);
        lum > 127.5
    }

    /// The X11 `rgb:rrrr/gggg/bbbb` form used in OSC 10/11/4 replies.
    #[must_use]
    pub fn to_x11(self) -> String {
        let w = |c: u8| u16::from(c) * 0x101;
        format!("rgb:{:04x}/{:04x}/{:04x}", w(self.r), w(self.g), w(self.b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_form_scales_to_16_bits() {
        assert_eq!(Rgb::new(0xff, 0x00, 0x80).to_x11(), "rgb:ffff/0000/8080");
    }

    #[test]
    fn lightness() {
        assert!(Rgb::new(250, 250, 250).is_light());
        assert!(!Rgb::new(20, 20, 30).is_light());
    }
}
