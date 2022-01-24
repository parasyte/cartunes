//! String extension traits.

use std::borrow::Cow;
use std::cmp::Ordering;
use unicode_segmentation::UnicodeSegmentation;

/// An extension trait for strings that adds a truncation method with ellipses.
pub(crate) trait Ellipsis<'a> {
    /// String truncation with ellipsis.
    fn ellipsis(self, max_length: usize) -> Cow<'a, str>;
}

impl<'a> Ellipsis<'a> for Cow<'a, str> {
    fn ellipsis(self, max_length: usize) -> Cow<'a, str> {
        const ELLIPSIS: &str = "…";
        const MIN_LENGTH: usize = 2;

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

impl<'a> Ellipsis<'a> for &'a str {
    fn ellipsis(self, max_length: usize) -> Cow<'a, str> {
        Cow::Borrowed(self).ellipsis(max_length)
    }
}

/// An extension trait for strings that adds a sentence capitalization method.
pub(crate) trait Capitalize<'a> {
    /// Capitalize words using ASCII uppercase/lowercase.
    fn capitalize_words(self) -> String;
}

impl<'a> Capitalize<'a> for &'a str {
    fn capitalize_words(self) -> String {
        self.split_word_bounds()
            .map(|word| {
                let mut graphemes = word.graphemes(true);

                if let Some(mut s) = graphemes.next().map(|ch| ch.to_uppercase()) {
                    s.push_str(&graphemes.as_str().to_lowercase());

                    s
                } else {
                    "".to_string()
                }
            })
            .collect()
    }
}

/// An extension trait for strings that adds "human sort" comparison methods.
pub(crate) trait HumanCompare {
    fn human_compare(&self, other: &str) -> Ordering;
}

impl HumanCompare for String {
    fn human_compare(&self, other: &str) -> Ordering {
        human_compare(self, other)
    }
}

impl HumanCompare for &String {
    fn human_compare(&self, other: &str) -> Ordering {
        human_compare(self, other)
    }
}

impl HumanCompare for &str {
    fn human_compare(&self, other: &str) -> Ordering {
        human_compare(self, other)
    }
}

fn human_compare(a: &str, b: &str) -> Ordering {
    if a.starts_with('-') && b.starts_with('-') {
        // Reverse parameter order when comparing negative numbers
        human_sort::compare(b, a)
    } else {
        human_sort::compare(a, b)
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
        assert_eq!(s.ellipsis(25), Cow::from("The number of graphemes …"));
    }

    /// Test the Ellipsis minimum string length.
    #[test]
    fn test_ellipsis_min() {
        for expected in ["", "A", "AB"] {
            let s = Cow::from(expected);
            assert_eq!(s.clone().ellipsis(0), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(1), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(2), Cow::from(expected));
            assert_eq!(s.clone().ellipsis(3), Cow::from(expected));
        }

        let s = Cow::from("The number of graphemes in this string is too damn high!");
        assert_eq!(s.clone().ellipsis(0), Cow::from("T…"));
        assert_eq!(s.clone().ellipsis(1), Cow::from("T…"));
        assert_eq!(s.clone().ellipsis(2), Cow::from("T…"));
        assert_eq!(s.clone().ellipsis(3), Cow::from("Th…"));
    }

    /// Test the Capitalize trait.
    #[test]
    fn test_capitalize_words() {
        assert_eq!(
            Cow::from("YOU KNOW, I FIND THAT I (ALWAYS) SHOUT A LOT! SORRY!").capitalize_words(),
            Cow::from("You Know, I Find That I (Always) Shout A Lot! Sorry!"),
        );
    }

    #[test]
    fn test_human_compare_text() {
        assert_eq!("a".human_compare("b"), Ordering::Less);
        assert_eq!("ab".human_compare("abc"), Ordering::Less);
        assert_eq!("abc".human_compare("abc"), Ordering::Equal);
    }

    #[test]
    fn test_human_compare_numbers() {
        assert_eq!("1".human_compare("1"), Ordering::Equal);
        assert_eq!("10".human_compare("10"), Ordering::Equal);
        assert_eq!("1".human_compare("10"), Ordering::Less);
        assert_eq!("10".human_compare("1"), Ordering::Greater);

        assert_eq!("1".human_compare("2"), Ordering::Less);
        assert_eq!("10".human_compare("2"), Ordering::Greater);
        assert_eq!("1".human_compare("-2"), Ordering::Greater);
        assert_eq!("10".human_compare("-2"), Ordering::Greater);
        assert_eq!("-1".human_compare("2"), Ordering::Less);
        assert_eq!("-10".human_compare("2"), Ordering::Less);
        assert_eq!("-1".human_compare("-2"), Ordering::Greater);
        assert_eq!("-10".human_compare("-2"), Ordering::Less);
    }

    #[test]
    #[ignore = "Fractions are not yet supported"]
    fn test_human_compare_fractions() {
        assert_eq!("3/8".human_compare("1/2"), Ordering::Less);
        assert_eq!("5/8".human_compare("1/2"), Ordering::Greater);
    }
}
