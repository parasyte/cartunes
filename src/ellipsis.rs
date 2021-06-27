use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;

/// An extension trait for strings that adds a truncation method with ellipses.
pub(crate) trait Ellipsis<'a> {
    /// String truncation with ellipsis.
    fn ellipsis(self, max_length: usize) -> Cow<'a, str>;
}

impl<'a> Ellipsis<'a> for Cow<'a, str> {
    fn ellipsis(self, max_length: usize) -> Cow<'a, str> {
        // XXX: It would be nice to use Unicode ellipsis "…", but it is not supported by my font.
        const ELLIPSIS: &str = "...";
        const MIN_LENGTH: usize = 4;

        let max_length = max_length.max(MIN_LENGTH);
        let graphemes = self.graphemes(true);
        let size_hint = graphemes.size_hint();
        let size_hint = size_hint.1.unwrap_or(size_hint.0);

        if size_hint > max_length {
            let mut s = graphemes
                .take(max_length - (MIN_LENGTH - 1))
                .collect::<String>();
            s.push_str(ELLIPSIS);

            Cow::Owned(s)
        } else if size_hint == 0 {
            Cow::Borrowed(ELLIPSIS)
        } else {
            self
        }
    }
}
