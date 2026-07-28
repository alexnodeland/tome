//! HTML escaping — the one function every string passes through on its way
//! into rendered output.
//!
//! This is small enough to inline at each call site and deliberately is not,
//! because the sanitizer (S1-9) delegates a named responsibility to it. The
//! sanitizer restricts the AST's *token* fields (URL schemes, `id`s, class
//! names) to safe character sets, and deliberately does **not** charset-strip
//! the free-text ones — `Link.title`, `Image.alt`, `Admonition.title`, and
//! every `Text` / `InlineCode` / `CodeBlock` body — because a documentation
//! title may legitimately contain a quote or an angle bracket, and stripping
//! it would destroy content to solve a problem escaping already solves. See
//! `sanitize.rs` § "The renderer contract for free text": those fields are
//! safe *because* the renderer escapes them. If this function is bypassed,
//! S1-9's guarantee is void.
//!
//! # Why one function for both text and attribute contexts
//!
//! `&`, `<`, and `>` are what a text node needs; `"` and `'` are what a
//! quoted attribute value needs. Two functions would mean every call site
//! choosing correctly forever, and the failure mode of choosing wrong is
//! silent until it is an XSS. Escaping all five in both contexts is
//! unconditionally correct — it produces character references a text node
//! renders identically — so there is one function and no choice to get wrong.
//!
//! Attribute values must additionally be **quoted**. An unquoted attribute is
//! escapable by whitespace alone, which no character escaping prevents; see
//! [`attr`], which is how the renderer writes them.

use std::fmt::Write as _;

/// Escape a string for insertion into HTML text or a quoted attribute value.
///
/// Escapes `& < > " '`. `&` must be first, or the escapes escape each other.
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    push_escaped(&mut out, input);
    out
}

/// [`escape`], appending to an existing buffer. The rendering hot path — a
/// document is thousands of these — so it avoids a `String` per node.
pub fn push_escaped(out: &mut String, input: &str) {
    let mut last = 0;
    for (i, byte) in input.bytes().enumerate() {
        let replacement = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        // Safe slicing: every byte matched above is ASCII, so `i` and `i + 1`
        // are always UTF-8 character boundaries however multi-byte the rest
        // of the input is.
        out.push_str(&input[last..i]);
        out.push_str(replacement);
        last = i + 1;
    }
    out.push_str(&input[last..]);
}

/// Write ` name="value"` with the value escaped and **quoted**.
///
/// The quoting is not cosmetic. Inside a double-quoted value the HTML parser
/// treats `<`, `>`, and a bare `&` as literal text and only the closing quote
/// ends the value; unquoted, a single space starts a new attribute, and
/// `id=x onload=alert(1)` is two attributes. The sanitizer's `id` denylist
/// strips whitespace for exactly this reason — as defence in depth behind
/// this function, not instead of it.
pub fn attr(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, " {name}=\"");
    push_escaped(out, value);
    out.push('"');
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_five_characters() {
        assert_eq!(escape("&<>\"'"), "&amp;&lt;&gt;&quot;&#39;");
    }

    #[test]
    fn ampersand_is_escaped_first() {
        // If `<` were substituted before `&`, this would come out as
        // `&amp;lt;` -- double-escaped -- or worse, `&lt;` re-entering the
        // parser as a real `<` on a second pass.
        assert_eq!(escape("&lt;"), "&amp;lt;");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        assert_eq!(escape("hello world"), "hello world");
        assert_eq!(escape(""), "");
    }

    #[test]
    fn preserves_multibyte_text() {
        // The byte-wise scan must not split a multi-byte character. Every
        // escaped byte is ASCII, so the slice boundaries are safe -- this is
        // the test that says so.
        assert_eq!(escape("café — 日本語 <b>"), "café — 日本語 &lt;b&gt;");
        assert_eq!(escape("🎉&🎉"), "🎉&amp;🎉");
    }

    #[test]
    fn closes_the_classic_breakout() {
        let payload = r#"" onload="alert(1)"#;
        let escaped = escape(payload);
        assert!(!escaped.contains('"'), "{escaped}");
    }

    #[test]
    fn attribute_is_quoted_and_escaped() {
        let mut out = String::from("<img");
        attr(&mut out, "alt", r#"a " b"#);
        out.push('>');
        assert_eq!(out, r#"<img alt="a &quot; b">"#);
    }

    #[test]
    fn attribute_value_cannot_start_a_second_attribute() {
        let mut out = String::new();
        attr(&mut out, "id", "x\" onload=\"alert(1)");
        assert_eq!(out, r#" id="x&quot; onload=&quot;alert(1)""#);
    }

    #[test]
    fn push_escaped_appends_rather_than_replacing() {
        let mut out = String::from("prefix:");
        push_escaped(&mut out, "<a>");
        assert_eq!(out, "prefix:&lt;a&gt;");
    }
}
