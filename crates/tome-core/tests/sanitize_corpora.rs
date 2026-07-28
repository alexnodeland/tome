//! The sanitizer against its two corpora (S1-9).
//!
//! The plan requires BOTH to pass, and they pull in opposite directions:
//! the XSS corpus demands that nothing script-capable survives, and the
//! anchor corpus demands that nothing the TOC and deep links depend on
//! breaks. A sanitizer that passes only the first is the `id`-stripping
//! draft that silently killed the table of contents.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use tome_core::model::Node;
use tome_core::parse::parse_page;
use tome_core::sanitize::sanitize;
use url::Url;

fn base() -> Url {
    "https://docs.example.test/guide/index.html"
        .parse()
        .unwrap()
}

/// Parse then sanitize, the order the pipeline uses.
fn clean(html: &str) -> Node {
    let parsed = parse_page(html, &base(), None);
    sanitize(parsed.body)
}

/// Every href in the tree.
fn hrefs(node: &Node, out: &mut Vec<String>) {
    if let Node::Link { href, .. } = node {
        out.push(href.clone());
    }
    for child in children(node) {
        hrefs(child, out);
    }
}

/// Every img src in the tree.
fn srcs(node: &Node, out: &mut Vec<String>) {
    if let Node::Image { src, .. } = node {
        out.push(src.clone());
    }
    for child in children(node) {
        srcs(child, out);
    }
}

/// Every id in the tree (headings, anchors, definitions).
fn ids(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Heading { id: Some(id), .. } | Node::Anchor { id } => out.push(id.clone()),
        Node::DefinitionList { items } => {
            for d in items {
                if let Some(id) = &d.id {
                    out.push(id.clone());
                }
            }
        }
        _ => {}
    }
    for child in children(node) {
        ids(child, out);
    }
}

fn children(node: &Node) -> Vec<&Node> {
    match node {
        Node::Document { children }
        | Node::Heading { children, .. }
        | Node::Paragraph { children }
        | Node::Blockquote { children }
        | Node::Emphasis { children }
        | Node::Strong { children }
        | Node::Admonition { children, .. }
        | Node::Link { children, .. } => children.iter().collect(),
        Node::List { items, .. } => items.iter().flat_map(|i| i.children.iter()).collect(),
        Node::DefinitionList { items } => items
            .iter()
            .flat_map(|d| d.term.iter().chain(d.definition.iter()))
            .collect(),
        Node::Table { headers, rows } => headers
            .iter()
            .flat_map(|c| c.children.iter())
            .chain(
                rows.iter()
                    .flat_map(|r| r.cells.iter().flat_map(|c| c.children.iter())),
            )
            .collect(),
        _ => Vec::new(),
    }
}

// ============================================================================
// Corpus 1: XSS — nothing script-capable may survive.
// ============================================================================

/// Every payload here is a link/image href/src that must NOT reach the
/// renderer with a script-capable scheme.
const XSS_HREF_PAYLOADS: &[&str] = &[
    "javascript:alert(1)",
    "JavaScript:alert(1)",     // case
    "  javascript:alert(1)",   // leading spaces (browsers strip)
    "java\tscript:alert(1)",   // interior tab — the confirmed refute-panel bypass
    "java\nscript:alert(1)",   // interior newline
    "java\rscript:alert(1)",   // interior CR
    "\x01javascript:alert(1)", // leading control
    "vbscript:msgbox(1)",
    "data:text/html,<script>alert(1)</script>",
    "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
    "file:///etc/passwd",
];

#[test]
fn no_javascript_or_data_href_survives() {
    for payload in XSS_HREF_PAYLOADS {
        // Build a link with the payload as its href. Use an <a> the parser
        // will keep (it has an href), inside content.
        let html = format!("<main><p><a href=\"{payload}\">click here</a></p></main>");
        let tree = clean(&html);

        let mut found = Vec::new();
        hrefs(&tree, &mut found);
        for href in &found {
            let scheme = href
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            assert!(
                !matches!(scheme.as_str(), "javascript" | "data" | "vbscript" | "file"),
                "payload {payload:?} survived as href {href:?}"
            );
        }
        // The words survive even though the link did not.
        assert!(
            tree_text(&tree).contains("click here"),
            "neutralizing the link must keep its text, for {payload:?}"
        );
    }
}

