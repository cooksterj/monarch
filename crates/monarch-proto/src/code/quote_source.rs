//! Where a quoted screen selection was found in the pane's checkout.

use serde::{Deserialize, Serialize};

/// The lines of a checkout's file that a selection on screen shows, found
/// by the server so an ask can name `path:lines` instead of a pane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteSource {
    /// The file, from the checkout's top.
    pub path: String,
    /// Its language (`rust`), to tag the quote's fence.
    pub language: Option<String>,
    /// The first line's number, counting from 1.
    pub start: usize,
    /// The file's lines as they are on disk, indentation kept.
    pub lines: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_survive_the_attach_encoding() {
        let source = QuoteSource {
            path: "src/a.rs".into(),
            language: Some("rust".into()),
            start: 42,
            lines: vec!["fn a() {".into(), "}".into()],
        };
        let bytes = postcard::to_stdvec(&source).expect("encode");
        assert_eq!(
            postcard::from_bytes::<QuoteSource>(&bytes).ok(),
            Some(source)
        );
        let empty = postcard::to_stdvec(&QuoteSource::default()).expect("encode");
        assert!(
            postcard::from_bytes::<QuoteSource>(&empty[..1]).is_err(),
            "cut short"
        );
    }
}
