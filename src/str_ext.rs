//! String extension traits.

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
        } else {
            self
        }
    }
}

/// An extension trait for strings that adds a sentence capitalization method.
pub(crate) trait Capitalize<'a> {
    /// Capitalize words using ASCII uppercase/lowercase.
    fn capitalize_words(self) -> Cow<'a, str>;
}

impl<'a> Capitalize<'a> for Cow<'a, str> {
    fn capitalize_words(self) -> Cow<'a, str> {
        let words: String = self
            .split_word_bounds()
            .map(|word| {
                let mut graphemes = word.graphemes(true);

                if let Some(mut s) = graphemes.next().map(|ch| ch.to_uppercase()) {
                    s.push_str(&graphemes.as_str().to_lowercase());

                    s
                } else {
                    "".to_string()
                }
            })
            .collect();

        Cow::from(words)
    }
}

impl<'a> Capitalize<'a> for &'a str {
    fn capitalize_words(self) -> Cow<'a, str> {
        Cow::from(self).capitalize_words()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test the Ellipsis trait.
    #[test]
    fn test_ellipsis() {
        let s = Cow::from("Small");
        assert_eq!(s.ellipsis(25), Cow::from("Small"));

        let s = Cow::from("The number of graphemes in this string is too damn high!");
        assert_eq!(s.ellipsis(25), Cow::from("The number of grapheme..."));
    }

    /// Test the Ellipsis minimum string length.
    #[test]
    fn test_ellipsis_min() {
        for expected in ["", "A", "AB", "ABC", "ABCD"] {
            let s = Cow::from(expected);
            assert_eq!(s.clone().ellipsis(0), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(1), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(2), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(3), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(4), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(5), Cow::from(expected));
        }

        let s = Cow::from("The number of graphemes in this string is too damn high!");
        assert_eq!(s.clone().ellipsis(0), Cow::from("T..."));
        assert_eq!(s.clone().ellipsis(1), Cow::from("T..."));
        assert_eq!(s.clone().ellipsis(2), Cow::from("T..."));
        assert_eq!(s.clone().ellipsis(3), Cow::from("T..."));
        assert_eq!(s.clone().ellipsis(4), Cow::from("T..."));
        assert_eq!(s.clone().ellipsis(5), Cow::from("Th..."));
    }

    /// Test the Capitalize trait.
    #[test]
    fn test_capitalize_words() {
        assert_eq!(
            Cow::from("YOU KNOW, I FIND THAT I (ALWAYS) SHOUT A LOT! SORRY!").capitalize_words(),
            Cow::from("You Know, I Find That I (Always) Shout A Lot! Sorry!"),
        );
    }
}
