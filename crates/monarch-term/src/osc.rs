//! An OSC (operating system command) sequence captured from pane output.

/// A captured `ESC ] <number> ; <payload> (BEL | ESC \)` sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Osc {
    /// The command number (9 notification, 7 cwd, 133 prompt mark, …).
    pub number: u32,
    /// Everything after the first `;`, raw bytes (usually UTF-8).
    pub payload: Vec<u8>,
}

impl Osc {
    /// OSC numbers the tap captures because the engine does not handle
    /// them: cwd (7), notifications (9, 99, 777), prompt marks (133) and
    /// the agent status protocol (7501).
    pub const CAPTURED: [u32; 6] = [7, 9, 99, 133, 777, 7501];

    /// Parses the raw body between `ESC ]` and the terminator. Returns
    /// `None` when the number is missing, malformed or not captured.
    #[must_use]
    pub fn parse(body: &[u8]) -> Option<Self> {
        let split = body.iter().position(|&b| b == b';').unwrap_or(body.len());
        let digits = &body[..split];
        if digits.is_empty() || digits.len() > 5 || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let number = digits
            .iter()
            .fold(0u32, |n, d| n * 10 + u32::from(d - b'0'));
        if !Self::CAPTURED.contains(&number) {
            return None;
        }
        let payload = body.get(split + 1..).unwrap_or_default().to_vec();
        Some(Self { number, payload })
    }

    /// The payload as text, replacing invalid UTF-8.
    #[must_use]
    pub fn payload_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notification() {
        let osc = Osc::parse(b"9;Build finished").expect("captured");
        assert_eq!(osc.number, 9);
        assert_eq!(osc.payload_str(), "Build finished");
    }

    #[test]
    fn number_without_payload() {
        let osc = Osc::parse(b"133").expect("captured");
        assert!(osc.payload.is_empty());
    }

    #[test]
    fn ignores_uncaptured_and_malformed() {
        assert_eq!(Osc::parse(b"0;title"), None);
        assert_eq!(Osc::parse(b"52;c;aGk="), None);
        assert_eq!(Osc::parse(b";9"), None);
        assert_eq!(Osc::parse(b"9x;hi"), None);
        assert_eq!(Osc::parse(b""), None);
        assert_eq!(Osc::parse(b"999999;x"), None);
    }
}
