//! Parses one complete escape sequence the host sent in answer to a probe.

use monarch_proto::Rgb;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// An answer to one of [`super::HostProbe`]'s queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostReply {
    /// OSC 10/11/12 colour report: `slot` is 10 (fg), 11 (bg) or 12 (cursor).
    Color {
        /// Which OSC slot was reported.
        slot: u16,
        /// The colour.
        rgb: Rgb,
    },
    /// `CSI 6 ; height ; width t`: cell size in pixels.
    CellSize {
        /// Width in pixels.
        width: u16,
        /// Height in pixels.
        height: u16,
    },
    /// DECRQM report for mode 2026 (synchronized output).
    SyncOutput(bool),
    /// `CSI ? flags u`: the host speaks the kitty keyboard protocol.
    KittyFlags(u8),
    /// Primary device attributes (`CSI ? … c`): the end of the probe.
    DeviceAttributes,
}

impl HostReply {
    /// Parses a complete sequence starting with `ESC`; `None` if it is not
    /// a probe reply (for example a key the user pressed meanwhile).
    #[must_use]
    pub fn parse(seq: &[u8]) -> Option<Self> {
        match seq {
            [ESC, b']', body @ ..] => Self::parse_osc(osc_body(body)?),
            [ESC, b'[', body @ ..] => Self::parse_csi(body),
            _ => None,
        }
    }

    fn parse_osc(body: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(body).ok()?;
        let (num, value) = text.split_once(';')?;
        let slot: u16 = num.parse().ok()?;
        if !(10..=12).contains(&slot) {
            return None;
        }
        Some(Self::Color {
            slot,
            rgb: parse_x11_color(value)?,
        })
    }

    fn parse_csi(body: &[u8]) -> Option<Self> {
        let (&last, rest) = body.split_last()?;
        let text = std::str::from_utf8(rest).ok()?;
        match (last, text.strip_prefix('?')) {
            (b'c', Some(_)) => Some(Self::DeviceAttributes),
            (b'u', Some(flags)) => flags.parse().ok().map(Self::KittyFlags),
            (b'y', Some(report)) => parse_decrqm(report),
            (b't', None) => parse_cell_size(text),
            _ => None,
        }
    }
}

/// Strips the BEL or ST terminator from an OSC body.
fn osc_body(body: &[u8]) -> Option<&[u8]> {
    body.strip_suffix(&[BEL])
        .or_else(|| body.strip_suffix(&[ESC, b'\\']))
}

/// `2026;Ps$`: Ps 1 (set) or 2 (reset) mean supported; 0 and 4 mean not.
fn parse_decrqm(report: &str) -> Option<HostReply> {
    let (mode, value) = report.strip_suffix('$')?.split_once(';')?;
    (mode == "2026").then_some(HostReply::SyncOutput(matches!(value, "1" | "2")))
}

fn parse_cell_size(text: &str) -> Option<HostReply> {
    let mut parts = text.split(';');
    if parts.next()? != "6" {
        return None;
    }
    let height = parts.next()?.parse().ok()?;
    let width = parts.next()?.parse().ok()?;
    Some(HostReply::CellSize { width, height })
}

/// `rgb:R/G/B` with 1–4 hex digits per component, scaled to 8 bits.
fn parse_x11_color(value: &str) -> Option<Rgb> {
    let mut parts = value.strip_prefix("rgb:")?.split('/');
    let mut next = || scale_component(parts.next()?);
    let rgb = Rgb::new(next()?, next()?, next()?);
    parts.next().is_none().then_some(rgb)
}

fn scale_component(hex: &str) -> Option<u8> {
    if hex.is_empty() || hex.len() > 4 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    let max = (1u32 << (4 * hex.len())) - 1;
    u8::try_from((value * 255 + max / 2) / max).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_reports_with_either_terminator() {
        let bg = Some(HostReply::Color {
            slot: 11,
            rgb: Rgb::new(0x28, 0x2c, 0x34),
        });
        assert_eq!(HostReply::parse(b"\x1b]11;rgb:2828/2c2c/3434\x1b\\"), bg);
        assert_eq!(HostReply::parse(b"\x1b]11;rgb:28/2c/34\x07"), bg);
    }

    #[test]
    fn short_components_are_scaled() {
        let white = HostReply::parse(b"\x1b]10;rgb:f/f/f\x07");
        assert_eq!(
            white,
            Some(HostReply::Color {
                slot: 10,
                rgb: Rgb::new(255, 255, 255)
            })
        );
    }

    #[test]
    fn csi_replies() {
        assert_eq!(
            HostReply::parse(b"\x1b[6;20;10t"),
            Some(HostReply::CellSize {
                width: 10,
                height: 20
            })
        );
        assert_eq!(
            HostReply::parse(b"\x1b[?2026;2$y"),
            Some(HostReply::SyncOutput(true))
        );
        assert_eq!(
            HostReply::parse(b"\x1b[?2026;0$y"),
            Some(HostReply::SyncOutput(false))
        );
        assert_eq!(
            HostReply::parse(b"\x1b[?1u"),
            Some(HostReply::KittyFlags(1))
        );
        assert_eq!(
            HostReply::parse(b"\x1b[?62;22c"),
            Some(HostReply::DeviceAttributes)
        );
    }

    #[test]
    fn keys_and_garbage_are_not_replies() {
        for seq in [
            &b"\x1b[A"[..],
            b"\x1b[13;2u",
            b"\x1b[1;5c",
            b"\x1b]4;1;rgb:0/0/0\x07",
            b"\x1b]11;rgb:zz/0/0\x07",
            b"\x1b]11;rgb:0/0\x07",
            b"\x1b[?1049;1$y",
            b"\x1b[8;24;80t",
            b"x",
        ] {
            assert_eq!(HostReply::parse(seq), None, "{seq:?}");
        }
    }
}