#[test]
fn no_script_capable_image_src_survives() {
    for payload in [
        "javascript:alert(1)",
        "data:image/svg+xml,<svg onload=alert(1)>",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox(1)",
    ] {
        let html = format!("<main><p><img src=\"{payload}\" alt=\"diagram\"></p></main>");
        let tree = clean(&html);

        let mut found = Vec::new();
        srcs(&tree, &mut found);
        assert!(
            found.is_empty(),
            "image with payload {payload:?} should be dropped, found srcs {found:?}"
        );
        // Degrades to alt text.
        assert!(tree_text(&tree).contains("diagram"));
    }
}

#[test]
fn ids_cannot_break_out_of_attribute_context() {
    // Ids that, unsanitized, would break the id="" attribute. After
    // sanitization the danger is measured by what can actually break out of,
    // or split, an attribute: quotes, whitespace, and control characters.
    // (`<` and `>` are intentionally kept — inside a quoted attribute value
    // they are literal text; rustdoc anchors depend on it.)
    for hostile in [
        "x\"><script>alert(1)</script>", // the double-quote is the breakout char
        "x' onmouseover='alert(1)",      // single quote + spaces
        "a b c",                         // spaces would split into attributes
        "x\">",
        "x\tonclick=y", // tab as whitespace
    ] {
        let html = format!("<main><h2 id=\"{hostile}\">Heading</h2></main>");
        let tree = clean(&html);
        let mut found = Vec::new();
        ids(&tree, &mut found);
        for id in &found {
            assert!(
                !id.chars()
                    .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '`')),
                "id {id:?} (from {hostile:?}) still contains an attribute-breaking character"
            );
        }
    }
}

// ---- refute-panel regressions ----------------------------------------------

#[test]
fn interior_control_chars_in_scheme_are_caught() {
    // The confirmed HIGH/critical bypass: java\tscript: etc. survived because
    // only leading controls were trimmed.
    for payload in [
        "java\tscript:alert(1)",
        "java\nscript:alert(1)",
        "java\rscript:alert(1)",
        "j\ta\nv\ra\tscript:alert(1)",
    ] {
        let html = format!("<main><p><a href=\"{payload}\">x</a></p></main>");
        let tree = clean(&html);
        let mut found = Vec::new();
        hrefs(&tree, &mut found);
        assert!(
            found.is_empty(),
            "interior-control payload {payload:?} survived as a link: {found:?}"
        );
    }
}

#[test]
fn a_fragment_link_stays_consistent_with_its_target_id() {
    // If a character IS stripped, the id and the #fragment that targets it go
    // through the same function, so they still match.
    let html = "<main><h2 id=\"a b\">H</h2><p><a href=\"#a b\">jump</a></p></main>";
    let tree = clean(html);
    let mut id_list = Vec::new();
    ids(&tree, &mut id_list);
    let mut href_list = Vec::new();
    hrefs(&tree, &mut href_list);
    // Both had the space stripped: id "ab", href "#ab" — still a match.
    assert!(id_list.contains(&"ab".to_string()), "ids: {id_list:?}");
    assert!(
        href_list.contains(&"#ab".to_string()),
        "hrefs: {href_list:?}"
    );
}

#[test]
fn code_block_language_is_a_safe_class_token() {
    let html =
        "<main><pre><code class=\"language-x&quot; onload=&quot;alert(1)\">c</code></pre></main>";
    let tree = clean(html);
    fn langs(node: &Node, out: &mut Vec<Option<String>>) {
        if let Node::CodeBlock { language, .. } = node {
            out.push(language.clone());
        }
        for c in children(node) {
            langs(c, out);
        }
    }
    let mut found = Vec::new();
    langs(&tree, &mut found);
    for lang in found.into_iter().flatten() {
        assert!(
            lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "language {lang:?} is not a safe class token"
        );
    }
}

