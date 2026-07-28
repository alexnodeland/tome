//! Syntax highlighting (implementation plan S1-11, ticket P1-014).
//!
//! # Highlighting is a render concern, not an AST mutation
//!
//! [`Node::CodeBlock`](crate::model::Node::CodeBlock) stays `{language,
//! code}` forever. Nothing here touches the AST; the renderer (S1-13) calls
//! [`Highlighter::highlight`] when it emits a `<pre><code>` and throws the
//! result away with the page. That matters for three reasons: the stored AST
//! stays a semantic model rather than a snapshot of one theme's markup, a
//! highlighter upgrade needs no re-crawl and no migration of the frozen serde
//! shape, and the golden corpus keeps diffing meaning rather than colour.
//!
//! # Classes, not colours
//!
//! Output carries CSS classes (`tok-keyword tok-control`), never inline
//! styles. So light and dark are one stylesheet swap in S1-12 with **no
//! re-highlighting**, the reader needs no client-side JavaScript — which
//! matters because the reader frame runs under a CSP with no `unsafe-inline`
//! (SPIKE-002) — and highlighting works with the network off, which is the
//! Stage 1 exit gate.
//!
//! This is why syntect is pulled in without `default-themes`: a theme would
//! only be used to generate colours we discard. See the dependency comment in
//! the workspace `Cargo.toml` for the rest of the feature trimming (notably
//! `regex-fancy`, so no oniguruma C dependency enters a pure-Rust workspace).
//!
//! # Output shape
//!
//! [`Highlighter::highlight`] returns the *inner* HTML of a `<code>` element:
//! one `<span class="line">` per source line, separated by real newlines so
//! that copying out of the reader yields the original text. Unknown or absent
//! languages produce the same structure with no token spans, so the CSS and
//! the line-number counter (S1-12) do not have two cases to handle.
//!
//! ```text
//! <span class="line"><span class="tok-keyword tok-source">fn</span> main() {</span>
//! <span class="line">}</span>
//! ```
//!
//! # Escaping
//!
//! Every character of code goes through [`crate::html::push_escaped`] — this
//! module's own escaping, not syntect's, so that the renderer contract S1-9
//! depends on has exactly one implementation to audit. `hostile_code_cannot_
//! escape_its_element` is the test that says so.

use std::sync::OnceLock;

use syntect::parsing::{
    BasicScopeStackOp, ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet,
};
use syntect::util::LinesWithEndings;

use crate::html::push_escaped;

/// Prefix on every token class this module emits.
///
/// syntect's scope atoms are words like `string`, `comment`, `keyword`, and
/// `meta` — names any stylesheet is likely to use for something else. The
/// prefix keeps the highlighter's classes in their own namespace so a rule in
/// the reader stylesheet can never accidentally style a token, or vice versa.
pub const TOKEN_CLASS_PREFIX: &str = "tok-";

/// Class on the wrapper around each source line.
///
/// Line numbers are a CSS counter over these (S1-12), which is what makes
/// them free to turn off: no second rendering path, no markup difference,
/// just a rule that stops incrementing.
pub const LINE_CLASS: &str = "line";

/// Upper bound on the code a single block will highlight, in bytes.
///
/// Highlighting is regex-driven and superlinear on pathological input. A
/// generated API page can carry a multi-megabyte code block, and a reader
/// that stalls on one page is worse than one that renders it unhighlighted —
/// so past this size the block renders as escaped plain text. It still
/// renders; that is the point.
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

