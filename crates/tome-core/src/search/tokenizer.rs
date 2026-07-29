//! A code-aware tokenizer (P2-002).
//!
//! The default tokenizer splits on non-alphanumerics, so `read_to_string`
//! becomes `read`, `to`, `string` and a search for the whole identifier no
//! longer matches it as a unit; `getUserName` stays one token and a search for
//! `user` misses it entirely. Documentation search needs both behaviours, so
//! this tokenizer emits **both**: the identifier whole, and its parts.
//!
//! ```text
//! read_to_string  ->  read_to_string  read  to  string
//! getUserName     ->  getUserName     get   User  Name
//! HTTPServer      ->  HTTPServer      HTTP  Server
//! Vec::new        ->  Vec  new                     (no sub-parts to add)
//! utf8            ->  utf8                         (digits stay attached)
//! ```
//!
//! Case folding is **not** done here — [`super::SearchEngine`] composes this
//! with Tantivy's `LowerCaser`. Doing it in one place is what keeps the
//! indexing and query paths identical.
//!
//! # Positions
//!
//! The whole identifier is emitted at the same position as its first part,
//! with `position_length` spanning all of them. That is what makes phrase
//! queries work in both directions: `"read to string"` matches, and so does a
//! phrase containing `read_to_string` as a single term. Naively emitting every
//! token at its own increasing position would break the first; emitting them
//! all at one position would break the second.

use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Longest token emitted. Anything longer is skipped entirely rather than
/// truncated — a 200-character "identifier" is minified output or a base64
/// blob, and half of one is not a useful search term.
///
/// Tantivy's own default is 40 bytes (`RemoveLongFilter`), which is too short
/// for real code: `serde::de::DeserializeOwned` is 28, and generated bindings
/// routinely exceed 40.
const MAX_TOKEN_BYTES: usize = 120;

/// Splits identifiers on `_`, `-`, and camelCase boundaries, emitting the
/// whole identifier alongside its parts.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodeTokenizer;

impl Tokenizer for CodeTokenizer {
    type TokenStream<'a> = CodeTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let mut tokens = Vec::new();
        let mut position = 0usize;

        for (word_start, word) in words(text) {
            let parts = split_identifier(word);

            // One part means the identifier has no internal structure, so the
            // "whole" token and the single part are the same string. Emitting
            // it twice would double its term frequency and quietly skew
            // scoring towards unstructured identifiers.
            if parts.len() > 1 {
                push(
                    &mut tokens,
                    word,
                    word_start,
                    0,
                    word.len(),
                    position,
                    parts.len(),
                );
            }

            for (i, &(start, end)) in parts.iter().enumerate() {
                push(&mut tokens, word, word_start, start, end, position + i, 1);
            }

            position += parts.len();
        }

        CodeTokenStream {
            remaining: tokens.into_iter(),
            current: Token::default(),
        }
    }
}

fn push(
    tokens: &mut Vec<Token>,
    word: &str,
    word_start: usize,
    start: usize,
    end: usize,
    position: usize,
    position_length: usize,
) {
    let Some(text) = word.get(start..end) else {
        // Unreachable for ranges produced by `split_identifier`, which are all
        // char boundaries. Skipping rather than slicing blind is what keeps a
        // future change to that function from panicking on multi-byte input.
        return;
    };
    if text.is_empty() || text.len() > MAX_TOKEN_BYTES {
        return;
    }
    tokens.push(Token {
        offset_from: word_start + start,
        offset_to: word_start + end,
        position,
        text: text.to_owned(),
        position_length,
    });
}

