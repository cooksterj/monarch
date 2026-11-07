//! How long an agent has been in its state, in a few characters.

use std::time::Duration;

/// `age` as the sidebar and inbox show it: seconds under a minute
/// (`42s`), then whole minutes (`3m`), hours (`2h`) and days (`1d`).
/// Short so it fits at the right of a row; coarse so it rarely changes.
#[must_use]
pub fn age_label(age: Duration) -> String {
    let secs = age.as_secs();
    match secs {
        0..60 => format!("{secs}s"),
        60..3_600 => format!("{}m", secs / 60),
        3_600..86_400 => format!("{}h", secs / 3_600),
        _ => format!("{}d", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_down_to_the_coarsest_unit() {
        let cases = [
            (0, "0s"),
            (42, "42s"),
            (59, "59s"),
            (60, "1m"),
            (3 * 60 + 59, "3m"),
            (59 * 60 + 59, "59m"),
            (3_600, "1h"),
            (2 * 3_600 + 3_599, "2h"),
            (86_400, "1d"),
            (10 * 86_400 + 5, "10d"),
        ];
        for (secs, want) in cases {
            assert_eq!(age_label(Duration::from_secs(secs)), want, "{secs}s");
        }
        assert_eq!(age_label(Duration::from_millis(999)), "0s", "sub-second");
    }
}