/// Aliases from the language names documentation sites actually emit to the
/// tokens syntect's default syntax set answers to.
///
/// Normalization (S1-8, `normalize::canonical_language`) already folds the
/// obvious shorthands — `rs` → `rust`, `py` → `python` — so what remains here
/// is the second gap: canonical names that syntect happens to file under a
/// different word. Anything not listed falls through to syntect's own
/// extension-then-name lookup, and anything that lookup misses renders plain.
///
/// # The TypeScript entry is a documented compromise
///
/// **syntect's bundled set has no TypeScript syntax** — its defaults come
/// from Sublime's shipped packages, which predate it. P1-014 names TypeScript
/// in its acceptance criteria, so the choice was between adding a second
/// asset crate (`two-face`, which vendors bat's ~200 extra syntaxes under a
/// mix of third-party licences) and mapping TypeScript onto JavaScript.
/// JavaScript is chosen: TS is a syntactic superset, so keywords, strings,
/// comments, numbers, and regexes all highlight correctly and only type
/// annotations (`interface`, `type`, `: string`) render as plain identifiers.
/// The same applies to `jsx`/`tsx`. If the owner wants exact TS — and TOML,
/// Dockerfile, Kotlin, and the other genuinely-absent syntaxes listed in
/// `known_gaps_are_recorded_not_forgotten` — `two-face` is the upgrade path,
/// and it is a licence decision (see SPIKE-010's posture) rather than a
/// technical one.
///
/// `resolves_every_language_the_prd_requires` is the test that keeps this
/// table honest against the actual bundled syntax set.
const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("bash", "sh"),
    ("cpp", "c++"),
    ("csharp", "cs"),
    ("golang", "go"),
    ("javascript", "js"),
    ("jsx", "js"),
    ("objc", "m"),
    ("python", "py"),
    ("restructuredtext", "rst"),
    ("rust", "rs"),
    ("shell", "sh"),
    ("tsx", "js"),
    ("typescript", "js"),
    ("zsh", "sh"),
];

/// Languages that must render as plain text even though a syntax exists.
///
/// `text` is what normalization canonicalizes "plain", "plaintext", and
/// "none" to, and it means the author said *do not highlight this*. syntect
/// would happily match "Plain Text" by name and produce a document-scope
/// span; honouring the author is cheaper and more correct.
const PLAIN_LANGUAGES: &[&str] = &["text", "txt", "output", "console"];

/// Holds the syntax set. Construction loads and inflates syntect's bundled
/// syntax dumps, which is why [`shared`](Self::shared) exists — doing it per
/// code block would dominate render time.
pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// Load the bundled syntax definitions. Costs tens of milliseconds and
    /// several megabytes; prefer [`shared`](Self::shared).
    pub fn new() -> Self {
        // `_newlines` and not `_nonewlines`: the no-newline dumps highlight
        // multi-line constructs incorrectly, and syntect's own docs deprecate
        // the API that needs them. Lines are fed in with their endings below.
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// The process-wide highlighter, loaded on first use.
    ///
    /// A `OnceLock` rather than a lazily-initialised global with interior
    /// mutability: the syntax set is immutable after loading, so there is
    /// nothing to lock on the hot path, and a page of two hundred code blocks
    /// pays the load cost once for the process.
    pub fn shared() -> &'static Self {
        static SHARED: OnceLock<Highlighter> = OnceLock::new();
        SHARED.get_or_init(Self::new)
    }

    /// Whether a language will actually be highlighted. Exposed for the
    /// renderer, which marks unhighlighted blocks so the stylesheet can drop
    /// their token padding, and for tests.
    pub fn supports(&self, language: Option<&str>) -> bool {
        self.syntax_for(language).is_some()
    }

    /// Highlight `code`, returning the inner HTML of a `<code>` element.
    ///
    /// Never fails and never panics: an unknown language, a syntax error
    /// inside syntect, or code past [`MAX_HIGHLIGHT_BYTES`] all degrade to
    /// escaped plain text in the same line-wrapped structure. A reader that
    /// refuses to show a page because one code block confused a regex engine
    /// would be trading the product for the feature.
    pub fn highlight(&self, code: &str, language: Option<&str>) -> String {
        match self.syntax_for(language) {
            Some(syntax) if code.len() <= MAX_HIGHLIGHT_BYTES => self
                .highlight_with(code, syntax)
                .unwrap_or_else(|| plain(code)),
            _ => plain(code),
        }
    }

    /// The syntax to use, or `None` for plain rendering.
    fn syntax_for(&self, language: Option<&str>) -> Option<&SyntaxReference> {
        // Normalization lowercases; the sanitizer restricts the value to
        // `[a-z0-9-]`. Neither is assumed here — this is called with strings
        // from the renderer, and a defensive `to_ascii_lowercase` is cheaper
        // than a bug that only appears on one crawler's output.
        let name = language?.trim().to_ascii_lowercase();
        if name.is_empty() || PLAIN_LANGUAGES.contains(&name.as_str()) {
            return None;
        }
        let token = LANGUAGE_ALIASES
            .iter()
            .find(|(from, _)| *from == name)
            .map_or(name.as_str(), |(_, to)| to);
        self.syntaxes
            .find_syntax_by_token(token)
            // A hyphenated name syntect does not know (`objective-c`,
            // `docker-compose`) may still match on its head.
            .or_else(|| {
                token
                    .split_once('-')
                    .and_then(|(head, _)| self.syntaxes.find_syntax_by_token(head))
            })
    }

    /// The real work. `None` means syntect gave up part-way; the caller falls
    /// back to plain text rather than emitting a half-highlighted block whose
    /// spans may not balance.
    fn highlight_with(&self, code: &str, syntax: &SyntaxReference) -> Option<String> {
        let mut state = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        // Classes of the spans currently open, outermost first. syntect's own
        // generator lets a span run across a line boundary, which cannot work
        // here: a `<span class="line">` must close at the end of its line, so
        // any token span still open has to close with it and reopen inside
        // the next one. Keeping the class strings is what makes reopening
        // possible, and it is the only reason this does not just call
        // `ClassedHTMLGenerator`.
        let mut open: Vec<String> = Vec::new();
        let mut out = String::with_capacity(code.len() * 2);
        let mut first = true;

        for line in LinesWithEndings::from(code) {
            if !first {
                out.push('\n');
            }
            first = false;

            open_line(&mut out);
            for class in &open {
                out.push_str("<span class=\"");
                out.push_str(class);
                out.push_str("\">");
            }

            let ops = state.parse_line(line, &self.syntaxes).ok()?;
            emit_line(line, &ops, &mut stack, &mut open, &mut out).ok()?;

            for _ in 0..open.len() {
                out.push_str("</span>");
            }
            out.push_str("</span>");
        }

        // `LinesWithEndings` yields nothing for empty input; `plain` has the
        // one-empty-line case and there is no reason for two copies of it.
        if first {
            return Some(plain(code));
        }
        Some(out)
    }
}

