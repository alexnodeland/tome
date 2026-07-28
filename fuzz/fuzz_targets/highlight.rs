//! The highlighter must emit well-formed, escape-tight markup on any input,
//! in any language, and never lose a character of the code it was given.
//!
//! Three properties, each a real failure mode rather than "does not panic":
//!
//! 1. **The only markup in the output is the two tags this module emits.**
//!    Code blocks carry attacker-authored text verbatim by design — a page
//!    documenting XSS contains payloads as content — so any other `<` in the
//!    output is an escaping hole. This is the property that backs S1-9's
//!    renderer contract.
//! 2. **Spans balance, and none crosses a line boundary.** The line wrapper
//!    exists so S1-12 can number lines with a CSS counter; a span that leaks
//!    past `</span>` gets repaired by the browser into different markup.
//! 3. **Stripping the tags and decoding the entities returns the input.**
//!    A highlighter that silently drops a character makes code in the reader
//!    uncopyable, which is worse than no highlighting at all.
//!
//! The language is taken from the first byte so the corpus explores the
//! plain-text path, the alias table, and several real grammars — a single
//! hardcoded language would only ever fuzz one state machine.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::highlight::{Highlighter, LINE_CLASS, TOKEN_CLASS_PREFIX};

/// A spread of grammars plus the two fallback paths (unknown, and none).
const LANGUAGES: &[Option<&str>] = &[
    Some("rust"),
    Some("python"),
    Some("javascript"),
    Some("typescript"),
    Some("html"),
    Some("bash"),
    Some("yaml"),
    Some("json"),
    Some("markdown"),
    Some("c"),
    Some("go"),
    Some("text"),
    Some("no-such-language"),
    None,
];

/// Property 1 and 2: walk the markup, asserting every tag is one of ours and
/// that depth returns to zero at every line break. Returns the text content
/// with tags removed, for property 3.
fn assert_well_formed_and_extract_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut depth: i32 = 0;
    let mut rest = html;

    while let Some(at) = rest.find('<') {
        let (before, tail) = rest.split_at(at);
        assert!(
            !before.contains('>'),
            "unescaped '>' in text content: {before:?}"
        );
        text.push_str(before);

        if let Some(tail) = tail.strip_prefix("</span>") {
            depth -= 1;
            assert!(depth >= 0, "closed more spans than were opened");
            rest = tail;
        } else if let Some(tail) = tail.strip_prefix("<span class=\"") {
            let end = tail.find('"').expect("class attribute is closed");
            let classes = &tail[..end];
            for class in classes.split(' ') {
                assert!(
                    class == LINE_CLASS || class.starts_with(TOKEN_CLASS_PREFIX),
                    "unnamespaced class {class:?}"
                );
                assert!(
                    class
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                    "class token would break the attribute: {class:?}"
                );
            }
            depth += 1;
            rest = tail[end..]
                .strip_prefix("\">")
                .expect("class attribute is closed by '\">'");
        } else {
            // The whole point: anything else here came from the input.
            panic!(
                "markup escaped into the output: {:?}",
                &tail[..tail.len().min(64)]
            );
        }

        // A line break may only occur outside every span.
        if let Some(newline) = rest.find('\n') {
            if rest[..newline].find('<').is_none() {
                assert_eq!(depth, 0, "a span crossed a line boundary");
            }
        }
    }

    assert!(!rest.contains('>'), "unescaped '>' in trailing text");
    assert_eq!(depth, 0, "spans left open at end of output");
    text.push_str(rest);
    text
}

/// Undo exactly the escaping `tome_core::html` applies. `&amp;` last, mirror
/// image of escaping `&` first — otherwise `&amp;lt;` decodes to `<`.
fn decode(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fuzz_target!(|data: &[u8]| {
    let Some((selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(code) = std::str::from_utf8(rest) else {
        return;
    };
    let language = LANGUAGES[*selector as usize % LANGUAGES.len()];

    let html = Highlighter::shared().highlight(code, language);
    let text = assert_well_formed_and_extract_text(&html);

    // Property 3. The renderer normalizes line endings (a `\r\n` in the
    // source becomes one line break in the output) and does not emit a
    // trailing newline, so compare against the same normalization.
    let expected = code.replace("\r\n", "\n");
    let expected = expected.strip_suffix('\n').unwrap_or(&expected);
    assert_eq!(
        decode(&text),
        expected,
        "code did not survive highlighting as {language:?}"
    );
});
