//! No documentation page, however hostile, can put markup of its own into the
//! reader's HTML.
//!
//! This is the fuzz form of the contract `sanitize.rs` delegates to the
//! renderer. The sanitizer deliberately does **not** charset-strip free-text
//! fields — `Link.title`, `Image.alt`, `Admonition.title`, and every text and
//! code node — because a real documentation title may contain a quote or an
//! angle bracket. Their safety is escaping, and escaping is here.
//!
//! Arbitrary bytes go in as HTML and travel the real path: parse (S1-7) →
//! sanitize (S1-9) → render (S1-13). Four properties come out, and none of
//! them is "did not panic":
//!
//! 1. **Every tag in the output is one the renderer wrote.** A `<` that
//!    escaped from page content shows up as an unexpected tag name. This is
//!    the XSS property, stated in the form that cannot be satisfied by
//!    accident.
//! 2. **Every attribute value is quoted.** An unquoted value is escapable by
//!    whitespace alone, which no character escaping prevents.
//! 3. **No `src` attribute leaves the local asset base.** The offline
//!    guarantee: a page must never reach the network to render.
//! 4. **Every outline entry points at an id that exists in the HTML.** A TOC
//!    whose links go nowhere is the failure the original sanitizer draft
//!    caused by stripping `id`, and it is invisible without a check.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::highlight::Highlighter;
use tome_core::parse::parse_page;
use tome_core::render::{render, OutlineEntry, RenderOptions};
use tome_core::sanitize::sanitize;

/// Every element the renderer is allowed to emit. Kept as a literal list
/// rather than derived, so adding a tag to the renderer is a deliberate edit
/// here too.
const ALLOWED_TAGS: &[&str] = &[
    "div", "h1", "h2", "h3", "h4", "h5", "h6", "p", "pre", "code", "blockquote", "ul", "ol", "li",
    "dl", "dt", "dd", "table", "thead", "tbody", "tr", "th", "td", "img", "hr", "br", "em",
    "strong", "a", "span",
];

const ASSET_BASE: &str = "tome://localhost/fuzz/";

/// Walk the output as markup, asserting properties 1 and 2.
///
/// A real scanner rather than a substring search, because the naive version
/// is wrong in a way that only arbitrary input reveals: scanning for `=` and
/// checking the next character breaks the moment an attribute *value*
/// contains an `=`, which page content routinely does. Everything the
/// renderer escapes (`&`, `<`, `>`, `"`, `'`) is gone from text and from
/// values, so the structure below is unambiguous — and if it ever is not,
/// that is itself the bug.
fn check_tags_and_attributes(html: &str) {
    let mut rest = html;
    loop {
        let Some(at) = rest.find('<') else {
            assert!(!rest.contains('>'), "unescaped '>' in trailing text");
            return;
        };
        assert!(!rest[..at].contains('>'), "unescaped '>' in text content");
        rest = &rest[at + 1..];

        let closing = rest.starts_with('/');
        if closing {
            rest = &rest[1..];
        }

        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        assert!(
            ALLOWED_TAGS.contains(&name),
            "markup escaped into the output as <{name}>"
        );
        rest = &rest[name_end..];

        loop {
            rest = rest.trim_start();
            if let Some(tail) = rest.strip_prefix("/>") {
                rest = tail;
                break;
            }
            if let Some(tail) = rest.strip_prefix('>') {
                rest = tail;
                break;
            }
            assert!(!closing, "a closing </{name}> carries attributes");

            // `>` cannot appear inside a value (it is escaped), so the first
            // one is the end of the tag and bounds the search for `=`.
            let tag_end = rest.find('>').expect("unterminated tag");
            let equals = rest.find('=').expect("attribute with no value");
            assert!(equals < tag_end, "attribute with no value in <{name}>");

            let attribute = &rest[..equals];
            assert!(
                attribute
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "odd attribute name {attribute:?} in <{name}>"
            );

            rest = &rest[equals + 1..];
            // Property 2. An unquoted value is escapable by whitespace alone,
            // which no character escaping prevents.
            rest = rest.strip_prefix('"').expect("unquoted attribute value");
            let end = rest.find('"').expect("unterminated attribute value");
            let value = &rest[..end];
            assert!(
                !value.contains('<') && !value.contains('>'),
                "unescaped bracket in the {attribute} value of <{name}>"
            );
            rest = &rest[end + 1..];
        }
    }
}

fn check_sources(html: &str) {
    for (at, _) in html.match_indices("src=\"") {
        let value = &html[at + 5..];
        let Some(end) = value.find('"') else { return };
        let value = &value[..end];
        assert!(
            value.starts_with(ASSET_BASE),
            "a src reaches outside the local asset store: {value}"
        );
    }
}

fn check_outline(outline: &[OutlineEntry], html: &str) {
    for entry in outline {
        assert!(!entry.id.is_empty(), "an outline entry has no anchor");
        // The anchor must exist in the document, escaped the same way the
        // renderer escaped it when it wrote the id attribute.
        let escaped = tome_core::html::escape(&entry.id);
        assert!(
            html.contains(&format!("id=\"{escaped}\"")),
            "the outline links to #{}, which is not in the page",
            entry.id
        );
        check_outline(&entry.children, html);
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(html) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(base) = url::Url::parse("https://fuzz.test/dir/page.html") else {
        return;
    };

    let parsed = parse_page(html, &base, None);
    let clean = sanitize(parsed.body);

    let out = render(
        &clean,
        &RenderOptions {
            asset_base: ASSET_BASE,
            highlighter: Highlighter::shared(),
        },
    );

    check_tags_and_attributes(&out.html);
    check_sources(&out.html);
    check_outline(&out.outline, &out.html);
});