/// Escaped plain text in the same line structure highlighting produces, so
/// the stylesheet, the line counter, and any future per-line feature see one
/// shape rather than two.
///
/// It splits lines with `LinesWithEndings`, the same iterator the highlighted
/// path feeds syntect, so that the two agree on what a line *is*. They did
/// not at first: `split('\n')` yields a trailing empty line for `"x\n"` and
/// `LinesWithEndings` does not, which made a code block gain or lose a blank
/// final line depending on whether its language happened to resolve.
fn plain(code: &str) -> String {
    let mut out = String::with_capacity(code.len() + 32);
    let mut first = true;
    for line in LinesWithEndings::from(code) {
        if !first {
            out.push('\n');
        }
        first = false;
        open_line(&mut out);
        push_escaped(&mut out, &line[..line_text_end(line)]);
        out.push_str("</span>");
    }
    // Empty input yields no lines at all, which would render as no element —
    // and the stylesheet would then collapse the `<pre>` to nothing. One
    // empty line is the honest rendering of an empty code block.
    if first {
        open_line(&mut out);
        out.push_str("</span>");
    }
    out
}

fn open_line(out: &mut String) {
    out.push_str("<span class=\"");
    out.push_str(LINE_CLASS);
    out.push_str("\">");
}

/// Where a line's text ends, excluding its terminator.
///
/// A `\r` counts as part of the terminator **only** as the `\r\n` pair. A
/// lone trailing `\r` is data — in a code block it may be exactly what the
/// page is documenting — and dropping it would violate the round-trip
/// property the fuzz target asserts.
fn line_text_end(line: &str) -> usize {
    match line.strip_suffix('\n') {
        Some(without_lf) => without_lf.strip_suffix('\r').unwrap_or(without_lf).len(),
        None => line.len(),
    }
}

