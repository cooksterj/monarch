//! A key pressed after the prefix.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Direction};

/// The command key that follows the prefix: a printable character (Shift
/// already applied, so `H` is Shift+h) or a named key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Key {
    /// A printable character, including space.
    Char(char),
    /// An arrow key.
    Arrow(Direction),
    /// Tab.
    Tab,
    /// Enter.
    Enter,
    /// Escape.
    Esc,
}

impl Key {
    /// The key a single input byte names, if it names one.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'\t' => Some(Self::Tab),
            b'\r' | b'\n' => Some(Self::Enter),
            0x1b => Some(Self::Esc),
            b' '..=b'~' => Some(Self::Char(char::from(byte))),
            _ => None,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(' ') => f.write_str("space"),
            Self::Char(c) => write!(f, "{c}"),
            Self::Arrow(d) => f.write_str(d.name()),
            Self::Tab => f.write_str("tab"),
            Self::Enter => f.write_str("enter"),
            Self::Esc => f.write_str("esc"),
        }
    }
}

impl FromStr for Key {
    type Err = CoreError;

    /// A single character (`%`, `H`), `shift+h`, or a name: `left`,
    /// `right`, `up`, `down`, `tab`, `enter`, `esc`, `space`, `minus`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || CoreError::UnknownKey(text.to_owned());
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(Self::Char(c));
        }
        let lower = text.trim().to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("shift+") {
            let mut chars = rest.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    Ok(Self::Char(c.to_ascii_uppercase()))
                }
                _ => Err(bad()),
            };
        }
        if let Some(dir) = Direction::ALL.into_iter().find(|d| d.name() == lower) {
            return Ok(Self::Arrow(dir));
        }
        match lower.as_str() {
            "tab" => Ok(Self::Tab),
            "enter" | "return" => Ok(Self::Enter),
            "esc" | "escape" => Ok(Self::Esc),
            "space" => Ok(Self::Char(' ')),
            "minus" => Ok(Self::Char('-')),
            _ => Err(bad()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_from_characters_and_names() {
        assert_eq!("%".parse::<Key>().ok(), Some(Key::Char('%')));
        assert_eq!("H".parse::<Key>().ok(), Some(Key::Char('H')));
        assert_eq!("shift+h".parse::<Key>().ok(), Some(Key::Char('H')));
        assert_eq!(
            "Left".parse::<Key>().ok(),
            Some(Key::Arrow(Direction::Left))
        );
        assert_eq!("minus".parse::<Key>().ok(), Some(Key::Char('-')));
        assert_eq!("space".parse::<Key>().ok(), Some(Key::Char(' ')));
        for bad in ["", "ctrl+x", "shift+1", "f13", "ab"] {
            assert!(bad.parse::<Key>().is_err(), "{bad}");
        }
    }

    #[test]
    fn display_round_trips() {
        for key in [
            Key::Char('x'),
            Key::Char(' '),
            Key::Arrow(Direction::Up),
            Key::Tab,
            Key::Esc,
        ] {
            assert_eq!(key.to_string().parse::<Key>().ok(), Some(key));
        }
    }

    #[test]
    fn bytes_map_to_keys() {
        assert_eq!(Key::from_byte(b'%'), Some(Key::Char('%')));
        assert_eq!(Key::from_byte(b'\r'), Some(Key::Enter));
        assert_eq!(
            Key::from_byte(0x01),
            None,
            "other control bytes are not keys"
        );
    }
}
