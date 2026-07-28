//! Property tests for the highlighter — implementation-plan **S1-11**.
//!
//! These are the same three invariants `fuzz/fuzz_targets/highlight.rs`
//! asserts, run under `proptest` rather than libFuzzer. That duplication is
//! deliberate and worth its cost here: `cargo-fuzz` needs a nightly toolchain
//! that is **not installed on this machine** (`scripts/check.sh` only
//! type-checks the fuzz targets), so without this file the highlighter's
//! invariants would be asserted by code nobody has ever executed. proptest
//! runs in the ordinary gate; the fuzz target goes deeper when someone
//! installs the toolchain. Keep the two in step.
//!
//! The invariants:
//!
//! 1. **The only markup in the output is the two tags the module emits.**
//!    Code blocks carry attacker-authored text verbatim by design, so any
//!    other `<` is an escaping hole — this is what backs the renderer
//!    contract S1-9 delegates (`sanitize.rs` § "The renderer contract").
//! 2. **Spans balance and none crosses a line boundary.** The `line` wrapper
//!    is what S1-12 counts for line numbers; a span leaking past its
//!    `</span>` gets repaired by the browser into different markup.
//! 3. **Stripping tags and decoding entities returns the input.** A
//!    highlighter that silently drops a character makes code in the reader
//!    uncopyable, which is worse than no highlighting.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use tome_core::highlight::{Highlighter, LINE_CLASS, TOKEN_CLASS_PREFIX};

/// The grammars worth exercising, plus both fallback paths (a language with
/// no syntax, and no language at all). A single hardcoded language would only
/// ever exercise one state machine.
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

fn language() -> impl Strategy<Value = Option<&'static str>> {
    proptest::sample::select(LANGUAGES.to_vec())
}

/// Code that leans on the awkward cases: markup, quotes, ampersands, line
/// terminators of both kinds, multi-byte text, and the unmatched-delimiter
/// inputs that leave a grammar's scope stack open at end of input.
fn code() -> impl Strategy<Value = String> {
    let fragments = proptest::sample::select(vec![
        "fn main() {",
        "}",
        "\n",
        "\r\n",
        "\r",
        "  ",
        "\t",
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "\" onload=\"",
        "&amp;",
        "&",
        "'",
        "\"\"\"",
        "/*",
        "*/",
        "// comment",
        "# comment",
        "\"unterminated string",
        "日本語 — café 🎉",
        "x",
        "",
        "\u{0}",
        "\u{202e}",
    ]);
    proptest::collection::vec(fragments, 0..24).prop_map(|parts| parts.concat())
}

/// Invariants 1 and 2, returning the text content for invariant 3.
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
            for class in tail[..end].split(' ') {
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

        // Every newline in the output is a separator this module emitted
        // between line wrappers, so depth must be zero at each one.
        if let Some(newline) = rest.find('\n') {
            if !rest[..newline].contains('<') {
                assert_eq!(depth, 0, "a span crossed a line boundary");
            }
        }
    }

    assert!(!rest.contains('>'), "unescaped '>' in trailing text");
    assert_eq!(depth, 0, "spans left open at end of output");
    text.push_str(rest);
    text
}

/// Undo exactly what `tome_core::html` applies. `&amp;` goes **last** — the
/// mirror image of escaping `&` first — or `&amp;lt;` would decode to `<`.
fn decode(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn output_is_well_formed_and_escape_tight(code in code(), language in language()) {
        let html = Highlighter::shared().highlight(&code, language);
        assert_well_formed_and_extract_text(&html);
    }

    #[test]
    fn code_survives_highlighting_verbatim(code in code(), language in language()) {
        let html = Highlighter::shared().highlight(&code, language);
        let text = assert_well_formed_and_extract_text(&html);

        // The renderer folds `\r\n` to one break and emits no trailing
        // newline (the `<span class="line">` wrapper carries the structure),
        // so compare against the same normalization.
        let expected = code.replace("\r\n", "\n");
        let expected = expected.strip_suffix('\n').unwrap_or(&expected);
        prop_assert_eq!(decode(&text), expected);
    }

    // Highlighting must not depend on whether a language happened to
    // resolve: a block gaining or losing a line when its language is
    // unknown would make the S1-12 line counter disagree with itself.
    #[test]
    fn line_count_is_independent_of_language(code in code(), language in language()) {
        let h = Highlighter::shared();
        prop_assert_eq!(
            h.highlight(&code, language).split('\n').count(),
            h.highlight(&code, None).split('\n').count()
        );
    }
}

// Arbitrary text, not just the fragment vocabulary above. Cheap insurance
// that nothing in the parse path indexes off a character boundary.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_text_never_panics(code in ".{0,2000}", language in language()) {
        let html = Highlighter::shared().highlight(&code, language);
        assert_well_formed_and_extract_text(&html);
    }
}