/// Emit one line's token spans into `out`, updating `stack` and `open`.
///
/// A close reimplementation of syntect's `line_tokens_to_classed_spans`,
/// which cannot be used directly for two reasons: it escapes with syntect's
/// own escaper (the renderer contract wants one auditable escape, see
/// `crate::html`), and it does not report which classes it opened, which is
/// what per-line wrapping needs.
///
/// The trailing newline is parsed but not emitted — syntect needs it to
/// highlight multi-line constructs correctly, and the `<span class="line">`
/// wrapper supplies the line break in the output.
fn emit_line(
    line: &str,
    ops: &[(usize, ScopeStackOp)],
    stack: &mut ScopeStack,
    open: &mut Vec<String>,
    out: &mut String,
) -> Result<(), ()> {
    let text_end = line_text_end(line);

    // Elision of empty spans. A grammar routinely pushes and immediately pops
    // a scope that matched nothing, and on a real page those empty pairs are
    // a fifth of the markup. `at` is where the innermost still-empty span's
    // opening tag begins, so popping it can rewind instead of closing it.
    let mut empty_span_at: Option<usize> = None;

    let mut cursor = 0usize;
    for (index, op) in ops {
        let index = (*index).min(text_end);
        if index > cursor {
            push_escaped(out, &line[cursor..index]);
            cursor = index;
            empty_span_at = None;
        }
        // The hook itself cannot fail; the error is syntect's own — a scope
        // stack underflow from a malformed syntax definition.
        stack
            .apply_with_hook(op, |basic, _| match basic {
                BasicScopeStackOp::Push(scope) => {
                    let class = token_class(scope);
                    empty_span_at = Some(out.len());
                    out.push_str("<span class=\"");
                    out.push_str(&class);
                    out.push_str("\">");
                    open.push(class);
                }
                BasicScopeStackOp::Pop => {
                    // A `pop` on an empty `open` would mean syntect popped a
                    // scope it never pushed. Emitting the closing tag anyway
                    // would unbalance the line wrapper; skipping it keeps the
                    // markup well-formed, which is the invariant that matters.
                    if open.pop().is_some() {
                        match empty_span_at.take() {
                            Some(at) => out.truncate(at),
                            None => out.push_str("</span>"),
                        }
                    }
                }
            })
            .map_err(|_| ())?;
    }
    if cursor < text_end {
        push_escaped(out, &line[cursor..text_end]);
    }
    Ok(())
}

