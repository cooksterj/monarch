//! Which session names are allowed: the one rule the server's socket path
//! and the client's session picker share.

/// Whether `name` can name a session: letters, digits, `-`, `_` and `.`,
/// not empty and not starting with a dot (it becomes a file name).
#[must_use]
pub fn is_session_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_pass_paths_and_spaces_do_not() {
        for ok in ["main", "work-2", "feat_x", "v1.2"] {
            assert!(is_session_name(ok), "{ok}");
        }
        for bad in ["", ".hidden", "../x", "a b", "a/b", "é"] {
            assert!(!is_session_name(bad), "{bad:?}");
        }
    }
}
