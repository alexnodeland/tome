//! Property tests for the renderer — implementation-plan **S1-13**.
//!
//! The same properties `fuzz/fuzz_targets/render.rs` asserts, run under
//! proptest so that they actually execute in the gate. `cargo-fuzz` needs a
//! nightly toolchain that is **not installed on this machine** — `check.sh`
//! only type-checks the fuzz targets — so without this file the renderer's
//! escaping contract would be asserted by code nobody has ever run. Keep the
//! two in step.
//!
//! Arbitrary hostile HTML travels the real path: parse (S1-7) → sanitize
//! (S1-9) → render (S1-13). The properties are:
//!
//! 1. **Every tag in the output is one the renderer wrote.** This is the XSS
//!    property in the form that cannot be satisfied by accident — a `<` that
//!    escaped from page content appears as an unexpected tag name.
//! 2. **Every attribute value is quoted, and contains no bracket.** An
//!    unquoted value is escapable by whitespace alone, which no character
//!    escaping prevents.
//! 3. **No `src` leaves the local asset base.** The offline guarantee.
//! 4. **Every outline entry points at an id that is in the HTML.** A TOC
//!    whose links go nowhere is exactly what the original sanitizer draft
//!    caused by stripping `id`, and it is invisible without a check.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use tome_core::highlight::Highlighter;
use tome_core::parse::parse_page;
use tome_core::render::{render, OutlineEntry, RenderOptions};
use tome_core::sanitize::sanitize;
use url::Url;

/// Every element the renderer is allowed to emit. A literal list, so adding a
/// tag to the renderer is a deliberate edit here too.
const ALLOWED_TAGS: &[&str] = &[
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
    "pre",
    "code",
    "blockquote",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "img",
    "hr",
    "br",
    "em",
    "strong",
    "a",
    "span",
];

const ASSET_BASE: &str = "tome://localhost/fuzz/";

/// HTML that leans on the cases a renderer gets wrong: attribute breakouts,
/// script and handler injection, the structures with the most attributes
/// (headings, definition lists, links, images), and multi-byte text.
fn html_fragment() -> impl Strategy<Value = String> {
    proptest::sample::select(vec![
        r#"<h2 id="a&quot;b">Heading</h2>"#,
        r#"<h2 id="x" onmouseover="alert(1)">Heading</h2>"#,
        r#"<h2>Duplicate</h2><h2>Duplicate</h2>"#,
        r#"<h1 id="impl-From<T>-for-T">rustdoc anchor</h1>"#,
        r#"<p title='" onload="alert(1)'>para</p>"#,
        r#"<a href="javascript:alert(1)" title="&quot;>">click</a>"#,
        r#"<a href="https://example.com/?a=b&c=d">link</a>"#,
        r##"<a href="#frag">same page</a>"##,
        r#"<img src="x" alt="&quot; onerror=&quot;alert(1)">"#,
        r#"<img src="https://cdn.example/l.png" alt="a=b">"#,
        r#"<pre><code class="language-rust">fn main() { "</code></pre>" }</code></pre>"#,
        r#"<pre><code>&lt;script&gt;alert(1)&lt;/script&gt;</code></pre>"#,
        r#"<div class="admonition warning"><p class="admonition-title">"=></p><p>b</p></div>"#,
        r#"<dl><dt id="a=b">term</dt><dd>def</dd></dl>"#,
        r#"<table><tr><th>a="b"</th></tr><tr><td>c</td></tr></table>"#,
        r#"<ol start="4"><li>x</li></ol>"#,
        r#"<span id="日本語">unicode anchor</span>"#,
        r#"<p>café — 日本語 🎉 &amp; &lt; &gt;</p>"#,
        r#"<blockquote><p>q</p></blockquote><hr><br>"#,
        r#"<p>unclosed <em>emphasis"#,
        r#"<h3 id="">empty id</h3>"#,
        r#"<p></p>"#,
        "",
    ])
    .prop_map(str::to_owned)
}

fn document() -> impl Strategy<Value = String> {
    proptest::collection::vec(html_fragment(), 0..10)
        .prop_map(|parts| format!("<main>{}</main>", parts.concat()))
}

/// Walk the output as markup, asserting properties 1 and 2.
///
/// A real scanner rather than a substring search: the naive "find `=`, check
/// the next character" version breaks the moment an attribute *value*
/// contains an `=`, which documentation routinely does.
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
            // one bounds the tag and therefore the search for `=`.
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
        assert!(
            value[..end].starts_with(ASSET_BASE),
            "a src reaches outside the local asset store: {}",
            &value[..end]
        );
    }
}

fn check_outline(outline: &[OutlineEntry], html: &str) {
    for entry in outline {
        assert!(!entry.id.is_empty(), "an outline entry has no anchor");
        let escaped = tome_core::html::escape(&entry.id);
        assert!(
            html.contains(&format!("id=\"{escaped}\"")),
            "the outline links to #{}, which is not in the page",
            entry.id
        );
        check_outline(&entry.children, html);
    }
}

fn render_hostile(html: &str) -> tome_core::render::RenderedPage {
    let base: Url = "https://fuzz.test/dir/page.html"
        .parse()
        .expect("a literal URL");
    let parsed = parse_page(html, &base, None);
    let clean = sanitize(parsed.body);
    render(
        &clean,
        &RenderOptions {
            asset_base: ASSET_BASE,
            highlighter: Highlighter::shared(),
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn no_page_can_put_markup_into_the_reader(html in document()) {
        let out = render_hostile(&html);
        check_tags_and_attributes(&out.html);
    }

    #[test]
    fn no_page_can_make_the_reader_fetch_anything(html in document()) {
        let out = render_hostile(&html);
        check_sources(&out.html);
    }

    #[test]
    fn every_outline_entry_reaches_its_heading(html in document()) {
        let out = render_hostile(&html);
        check_outline(&out.outline, &out.html);
    }

    #[test]
    fn rendering_is_deterministic(html in document()) {
        // Derived heading ids come from a counter. If that counter ever
        // leaked between renders, two views of the same page would disagree
        // about their own anchors and the TOC would break on a revisit.
        prop_assert_eq!(render_hostile(&html), render_hostile(&html));
    }
}

// Arbitrary bytes, not just the hostile vocabulary above.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_html_never_produces_unexpected_markup(html in ".{0,2000}") {
        let out = render_hostile(&html);
        check_tags_and_attributes(&out.html);
        check_sources(&out.html);
        check_outline(&out.outline, &out.html);
    }
}
