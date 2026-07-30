//! Byte-safe operations on text that came from somewhere else.
//!
//! Everything Tome truncates is text it did not write: a crawled page, a
//! `robots.txt`, rendered markdown on its way to a model. Rust's `&s[..n]`
//! **panics** when `n` lands inside a multi-byte character, and documentation
//! is full of em dashes, curly quotes, accented names and CJK — so for any
//! sufficiently long input the odds that an arbitrary byte offset is a
//! character boundary are not much better than even.
//!
//! This module exists because that bug was written twice: once in
//! `fetch::robots`, where it was caught and fixed with a comment explaining
//! the trap, and again in the MCP page budget three files away (S3-4, found by
//! S4-1's bug hunt). A private helper with a good comment did not stop the
//! second one. A shared function has a chance.

/// Truncate to at most `max` bytes, never splitting a UTF-8 character.
///
/// Returns the longest prefix that is both within the budget and valid — so
/// the result may be up to three bytes shorter than `max`, and is `""` only
/// if the very first character is longer than the budget.
pub fn truncate_at_char_boundary(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn short_input_is_returned_whole() {
        assert_eq!(truncate_at_char_boundary("hello", 64), "hello");
        assert_eq!(truncate_at_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn a_boundary_inside_a_character_walks_back() {
        // `—` is three bytes at 3..6. Cutting at 4 or 5 would panic.
        let text = "abc—def";
        assert_eq!(truncate_at_char_boundary(text, 4), "abc");
        assert_eq!(truncate_at_char_boundary(text, 5), "abc");
        assert_eq!(truncate_at_char_boundary(text, 6), "abc—");
    }

    #[test]
    fn a_boundary_inside_the_first_character_yields_nothing() {
        // Degenerate, and the only case that returns empty. Reported as empty
        // rather than as a partial character, which would not be a `str`.
        assert_eq!(truncate_at_char_boundary("—abc", 2), "");
    }

    #[test]
    fn every_offset_of_a_mixed_string_is_safe() {
        // The property, over the whole range: no offset panics, and the
        // result is always a prefix.
        let text = "a—b😀c\u{0301}d";
        for n in 0..=text.len() + 4 {
            let cut = truncate_at_char_boundary(text, n);
            assert!(text.starts_with(cut));
            assert!(cut.len() <= n.min(text.len()));
        }
    }
}
