//! Cutting a line of text to the columns it has, the way the sidebar does.

use std::borrow::Cow;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// `text` as it fits in `width` columns: whole when it fits, else cut
/// and ended with `…`. The cut falls between words when one ends in the
/// second half of the room, so `Found 4 call sites` becomes
/// `Found 4 call…`, not `Found 4 call s…`; a long word (a path) is cut
/// mid-word. A room too small to keep any of it gets nothing, not a lone
/// `…`. Cut each frame at the width drawn into, so a wider sidebar shows
/// more and the `…` goes once the text fits.
pub(crate) fn fit_text(text: &str, width: usize) -> Cow<'_, str> {
    if text.width() <= width {
        return Cow::Borrowed(text);
    }
    let Some(room) = width.checked_sub(1) else {
        return Cow::Borrowed("");
    };
    let mut end = 0;
    let mut used = 0;
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > room {
            break;
        }
        used += w;
        end = i + c.len_utf8();
    }
    let cut = &text[..end];
    let at_boundary = text[end..].starts_with(char::is_whitespace);
    let word = cut
        .rfind(char::is_whitespace)
        .map(|i| cut[..i].trim_end())
        .filter(|kept| !at_boundary && kept.width() * 2 >= room);
    let kept = word.unwrap_or_else(|| cut.trim_end());
    if kept.is_empty() {
        return Cow::Borrowed("");
    }
    Cow::Owned(format!("{kept}…"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_that_fits_is_left_alone() {
        assert!(matches!(
            fit_text("Grep TODO", 9),
            Cow::Borrowed("Grep TODO")
        ));
        assert_eq!(fit_text("Grep TODO", 40), "Grep TODO");
        assert_eq!(fit_text("", 0), "");
    }

    #[test]
    fn cuts_fall_between_words_and_end_in_an_ellipsis() {
        assert_eq!(
            fit_text("Found 4 call sites of login_retry", 14),
            "Found 4 call…"
        );
        assert_eq!(
            fit_text("Found 4 call sites", 13),
            "Found 4 call…",
            "at a boundary"
        );
        let fitted = fit_text("Allow Bash cargo test -p auth login?", 20);
        assert_eq!(fitted, "Allow Bash cargo…");
        assert!(fitted.width() <= 20);
    }

    #[test]
    fn long_words_and_narrow_rooms_are_cut_mid_word() {
        assert_eq!(fit_text("Read src/auth/session.rs", 14), "Read src/auth…");
        assert_eq!(fit_text("supercalifragilistic", 6), "super…");
        assert_eq!(fit_text("Grep TODO", 2), "G…");
        assert_eq!(fit_text("Grep TODO", 1), "", "no lone ellipsis");
        assert_eq!(fit_text("Grep TODO", 0), "");
    }

    #[test]
    fn wide_characters_count_as_two_columns() {
        let fitted = fit_text("修正 認証 テスト", 8);
        assert!(fitted.width() <= 8, "{fitted}");
        assert!(fitted.ends_with('…'));
        assert_eq!(fit_text("修正", 4), "修正");
    }
}