#[test]
fn admonition_kind_is_a_safe_class_token() {
    let html = "<main><div class=\"admonition x&quot;&gt;&lt;script&gt;\"><p>y</p></div></main>";
    let tree = clean(html);
    fn kinds(node: &Node, out: &mut Vec<String>) {
        if let Node::Admonition { kind, .. } = node {
            out.push(kind.clone());
        }
        for c in children(node) {
            kinds(c, out);
        }
    }
    let mut found = Vec::new();
    kinds(&tree, &mut found);
    for kind in &found {
        assert!(
            kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "admonition kind {kind:?} is not a safe class token"
        );
    }
}

// ============================================================================
// Corpus 2: anchors — nothing the TOC and deep links depend on may break.
// ============================================================================

/// Real ids seen on documentation pages; every one must survive sanitization
/// unchanged, or the TOC and `#fragment` links break.
const ANCHOR_IDS: &[&str] = &[
    "installation",
    "api-reference",
    "widget.Widget", // Sphinx dotted permalink
    "widget.Widget.resize",
    "os.path.join",
    "section-3",
    "module-widget",
    "v1.2.3",
    "std:option", // Sphinx cross-ref style
    "_private_note",
    // Refute-panel regressions: these MUST survive after the ASCII-allowlist
    // became a denylist.
    "café",               // non-ASCII (any non-English docs site)
    "安装",               // CJK slug
    "naïve",              // would collide with "nave" under ASCII-folding
    "impl-From<T>-for-T", // rustdoc generic-impl permalink
    "method.map%3CU%3E",  // rustdoc percent-encoded form
    "operator()",         // C++ signature anchor
];

#[test]
fn real_documentation_ids_survive_unchanged() {
    for id in ANCHOR_IDS {
        let html = format!("<main><h2 id=\"{id}\">Heading</h2><p>text</p></main>");
        let tree = clean(&html);
        let mut found = Vec::new();
        ids(&tree, &mut found);
        assert!(
            found.contains(&id.to_string()),
            "anchor id {id:?} was altered or dropped — the TOC would break; got {found:?}"
        );
    }
}

#[test]
fn definition_list_permalink_ids_survive() {
    // The Sphinx API-entry case: the dl carries the permalink target.
    let html = r##"<main><dl class="py class">
        <dt id="widget.Widget"><span class="sig-name">Widget</span></dt>
        <dd><p>A widget.</p></dd>
    </dl></main>"##;
    let tree = clean(html);
    let mut found = Vec::new();
    ids(&tree, &mut found);
    assert!(
        found.contains(&"widget.Widget".to_string()),
        "dl id lost: {found:?}"
    );
}

#[test]
fn ordinary_links_and_images_are_untouched() {
    // r## because href="#installation" contains the `"#` raw-string closer.
    let html = r##"<main><p>
        <a href="https://docs.python.org/3/">python</a>
        <a href="../api/os.html">os</a>
        <a href="mailto:a@b.test">mail</a>
        <a href="#installation">jump</a>
        <img src="https://x.test/diagram.png" alt="d">
    </p></main>"##;
    let tree = clean(html);

    let mut found_hrefs = Vec::new();
    hrefs(&tree, &mut found_hrefs);
    assert!(found_hrefs.iter().any(|h| h.contains("docs.python.org")));
    assert!(found_hrefs.iter().any(|h| h == "mailto:a@b.test"));
    assert!(found_hrefs
        .iter()
        .any(|h| h.contains("#installation") || h.ends_with("os.html")));

    let mut found_srcs = Vec::new();
    srcs(&tree, &mut found_srcs);
    assert!(found_srcs.iter().any(|s| s.contains("diagram.png")));
}

#[test]
fn sanitize_is_idempotent() {
    let html = "<main><h2 id=\"a-b.c\">H</h2><p><a href=\"javascript:x\">t</a></p></main>";
    let once = clean(html);
    let twice = sanitize(once.clone());
    assert_eq!(once, twice, "sanitizing clean output must change nothing");
}

// ---- helper ---------------------------------------------------------------

fn tree_text(node: &Node) -> String {
    node.text_content()
}