/// Split text into identifier-shaped runs: alphanumerics plus `_` and `-`.
///
/// Returns each run with its byte offset in `text`. Everything else (`::`,
/// `.`, `(`, whitespace) is a separator.
fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            if start.is_none() {
                start = Some(idx);
            }
        } else if let Some(s) = start.take() {
            out.push((s, &text[s..idx]));
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// Byte ranges of `word`'s sub-parts, splitting on `_`/`-` and camelCase.
///
/// Every returned range starts and ends on a char boundary: they come from
/// `char_indices`, never from arithmetic on byte lengths. Getting this wrong
/// is how a tokenizer panics the first time it meets a non-ASCII identifier,
/// which documentation absolutely contains.
fn split_identifier(word: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = word.char_indices().collect();
    let mut parts = Vec::new();
    let mut start: Option<usize> = None;
    let mut prev: Option<char> = None;

    for i in 0..chars.len() {
        let (idx, ch) = chars[i];

        if ch == '_' || ch == '-' {
            if let Some(s) = start.take() {
                parts.push((s, idx));
            }
            prev = Some(ch);
            continue;
        }

        let boundary = match prev {
            Some(p) if p != '_' && p != '-' => {
                // `foo|Bar` and `utf8|Encoder`: a lowercase letter or digit
                // followed by an uppercase one.
                let camel = (p.is_lowercase() || p.is_numeric()) && ch.is_uppercase();
                // `HTTP|Server`: the last capital of an acronym run belongs to
                // the following word, not to the acronym.
                let acronym = p.is_uppercase()
                    && ch.is_uppercase()
                    && chars
                        .get(i + 1)
                        .is_some_and(|(_, next)| next.is_lowercase());
                camel || acronym
            }
            _ => false,
        };

        if boundary {
            if let Some(s) = start.take() {
                parts.push((s, idx));
            }
        }
        if start.is_none() {
            start = Some(idx);
        }
        prev = Some(ch);
    }

    if let Some(s) = start {
        parts.push((s, word.len()));
    }
    parts
}

/// Precomputed tokens, handed out one at a time.
///
/// The tokens are materialised up front rather than produced lazily because
/// emitting a whole identifier *and* its parts needs to know how many parts
/// there are before it can set the whole token's `position_length`. Holding a
/// `current` token — rather than an index into the vector — is what lets
/// `token`/`token_mut` be total functions with no panicking path, which the
/// workspace lints require of library code.
pub struct CodeTokenStream {
    remaining: std::vec::IntoIter<Token>,
    current: Token,
}

impl TokenStream for CodeTokenStream {
    fn advance(&mut self) -> bool {
        match self.remaining.next() {
            Some(token) => {
                self.current = token;
                true
            }
            None => false,
        }
    }

    fn token(&self) -> &Token {
        &self.current
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn tokens(text: &str) -> Vec<String> {
        let mut tokenizer = CodeTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut out = Vec::new();
        while stream.advance() {
            out.push(stream.token().text.clone());
        }
        out
    }

    fn positions(text: &str) -> Vec<(String, usize, usize)> {
        let mut tokenizer = CodeTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut out = Vec::new();
        while stream.advance() {
            let t = stream.token();
            out.push((t.text.clone(), t.position, t.position_length));
        }
        out
    }

    #[test]
    fn snake_case_emits_whole_and_parts() {
        assert_eq!(
            tokens("read_to_string"),
            ["read_to_string", "read", "to", "string"]
        );
    }

    #[test]
    fn camel_case_splits_on_case_change() {
        assert_eq!(
            tokens("getUserName"),
            ["getUserName", "get", "User", "Name"]
        );
    }

    #[test]
    fn acronym_keeps_its_run_together() {
        assert_eq!(tokens("HTTPServer"), ["HTTPServer", "HTTP", "Server"]);
        assert_eq!(
            tokens("parseJSONValue"),
            ["parseJSONValue", "parse", "JSON", "Value"]
        );
    }

    #[test]
    fn unstructured_identifier_is_emitted_once() {
        // Emitting `Vec` twice would double its term frequency and skew
        // scoring toward identifiers that happen to have no internal
        // structure.
        assert_eq!(tokens("Vec"), ["Vec"]);
        assert_eq!(tokens("Vec::new"), ["Vec", "new"]);
    }

    #[test]
    fn digits_stay_attached_to_their_word() {
        assert_eq!(tokens("utf8"), ["utf8"]);
        assert_eq!(tokens("base64Encode"), ["base64Encode", "base64", "Encode"]);
    }

    #[test]
    fn positions_let_phrases_match_both_ways() {
        // The whole identifier shares the first part's position and spans all
        // of them, so `"read to string"` and a phrase containing
        // `read_to_string` as one term both match.
        assert_eq!(
            positions("read_to_string"),
            [
                ("read_to_string".to_owned(), 0, 3),
                ("read".to_owned(), 0, 1),
                ("to".to_owned(), 1, 1),
                ("string".to_owned(), 2, 1),
            ]
        );
    }

    #[test]
    fn separate_words_advance_position() {
        let got = positions("Vec::new");
        assert_eq!(got[0].1, 0);
        assert_eq!(got[1].1, 1);
    }

    #[test]
    fn offsets_point_into_the_original_text() {
        let mut tokenizer = CodeTokenizer;
        let text = "let x = read_to_string(p);";
        let mut stream = tokenizer.token_stream(text);
        while stream.advance() {
            let t = stream.token();
            assert_eq!(
                text.get(t.offset_from..t.offset_to),
                Some(t.text.as_str()),
                "offsets must slice back to the token text"
            );
        }
    }

    #[test]
    fn multi_byte_input_does_not_panic_and_slices_cleanly() {
        // The corrective case: byte arithmetic here would panic mid-character.
        for text in [
            "naïve_case",
            "Ünicode",
            "日本語のテキスト",
            "emoji🎉Boundary",
            "café_auLait",
        ] {
            let mut tokenizer = CodeTokenizer;
            let mut stream = tokenizer.token_stream(text);
            while stream.advance() {
                let t = stream.token();
                assert_eq!(text.get(t.offset_from..t.offset_to), Some(t.text.as_str()));
            }
        }
    }

    #[test]
    fn overlong_tokens_are_dropped_not_truncated() {
        let long = "a".repeat(MAX_TOKEN_BYTES + 1);
        assert!(tokens(&long).is_empty());
        // A long identifier's short parts still survive, which is what makes
        // dropping the whole token acceptable.
        let structured = format!("{}_short", "b".repeat(MAX_TOKEN_BYTES + 1));
        assert_eq!(tokens(&structured), ["short"]);
    }

    #[test]
    fn empty_and_separator_only_input_yield_nothing() {
        assert!(tokens("").is_empty());
        assert!(tokens("   ::  .. ").is_empty());
        assert!(tokens("___").is_empty());
    }

    #[test]
    fn token_after_exhaustion_does_not_panic() {
        let mut tokenizer = CodeTokenizer;
        let mut stream = tokenizer.token_stream("Vec");
        while stream.advance() {}
        // Contract violation by the caller; must not panic in library code.
        let _ = stream.token();
    }
}