/// The CSS class for one scope: every atom, prefixed.
///
/// `keyword.control.rust` becomes `tok-keyword tok-control tok-rust`, so a
/// stylesheet can be as coarse as `.tok-keyword` or as fine as
/// `.tok-keyword.tok-control`. The atoms come from syntect's own scope
/// vocabulary and are `[a-z0-9._-]`, but they are filtered anyway: this
/// string lands in a `class` attribute, and a class list is one of the few
/// places where escaping is not sufficient on its own — a space would create
/// a class, not break out, but a quote would break out.
fn token_class(scope: Scope) -> String {
    let mut class = String::new();
    for atom in scope.build_string().split('.') {
        if atom.is_empty() {
            continue;
        }
        if !class.is_empty() {
            class.push(' ');
        }
        class.push_str(TOKEN_CLASS_PREFIX);
        class.extend(
            atom.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_'),
        );
    }
    class
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn h() -> &'static Highlighter {
        Highlighter::shared()
    }

    /// Every line span opened is closed, and no `<span>` crosses a line
    /// boundary. This is the structural invariant the whole module rests on;
    /// the fuzz target asserts the same thing on arbitrary input.
    fn assert_well_formed(html: &str) {
        for line in html.split('\n') {
            let mut depth = 0i32;
            let mut rest = line;
            while let Some(at) = rest.find('<') {
                rest = &rest[at..];
                if rest.starts_with("</span>") {
                    depth -= 1;
                    rest = &rest[7..];
                } else if rest.starts_with("<span class=\"") {
                    depth += 1;
                    rest = &rest[13..];
                } else {
                    panic!("unexpected markup in {line:?}");
                }
                assert!(depth >= 0, "closed more spans than opened in {line:?}");
            }
            assert_eq!(depth, 0, "unbalanced spans in {line:?}");
        }
    }

    #[test]
    fn highlights_rust() {
        let html = h().highlight("fn main() {}\n", Some("rust"));
        // `fn` is `storage.type.function` in Sublime's Rust grammar, not
        // `keyword` -- assert on the structure that is actually there rather
        // than on a scope name guessed from another highlighter's vocabulary.
        assert!(html.contains("tok-storage"), "{html}");
        assert!(html.contains("tok-entity"), "{html}");
        assert!(html.contains(">main<"), "{html}");
        assert_well_formed(&html);
    }

    #[test]
    fn resolves_every_language_the_prd_requires() {
        // P1-014's acceptance list, plus the rest of what normalization
        // canonicalizes to and what the Stage 1 corpus actually contains.
        for language in [
            "rust",
            "python",
            "javascript",
            "typescript", // via the documented alias to JavaScript
            "tsx",
            "jsx",
            "go",
            "java",
            "c",
            "cpp",
            "csharp",
            "bash",
            "shell",
            "zsh",
            "html",
            "css",
            "json",
            "yaml",
            "sql",
            "ruby",
            "php",
            "perl",
            "lua",
            "haskell",
            "scala",
            "markdown",
            "xml",
            "diff",
            "makefile",
            "restructuredtext",
        ] {
            assert!(
                h().supports(Some(language)),
                "no syntax resolved for {language:?}"
            );
        }
    }

    #[test]
    fn known_gaps_are_recorded_not_forgotten() {
        // syntect's bundled set stops at Sublime's shipped packages. These
        // render as plain text today; the list is here so the gap is a fact
        // in the test suite rather than a surprise in the reader, and so that
        // adding an asset crate later has an assertion to flip. See
        // LANGUAGE_ALIASES for why `two-face` was not adopted unilaterally.
        for language in ["toml", "dockerfile", "kotlin", "swift", "zig", "elixir"] {
            assert!(
                !h().supports(Some(language)),
                "{language:?} now resolves -- move it to the supported list"
            );
        }
    }

    #[test]
    fn unknown_language_renders_as_plain_text() {
        let html = h().highlight("some code\n", Some("brainfuck-9000"));
        assert!(!html.contains("tok-"), "{html}");
        assert!(html.contains("some code"), "{html}");
        assert_well_formed(&html);
    }

    #[test]
    fn absent_language_renders_as_plain_text() {
        let html = h().highlight("x = 1", None);
        assert_eq!(html, "<span class=\"line\">x = 1</span>");
    }

    #[test]
    fn explicit_plain_text_is_not_highlighted() {
        // The author wrote ```text; honour it rather than letting syntect
        // match "Plain Text" by name.
        for language in PLAIN_LANGUAGES {
            assert!(!h().supports(Some(language)), "{language} should be plain");
        }
    }

    #[test]
    fn hostile_code_cannot_escape_its_element() {
        // The whole reason this module escapes rather than trusting its
        // input: code blocks carry attacker-authored text verbatim, by
        // design -- a page documenting XSS contains payloads as content.
        let payloads = [
            "</code></pre><script>alert(1)</script>",
            "<img src=x onerror=alert(1)>",
            "\" onload=\"alert(1)",
            "'; DROP TABLE pages; --",
        ];
        for payload in payloads {
            for language in [None, Some("rust"), Some("html"), Some("nonsense")] {
                let html = h().highlight(payload, language);
                // `assert_well_formed` rejects any tag that is not one of the
                // two this module emits, so it *is* the escaping assertion:
                // an unescaped `<script` or `<img` from the payload would be
                // "unexpected markup". The explicit checks below say so out
                // loud, since that is not obvious from the helper's name.
                assert_well_formed(&html);
                assert!(
                    !html.contains("<script") && !html.contains("<img"),
                    "{payload:?} as {language:?}: {html}"
                );
                // The content is not lost, only neutralised -- a page about
                // XSS must still be able to show its own examples.
                assert!(html.contains("&lt;") || !payload.contains('<'));
            }
        }
    }

    #[test]
    fn every_line_is_its_own_span() {
        let html = h().highlight("fn a() {\n    b();\n}\n", Some("rust"));
        let lines: Vec<&str> = html.split('\n').collect();
        assert_eq!(lines.len(), 3, "{html}");
        for line in &lines {
            assert!(line.starts_with("<span class=\"line\">"), "{line}");
            assert!(line.ends_with("</span>"), "{line}");
        }
        assert_well_formed(&html);
    }

    #[test]
    fn spans_reopen_across_a_multi_line_construct() {
        // A block comment opens a scope on line 1 that is still open on line
        // 3. Without the reopen logic the markup would nest across the line
        // wrappers and the browser would repair it into something else.
        let html = h().highlight("/* one\n   two\n   three */\n", Some("rust"));
        assert_well_formed(&html);
        for line in html.split('\n') {
            assert!(line.contains("tok-comment"), "{line}");
        }
    }

    #[test]
    fn text_survives_round_trip() {
        // Strip the markup and the original code must come back, entities
        // decoded. A highlighter that silently drops a character makes code
        // in the reader uncopyable, which is worse than no highlighting.
        let code = "let s = \"a & b < c\";\nlet t = 'x';\n";
        let html = h().highlight(code, Some("rust"));
        let mut text = String::new();
        let mut rest = html.as_str();
        while let Some(at) = rest.find('<') {
            text.push_str(&rest[..at]);
            let close = rest[at..].find('>').unwrap() + at;
            rest = &rest[close + 1..];
        }
        text.push_str(rest);
        let decoded = text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&");
        assert_eq!(decoded, code.trim_end_matches('\n'));
    }

    #[test]
    fn empty_code_still_renders_one_line() {
        let html = h().highlight("", Some("rust"));
        assert_eq!(html, "<span class=\"line\"></span>");
        assert_well_formed(&html);
    }

    #[test]
    fn highlighted_and_plain_agree_on_what_a_line_is() {
        // These two paths split lines with different code, and a code block
        // must not gain or lose a trailing blank line depending on whether
        // its language happened to resolve. Caught by the fuzz target's
        // round-trip property before this test existed.
        for code in ["x\n", "x", "x\n\n", "", "\n", "a\nb\n", "a\r\nb\r\n", "a\r"] {
            let lines = |s: &str| s.split('\n').count();
            assert_eq!(
                lines(&h().highlight(code, Some("rust"))),
                lines(&h().highlight(code, None)),
                "line count differs for {code:?}"
            );
        }
    }

    #[test]
    fn a_lone_carriage_return_is_data_not_a_terminator() {
        // Only `\r\n` is a line ending. A bare trailing `\r` is content -- in
        // a code block it may be exactly what the page documents -- and the
        // fuzz target's round-trip property fails if it is silently eaten.
        let html = h().highlight("a\r", None);
        assert_eq!(html, "<span class=\"line\">a\r</span>");
    }

    #[test]
    fn crlf_line_endings_do_not_leak_into_output() {
        let html = h().highlight("a = 1\r\nb = 2\r\n", Some("python"));
        assert!(!html.contains('\r'), "{html:?}");
        assert_eq!(html.split('\n').count(), 2, "{html}");
        assert_well_formed(&html);
    }

    #[test]
    fn multibyte_code_is_not_split() {
        let html = h().highlight("s = \"日本語 — café 🎉\"\n", Some("python"));
        assert!(html.contains("日本語"), "{html}");
        assert!(html.contains("🎉"), "{html}");
        assert_well_formed(&html);
    }

    #[test]
    fn oversized_blocks_degrade_to_plain_rather_than_stalling() {
        let code = "fn f() {}\n".repeat(MAX_HIGHLIGHT_BYTES / 10 + 100);
        assert!(code.len() > MAX_HIGHLIGHT_BYTES);
        let html = h().highlight(&code, Some("rust"));
        assert!(!html.contains("tok-"), "oversized block was highlighted");
        assert!(html.contains("fn f() {}"));
    }

    #[test]
    fn token_classes_are_prefixed_and_charset_restricted() {
        let html = h().highlight("# comment\n", Some("python"));
        for class in html
            .split("class=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
        {
            for token in class.split(' ') {
                assert!(
                    token == LINE_CLASS || token.starts_with(TOKEN_CLASS_PREFIX),
                    "unprefixed class {token:?} in {html}"
                );
                assert!(
                    token
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                    "unsafe class {token:?}"
                );
            }
        }
    }

    #[test]
    fn language_lookup_is_case_and_whitespace_insensitive() {
        assert!(h().supports(Some("  RUST ")));
        assert!(!h().supports(Some("   ")));
        assert!(!h().supports(None));
    }

    #[test]
    fn hyphenated_names_fall_back_to_their_head() {
        // Sphinx emits `python-console`, rustdoc `rust-ignore`; both should
        // land on the base language rather than rendering plain.
        assert!(h().supports(Some("rust-ignore")));
        assert!(h().supports(Some("python-repl")));
    }
}
