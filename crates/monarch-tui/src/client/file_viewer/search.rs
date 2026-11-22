//! Searching the file viewer (`/`, then `n` and `N`): typing the query,
//! finding the next line that has it, and where it is in a line so the
//! view can mark it. Case-insensitive unless the query has a capital.

use super::FileViewer;
use crate::OverlayKey;

impl FileViewer {
    /// `/`, `n`, `N`, and every key while a query is being typed; `false`
    /// for other keys.
    pub(super) fn search_key(&mut self, key: OverlayKey) -> bool {
        let Some(typed) = self.typing.as_mut() else {
            match key {
                OverlayKey::Char('/') => self.start_search(),
                OverlayKey::Char(c @ ('n' | 'N')) => self.find(c == 'n'),
                _ => return false,
            }
            return true;
        };
        match key {
            OverlayKey::Char(c) => typed.push(c),
            OverlayKey::Backspace => {
                typed.pop();
            }
            OverlayKey::Enter => {
                let query = self.typing.take().unwrap_or_default();
                if !query.is_empty() {
                    self.query = Some(query);
                    self.find(true);
                }
            }
            OverlayKey::Esc | OverlayKey::Ctrl('c') => self.typing = None,
            _ => {}
        }
        true
    }

    /// `/`: starts typing a query.
    fn start_search(&mut self) {
        self.typing = Some(String::new());
        self.status = None;
    }

    /// Moves to the next line (`forward`) or the one before that has the
    /// query, wrapping round the file; says so when no line has it.
    fn find(&mut self, forward: bool) {
        let (Some(query), Some(Ok(file))) = (self.query.as_deref(), &self.file) else {
            return;
        };
        let len = file.lines.len();
        let found = (1..=len)
            .map(|step| {
                if forward {
                    (self.cursor + step) % len
                } else {
                    (self.cursor + len - step % len) % len
                }
            })
            .find(|&i| !matches(&file.lines[i].text(), query).is_empty());
        match found {
            Some(line) => {
                self.status = None;
                self.go_to_line(line);
            }
            None => self.status = Some(format!("no line has {query:?}")),
        }
    }

    /// The query being typed, if one is.
    #[must_use]
    pub fn typing(&self) -> Option<&str> {
        self.typing.as_deref()
    }

    /// The last query searched for.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Where the query is in `text`, as character ranges, to mark.
    #[must_use]
    pub fn matches_in(&self, text: &str) -> Vec<(usize, usize)> {
        self.query
            .as_deref()
            .map_or_else(Vec::new, |q| matches(text, q))
    }
}

/// Each place `query` is in `text`, as `(start, end)` character indexes;
/// any case unless `query` has a capital.
fn matches(text: &str, query: &str) -> Vec<(usize, usize)> {
    let fold = !query.chars().any(char::is_uppercase);
    let norm = |c: char| {
        if fold {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        }
    };
    let hay: Vec<char> = text.chars().map(norm).collect();
    let needle: Vec<char> = query.chars().map(norm).collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == needle[..] {
            found.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn matching_ignores_case_unless_the_query_has_a_capital() {
        assert_eq!(matches("Foo foo FOO", "foo"), [(0, 3), (4, 7), (8, 11)]);
        assert_eq!(matches("Foo foo FOO", "Foo"), [(0, 3)]);
        assert_eq!(matches("aaaa", "aa"), [(0, 2), (2, 4)], "no overlaps");
        assert_eq!(matches("é café", "café"), [(2, 6)], "characters, not bytes");
        assert!(matches("short", "longer than it").is_empty());
        assert!(matches("x", "").is_empty());
    }
}
